//! Static sites: listing, creation, incremental publishing, files.
//!
//! Publishing is a stateless etag diff, with no local manifest: the server returns
//! the etag (hex MD5) of every draft file, the client hashes files locally and
//! uploads only the ones that differ, deletes the vanished ones, then publishes an
//! immutable snapshot. Batch uploads run CONCURRENTLY (the async advantage).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::api::models::sites::{
    FilesResponseSite, SiteAdd, SiteAddReq, SiteDisable, SiteFiles, SiteFilesDeleteBatch,
    SiteFilesPaths, SiteFilesUploadBatch, SiteGet, SitePublish, SitePublishBucketReq,
    SitePublishFromBucket, Sites, SitesList, SitesListInner, SitesResolve,
};
use crate::api::run::Run;
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::commands::util::Page;
use crate::t;
use crate::util::context::Context;
use crate::util::output::{info, success};
use anyhow::{bail, Context as _, Result};
use clap::Subcommand;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use md5::{Digest, Md5};
use reqwest::multipart::{Form, Part};

mod tests;

// Batch limits (see server-side restrictions): 100 is the Django
// DATA_UPLOAD_MAX_NUMBER_FILES ceiling, 32 MB is a sane size for one multipart.
const BATCH_MAX_FILES: usize = 100;
const BATCH_MAX_BYTES: u64 = 32 * 1024 * 1024;
const DELETE_BATCH: usize = 500;
// Number of upload batches in flight at once.
const UPLOAD_CONCURRENCY: usize = 4;
// Number of threads hashing local files.
const HASH_CONCURRENCY: usize = 8;

#[derive(Subcommand)]
pub enum SitesCommand {
    #[command(about = t!(cmd_sites_list))]
    List(Page),
    #[command(about = t!(cmd_sites_create))]
    Create {
        #[arg(help = t!(arg_site_hostname))]
        hostname: String,
        #[arg(long, help = t!(arg_site_domain))]
        domain: String,
    },
    #[command(about = t!(cmd_sites_publish))]
    Publish {
        #[arg(help = t!(arg_publish_hostname))]
        hostname: Option<String>,
        #[arg(long, help = t!(arg_publish_site_id))]
        site_id: Option<i64>,
        #[arg(long, help = t!(arg_publish_dir))]
        dir: PathBuf,
        #[arg(long, help = t!(arg_publish_dry_run))]
        dry_run: bool,
    },
    #[command(about = t!(cmd_sites_publish_bucket))]
    PublishFromBucket {
        #[arg(help = t!(arg_site_hostname))]
        hostname: String,
        #[arg(long, help = t!(arg_bucket))]
        bucket: String,
        #[arg(long, default_value = "", help = t!(arg_bucket_path))]
        path: String,
    },
    #[command(about = t!(cmd_sites_files))]
    Files {
        #[arg(help = t!(arg_site_hostname))]
        hostname: String,
    },
    #[command(about = t!(cmd_sites_disable))]
    Disable {
        #[arg(help = t!(arg_site_hostname))]
        hostname: String,
    },
}

impl Run for SitesCommand {
    async fn run<'a>(self, ctx: &'a mut Context<'a>) -> Result<ProgramRes> {
        let client = ctx.client()?;
        match self {
            Self::List(page) => list(client, page.into()).await.map(ProgramRes::from),
            Self::Create { hostname, domain } => create(client, &hostname, &domain)
                .await
                .map(ProgramRes::from),
            Self::Publish {
                hostname,
                site_id,
                dir,
                dry_run,
            } => {
                // --site-id skips the site listing: a narrow sites:publish token has nothing else.
                let id = match (site_id, hostname) {
                    (Some(id), _) => id,
                    (None, Some(host)) => resolve_site(client, host).await?.id,
                    (None, None) => bail!(t!(publish_needs_site_ref)),
                };
                publish(client, id, &dir, dry_run)
                    .await
                    .map(ProgramRes::from)
            }
            Self::PublishFromBucket {
                hostname,
                bucket,
                path,
            } => {
                let site = resolve_site(client, hostname).await?;
                publish_from_bucket(client, site.id, &bucket, &path)
                    .await
                    .map(ProgramRes::from)
            }
            Self::Files { hostname } => files(client, hostname).await.map(ProgramRes::from),
            Self::Disable { hostname } => {
                let site = resolve_site(client, hostname).await?;
                client.send::<SiteDisable>(site.id).await?;
                success(t!(site_disabled, &site.hostname));
                Ok(ProgramRes::Idle)
            }
        }
    }
}

async fn resolve_site(client: &Client<'_>, hostname: String) -> Result<SitesListInner> {
    let needle = hostname.trim().to_lowercase();

    let sites = client.send::<SitesResolve>(needle).await?;

    sites
        .results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!(t!(not_found_site, &hostname)))
}

async fn list(client: &Client<'_>, page: u32) -> Result<SitesList> {
    client.send::<Sites>(page).await
}

async fn create(client: &Client<'_>, hostname: &str, domain: &str) -> Result<SitesListInner> {
    let d = resolve_domain(client, domain).await?;
    let site: SitesListInner = client
        .send_json::<SiteAdd>(
            SiteAddReq {
                hostname: hostname.to_string(),
                domain_id: d.id,
            },
            (),
        )
        .await?;

    Ok(site)
}

async fn files(client: &Client<'_>, hostname: String) -> Result<FilesResponseSite> {
    let site = resolve_site(client, hostname).await?;

    let resp: FilesResponseSite = client.send::<SiteFiles>(site.id).await?;

    Ok(resp)
}

// --- Publish from an object-storage bucket (variant B) ---

// Poll interval and ceiling while the server ingests+publishes asynchronously.
const BUCKET_POLL_INTERVAL_SECS: u64 = 2;
const BUCKET_POLL_MAX_ATTEMPTS: usize = 300; // ~10 minutes.

async fn publish_from_bucket(
    client: &Client<'_>,
    site_id: i64,
    bucket: &str,
    path: &str,
) -> Result<()> {
    // Kick off the async publish (202 → status "publishing").
    client
        .send_json::<SitePublishFromBucket>(
            SitePublishBucketReq {
                bucket: bucket.to_string(),
                path: path.to_string(),
            },
            site_id,
        )
        .await?;

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner());
    spinner.set_message(t!(bucket_publish_started).to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(120));

    // Poll until the site leaves the transient "publishing" status.
    for _ in 0..BUCKET_POLL_MAX_ATTEMPTS {
        tokio::time::sleep(std::time::Duration::from_secs(BUCKET_POLL_INTERVAL_SECS)).await;
        let site: SitesListInner = client.send::<SiteGet>(site_id).await?;
        match site.status.as_deref() {
            Some("publishing") => continue,
            Some("active") => {
                spinner.finish_and_clear();
                let version = site
                    .content_version
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                success(t!(bucket_published, &version));
                return Ok(());
            }
            _ => {
                spinner.finish_and_clear();
                let err = site.publish_error.unwrap_or_default();
                bail!(t!(bucket_publish_failed, &err));
            }
        }
    }
    spinner.finish_and_clear();
    bail!(t!(bucket_publish_timeout))
}

// --- Publishing ---

async fn publish(client: &Client<'_>, site_id: i64, dir: &Path, dry_run: bool) -> Result<()> {
    let root = std::fs::canonicalize(dir)
        .with_context(|| t!(dir_not_found, &dir.display().to_string()))?;
    if !root.is_dir() {
        bail!(t!(not_a_dir, &root.display().to_string()));
    }

    let resp = client.send::<SiteFiles>(site_id).await?;

    // 1. Current draft state on the server: path -> etag.
    let server: HashMap<String, String> = resp
        .files
        .into_iter()
        .filter(|f| !f.is_dir)
        .map(|f| (f.path, f.etag.unwrap_or_default()))
        .collect();

    // 2. Local files + MD5 (concurrent hashing).
    let local = scan_local(&root).await?;

    // 3. Diff.
    let mut to_upload: Vec<(String, PathBuf)> = local
        .iter()
        .filter(|(rel, (_, md5))| server.get(*rel).map(|e| e != md5).unwrap_or(true))
        .map(|(rel, (abs, _))| (rel.clone(), abs.clone()))
        .collect();
    to_upload.sort();
    let mut to_delete: Vec<String> = server
        .keys()
        .filter(|k| !local.contains_key(*k))
        .cloned()
        .collect();
    to_delete.sort();

    let unchanged = local.len() - to_upload.len();
    println!(
        "{}",
        t!(
            publish_summary,
            &site_id.to_string(),
            &local.len().to_string(),
            &server.len().to_string(),
            &to_upload.len().to_string(),
            &to_delete.len().to_string(),
            &unchanged.to_string()
        )
    );

    if to_upload.is_empty() && to_delete.is_empty() {
        info(t!(publish_no_changes));
        return Ok(());
    }
    if dry_run {
        info(t!(publish_dry_run));
        return Ok(());
    }

    // 4. Upload changed files in batches, concurrently.
    if !to_upload.is_empty() {
        upload_all(client, site_id, to_upload).await?;
    }
    // 5. Delete vanished files.
    if !to_delete.is_empty() {
        delete_all(client, site_id, &to_delete).await?;
    }
    // 6. Publish the snapshot.
    client.send::<SitePublish>(site_id).await?;
    success(t!(published));
    Ok(())
}

/// Walks the directory and computes the MD5 of every file (symlinks are skipped).
async fn scan_local(root: &Path) -> Result<HashMap<String, (PathBuf, String)>> {
    let root = root.to_path_buf();
    let entries: Vec<(String, PathBuf)> = tokio::task::spawn_blocking(move || {
        let mut out = Vec::new();
        for entry in walkdir::WalkDir::new(&root).follow_links(false) {
            let entry = entry?;
            let ft = entry.file_type();
            if ft.is_symlink() || !ft.is_file() {
                continue;
            }
            let abs = entry.path().to_path_buf();
            let rel = abs
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, abs));
        }
        Ok::<_, walkdir::Error>(out)
    })
    .await
    .context("directory walk failed")??;

    // Hash files concurrently on the blocking pool.
    let hashed = stream::iter(entries.into_iter().map(|(rel, abs)| async move {
        let abs2 = abs.clone();
        let digest = tokio::task::spawn_blocking(move || -> Result<String> {
            let bytes = std::fs::read(&abs2)
                .with_context(|| format!("failed to read {}", abs2.display()))?;
            let mut hasher = Md5::new();
            hasher.update(&bytes);
            Ok(hex::encode(hasher.finalize()))
        })
        .await
        .context("hashing failed")??;
        Ok::<_, anyhow::Error>((rel, (abs, digest)))
    }))
    .buffer_unordered(HASH_CONCURRENCY)
    .collect::<Vec<Result<_>>>()
    .await;

    let mut map = HashMap::with_capacity(hashed.len());
    for item in hashed {
        let (rel, meta) = item?;
        map.insert(rel, meta);
    }
    Ok(map)
}

/// Groups files into batches by count and total size.
fn make_batches(files: Vec<(String, PathBuf)>) -> Vec<Vec<(String, PathBuf)>> {
    let mut batches = Vec::new();
    let mut batch: Vec<(String, PathBuf)> = Vec::new();
    let mut bytes: u64 = 0;
    for (rel, abs) in files {
        let size = std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
        if !batch.is_empty() && (batch.len() >= BATCH_MAX_FILES || bytes + size > BATCH_MAX_BYTES) {
            batches.push(std::mem::take(&mut batch));
            bytes = 0;
        }
        batch.push((rel, abs));
        bytes += size;
    }
    if !batch.is_empty() {
        batches.push(batch);
    }
    batches
}

async fn upload_all(
    client: &Client<'_>,
    site_id: i64,
    files: Vec<(String, PathBuf)>,
) -> Result<()> {
    let total = files.len() as u64;
    let batches = make_batches(files);
    let bar = ProgressBar::new(total);
    bar.set_style(ProgressStyle::with_template(t!(upload_bar))?.progress_chars("=>-"));

    let results = stream::iter(batches.into_iter().map(|batch| {
        let bar = bar.clone();
        async move {
            let n = batch.len() as u64;
            upload_batch(client, site_id, batch).await?;
            bar.inc(n);
            Ok::<_, anyhow::Error>(())
        }
    }))
    .buffer_unordered(UPLOAD_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;

    bar.finish_and_clear();
    for r in results {
        r?;
    }
    Ok(())
}

async fn upload_batch(
    client: &Client<'_>,
    site_id: i64,
    batch: Vec<(String, PathBuf)>,
) -> Result<()> {
    let mut form = Form::new();
    for (rel, abs) in &batch {
        let data = tokio::fs::read(abs)
            .await
            .with_context(|| format!("failed to read {}", abs.display()))?;
        let ctype = mime_guess::from_path(rel)
            .first_or_octet_stream()
            .to_string();
        let filename = rel.rsplit('/').next().unwrap_or(rel).to_string();
        // Order matters: the server pairs paths[i] with files[i] by index.
        form = form.text("paths", rel.clone());
        let part = Part::bytes(data).file_name(filename).mime_str(&ctype)?;
        form = form.part("files", part);
    }
    client
        .send_multipart::<SiteFilesUploadBatch>(site_id, form)
        .await?;
    Ok(())
}

async fn delete_all(client: &Client<'_>, site_id: i64, paths: &[String]) -> Result<()> {
    for chunk in paths.chunks(DELETE_BATCH) {
        client
            .send_json::<SiteFilesDeleteBatch>(
                SiteFilesPaths {
                    paths: chunk.to_owned(),
                },
                site_id,
            )
            .await?;
    }
    info(t!(deleted_files, &paths.len().to_string()));
    Ok(())
}

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
use crate::api::table::ProgramRes;
use crate::api::Client;
use crate::commands::domains::resolve_domain;
use crate::i18n::{self, M};
use crate::util::output::{info, success};
use crate::Context;
use anyhow::{bail, Context as _, Result};
use clap::Subcommand;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use md5::{Digest, Md5};
use reqwest::multipart::{Form, Part};

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
    /// List static sites.
    List {
        #[arg(value_name = "PAGE(1..n)")]
        page: u32,
    },
    /// Create a static site on a host.
    Create {
        /// Site hostname (e.g. www.example.com).
        hostname: String,
        /// Owner domain.
        #[arg(long)]
        domain: String,
    },
    /// Incrementally publish a directory as the site content.
    Publish {
        /// Site hostname (omit when --site-id is given).
        hostname: Option<String>,
        /// Site id; skips the hostname lookup (useful for narrow sites:publish tokens).
        #[arg(long)]
        site_id: Option<i64>,
        /// Built site directory.
        #[arg(long)]
        dir: PathBuf,
        /// Show the plan without applying changes.
        #[arg(long)]
        dry_run: bool,
    },
    /// Publish the site from one of your own object-storage buckets.
    PublishFromBucket {
        /// Site hostname.
        hostname: String,
        /// Source bucket name (short `<name>` or full `u<id>-<name>`).
        #[arg(long)]
        bucket: String,
        /// Source prefix (directory) inside the bucket. Empty = bucket root.
        #[arg(long, default_value = "")]
        path: String,
    },
    /// List files of the current draft.
    Files { hostname: String },
    /// Unpublish the site.
    Disable { hostname: String },
}

pub async fn run(ctx: &Context, cmd: SitesCommand) -> Result<ProgramRes> {
    let client = ctx.client()?;
    match cmd {
        SitesCommand::List { page } => list(&client, page).await.map(ProgramRes::from),
        SitesCommand::Create { hostname, domain } => create(&client, &hostname, &domain)
            .await
            .map(ProgramRes::from),
        SitesCommand::Publish {
            hostname,
            site_id,
            dir,
            dry_run,
        } => {
            // --site-id обходит листинг сайтов: узкому токену sites:publish его достаточно.
            let id = match (site_id, hostname) {
                (Some(id), _) => id,
                (None, Some(host)) => resolve_site(&client, host).await?.id,
                (None, None) => bail!(i18n::tr(M::PublishNeedsSiteRef)),
            };
            publish(&client, id, &dir, dry_run)
                .await
                .map(ProgramRes::from)
        }
        SitesCommand::PublishFromBucket {
            hostname,
            bucket,
            path,
        } => {
            let site = resolve_site(&client, hostname).await?;
            publish_from_bucket(&client, site.id, &bucket, &path)
                .await
                .map(ProgramRes::from)
        }
        SitesCommand::Files { hostname } => files(&client, hostname).await.map(ProgramRes::from),
        SitesCommand::Disable { hostname } => {
            let site = resolve_site(&client, hostname).await?;
            client.n_send::<SiteDisable>(site.id).await?;
            success(i18n::f(M::SiteDisabled, &[("host", &site.hostname)]));
            Ok(ProgramRes::Idle)
        }
    }
}

async fn resolve_site(client: &Client, hostname: String) -> Result<SitesListInner> {
    let needle = hostname.trim().to_lowercase();

    let sites = client.n_send::<SitesResolve>(needle).await?;

    sites
        .results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!(i18n::f(M::NotFoundSite, &[("host", &hostname)])))
}

async fn list(client: &Client, page: u32) -> Result<SitesList> {
    client.n_send::<Sites>(page).await
}

async fn create(client: &Client, hostname: &str, domain: &str) -> Result<SitesListInner> {
    let d = resolve_domain(client, domain).await?;
    let site: SitesListInner = client
        .n_send_ser::<SiteAdd>(
            SiteAddReq {
                hostname: hostname.to_string(),
                domain_id: d.id,
            },
            (),
        )
        .await?;

    Ok(site)
}

async fn files(client: &Client, hostname: String) -> Result<FilesResponseSite> {
    let site = resolve_site(client, hostname).await?;

    let resp: FilesResponseSite = client.n_send::<SiteFiles>(site.id).await?;

    Ok(resp)
}

// --- Publish from an object-storage bucket (variant B) ---

// Poll interval and ceiling while the server ingests+publishes asynchronously.
const BUCKET_POLL_INTERVAL_SECS: u64 = 2;
const BUCKET_POLL_MAX_ATTEMPTS: usize = 300; // ~10 minutes.

async fn publish_from_bucket(
    client: &Client,
    site_id: i64,
    bucket: &str,
    path: &str,
) -> Result<()> {
    // Kick off the async publish (202 → status "publishing").
    client
        .n_send_ser::<SitePublishFromBucket>(
            SitePublishBucketReq {
                bucket: bucket.to_string(),
                path: path.to_string(),
            },
            site_id,
        )
        .await?;

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner());
    spinner.set_message(i18n::tr(M::BucketPublishStarted).to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(120));

    // Poll until the site leaves the transient "publishing" status.
    for _ in 0..BUCKET_POLL_MAX_ATTEMPTS {
        tokio::time::sleep(std::time::Duration::from_secs(BUCKET_POLL_INTERVAL_SECS)).await;
        let site: SitesListInner = client.n_send::<SiteGet>(site_id).await?;
        match site.status.as_deref() {
            Some("publishing") => continue,
            Some("active") => {
                spinner.finish_and_clear();
                let version = site
                    .content_version
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                success(i18n::f(M::BucketPublished, &[("version", &version)]));
                return Ok(());
            }
            _ => {
                spinner.finish_and_clear();
                let err = site.publish_error.unwrap_or_default();
                bail!(i18n::f(M::BucketPublishFailed, &[("error", &err)]));
            }
        }
    }
    spinner.finish_and_clear();
    bail!(i18n::tr(M::BucketPublishTimeout))
}

// --- Publishing ---

async fn publish(client: &Client, site_id: i64, dir: &Path, dry_run: bool) -> Result<()> {
    let root = std::fs::canonicalize(dir)
        .with_context(|| i18n::f(M::DirNotFound, &[("path", &dir.display().to_string())]))?;
    if !root.is_dir() {
        bail!(i18n::f(
            M::NotADir,
            &[("path", &root.display().to_string())]
        ));
    }

    let resp = client.n_send::<SiteFiles>(site_id).await?;

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
        i18n::f(
            M::PublishSummary,
            &[
                ("id", &site_id.to_string()),
                ("local", &local.len().to_string()),
                ("server", &server.len().to_string()),
                ("up", &to_upload.len().to_string()),
                ("del", &to_delete.len().to_string()),
                ("same", &unchanged.to_string()),
            ],
        )
    );

    if to_upload.is_empty() && to_delete.is_empty() {
        info(i18n::tr(M::PublishNoChanges));
        return Ok(());
    }
    if dry_run {
        info(i18n::tr(M::PublishDryRun));
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
    client.n_send::<SitePublish>(site_id).await?;
    success(i18n::tr(M::Published));
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

    let mut map = HashMap::new();
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

async fn upload_all(client: &Client, site_id: i64, files: Vec<(String, PathBuf)>) -> Result<()> {
    let total = files.len() as u64;
    let batches = make_batches(files);
    let bar = ProgressBar::new(total);
    bar.set_style(ProgressStyle::with_template(i18n::tr(M::UploadBar))?.progress_chars("=>-"));

    let results = stream::iter(batches.into_iter().map(|batch| {
        let client = client.clone();
        let bar = bar.clone();
        async move {
            let n = batch.len() as u64;
            upload_batch(&client, site_id, batch).await?;
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

async fn upload_batch(client: &Client, site_id: i64, batch: Vec<(String, PathBuf)>) -> Result<()> {
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
        .n_send_multipart::<SiteFilesUploadBatch>(site_id, form)
        .await?;
    Ok(())
}

async fn delete_all(client: &Client, site_id: i64, paths: &[String]) -> Result<()> {
    for chunk in paths.chunks(DELETE_BATCH) {
        client
            .n_send_ser::<SiteFilesDeleteBatch>(
                SiteFilesPaths {
                    paths: chunk.to_owned(),
                },
                site_id,
            )
            .await?;
    }
    info(i18n::f(
        M::DeletedFiles,
        &[("count", &paths.len().to_string())],
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use wiremock::matchers::{body_json, method, path as url_path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn md5_hex(data: &[u8]) -> String {
        let mut hasher = Md5::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    fn touch(dir: &Path, rel: &str, content: &[u8]) -> PathBuf {
        let abs = dir.join(rel);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&abs, content).unwrap();
        abs
    }

    #[test]
    fn make_batches_splits_by_file_count() {
        let dir = tempfile::tempdir().unwrap();
        let files: Vec<(String, PathBuf)> = (0..250)
            .map(|i| {
                let rel = format!("f{i}.txt");
                let abs = touch(dir.path(), &rel, b"");
                (rel, abs)
            })
            .collect();
        let batches = make_batches(files);
        let sizes: Vec<usize> = batches.iter().map(Vec::len).collect();
        assert_eq!(sizes, vec![100, 100, 50]);
    }

    #[test]
    fn make_batches_splits_by_total_size() {
        let dir = tempfile::tempdir().unwrap();
        // Sparse 20 MB files: two of them exceed the 32 MB batch ceiling.
        let files: Vec<(String, PathBuf)> = (0..3)
            .map(|i| {
                let rel = format!("big{i}.bin");
                let abs = dir.path().join(&rel);
                let f = std::fs::File::create(&abs).unwrap();
                f.set_len(20 * 1024 * 1024).unwrap();
                (rel, abs)
            })
            .collect();
        let batches = make_batches(files);
        let sizes: Vec<usize> = batches.iter().map(Vec::len).collect();
        assert_eq!(sizes, vec![1, 1, 1]);
    }

    #[test]
    fn make_batches_of_nothing_is_empty() {
        assert!(make_batches(Vec::new()).is_empty());
    }

    fn client(server: &MockServer) -> Client {
        Client::new(&server.uri(), "wsk_test").unwrap()
    }

    /// Mounts `GET files` returning the given draft state.
    async fn mount_files(server: &MockServer, files: Value) {
        Mock::given(method("GET"))
            .and(url_path("/api/v1/static-sites/5/files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "files": files })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn publish_uploads_changed_deletes_vanished_then_publishes() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "index.html", b"<html>new</html>");
        touch(dir.path(), "css/app.css", b"body{}");
        let same = b"unchanged";
        touch(dir.path(), "same.txt", same);

        let server = MockServer::start().await;
        // Draft on the server: index.html is stale, same.txt matches its local
        // MD5 (must be skipped), old.txt vanished locally (must be deleted).
        mount_files(
            &server,
            json!([
                {"path": "index.html", "etag": "0000stale0000"},
                {"path": "same.txt", "etag": md5_hex(same)},
                {"path": "old.txt", "etag": "aaaa"},
            ]),
        )
        .await;
        // index.html + css/app.css fit one batch → exactly one upload request.
        Mock::given(method("POST"))
            .and(url_path("/api/v1/static-sites/5/upload"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(url_path("/api/v1/static-sites/5/delete-files"))
            .and(body_json(json!({"paths": ["old.txt"]})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(url_path("/api/v1/static-sites/5/publish"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;

        publish(&client(&server), 5, dir.path(), false)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn publish_dry_run_only_reads() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "index.html", b"data");

        let server = MockServer::start().await;
        mount_files(&server, json!([])).await;
        // No POST mocks are mounted: any write attempt would 404 and fail the run.
        publish(&client(&server), 5, dir.path(), true)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn publish_skips_snapshot_when_nothing_changed() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"stable";
        touch(dir.path(), "index.html", content);

        let server = MockServer::start().await;
        mount_files(
            &server,
            json!([{"path": "index.html", "etag": md5_hex(content)}]),
        )
        .await;
        publish(&client(&server), 5, dir.path(), false)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn publish_fails_on_missing_directory() {
        let server = MockServer::start().await;
        let missing = std::env::temp_dir().join("webshield-cli-no-such-dir-xyz");
        assert!(publish(&client(&server), 5, &missing, true).await.is_err());
    }

    #[tokio::test]
    async fn publish_from_bucket_posts_body_and_polls_until_active() {
        let server = MockServer::start().await;
        // The action posts {bucket, path} and returns 202 with status "publishing".
        Mock::given(method("POST"))
            .and(url_path("/api/v1/static-sites/5/publish-from-bucket"))
            .and(body_json(json!({"bucket": "web", "path": "public/"})))
            .respond_with(
                ResponseTemplate::new(202)
                    .set_body_json(json!({"id": 5, "hostname": "s", "status": "publishing"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        // Polling the site returns "active" → success.
        Mock::given(method("GET"))
            .and(url_path("/api/v1/static-sites/5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"id": 5, "hostname": "s", "status": "active", "content_version": 7}),
            ))
            .mount(&server)
            .await;

        publish_from_bucket(&client(&server), 5, "web", "public/")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn publish_from_bucket_surfaces_publish_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(url_path("/api/v1/static-sites/5/publish-from-bucket"))
            .respond_with(
                ResponseTemplate::new(202)
                    .set_body_json(json!({"id": 5, "hostname": "s", "status": "publishing"})),
            )
            .mount(&server)
            .await;
        // Async publish failed: status reverted, publish_error carries the reason.
        Mock::given(method("GET"))
            .and(url_path("/api/v1/static-sites/5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"id": 5, "hostname": "s", "status": "disabled", "publish_error": "boom"}),
            ))
            .mount(&server)
            .await;

        let err = publish_from_bucket(&client(&server), 5, "web", "")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("boom"));
    }
}

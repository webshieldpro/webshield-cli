//! Apps (compute): deploy a directory, follow the build, read logs.
//!
//! `deploy` packs the directory into a `.tar.gz` (honouring `.gitignore` and
//! `.webshieldignore`, never `.git`), uploads it and waits for the deployment: the
//! server builds by Dockerfile or by the auto-detected stack and rolls out the image.
//! Project and app may come from flags or from `webshield.toml` next to the code:
//!
//! ```toml
//! [app]
//! project = "shop"
//! name = "web"
//! subdir = "."
//! ```

mod data;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::api::models::apps::{
    App, AppList, AppsByName, ComputeBillingGet, DeployUpload, DeployUrl, DeployUrlReq, Deployment,
    DeploymentGet, Deployments, LogLine, Logs, Projects, Redeploy,
};
use crate::api::Client;
use crate::commands::prog_res::ProgramRes;
use crate::commands::run::Run;
use crate::t;
use crate::util::context::Context;
use crate::util::output::{info, success, warn};
use anyhow::{bail, Context as _, Result};
use clap::{Args, Subcommand};
use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;

/// Server-side limit on the uploaded archive.
const MAX_ARCHIVE_BYTES: usize = 100 * 1024 * 1024;
const POLL: Duration = Duration::from_secs(3);
const FOLLOW: Duration = Duration::from_secs(2);
/// Build log lines shown when a deployment fails.
const LOG_TAIL: usize = 40;

#[derive(Args, Clone, Default)]
pub struct AppRef {
    #[arg(long, help = t!(arg_app_project))]
    project: Option<String>,
    #[arg(long, help = t!(arg_app_name))]
    app: Option<String>,
    #[arg(long, help = t!(arg_app_dir), default_value = ".")]
    dir: PathBuf,
}

#[derive(Subcommand)]
#[command(about = t!(cmd_apps))]
pub enum AppsCommand {
    #[command(about = t!(cmd_apps_list))]
    List,
    #[command(about = t!(cmd_apps_deploy))]
    Deploy {
        #[command(flatten)]
        target: AppRef,
        #[arg(long, help = t!(arg_app_subdir))]
        subdir: Option<String>,
        #[arg(long, help = t!(arg_app_url))]
        url: Option<String>,
        #[arg(long, help = t!(arg_app_no_wait))]
        no_wait: bool,
    },
    #[command(about = t!(cmd_apps_deployments))]
    Deployments {
        #[command(flatten)]
        target: AppRef,
    },
    #[command(about = t!(cmd_apps_redeploy))]
    Redeploy {
        #[arg(help = t!(arg_deployment_id))]
        deployment: i64,
        #[arg(long, help = t!(arg_app_no_wait))]
        no_wait: bool,
    },
    #[command(about = t!(cmd_apps_logs))]
    Logs {
        #[command(flatten)]
        target: AppRef,
        #[arg(long, default_value = "1h", help = t!(arg_logs_since))]
        since: String,
        #[arg(long, help = t!(arg_logs_search))]
        search: Option<String>,
        #[arg(long, short = 'f', help = t!(arg_logs_follow))]
        follow: bool,
    },
    #[command(subcommand)]
    #[command(about = t!(cmd_apps_env))]
    Env(data::EnvCommand),
    #[command(subcommand)]
    #[command(about = t!(cmd_apps_db))]
    Db(data::DbCommand),
    #[command(subcommand)]
    #[command(about = t!(cmd_apps_kv))]
    Kv(data::KvCommand),
    #[command(about = t!(cmd_apps_usage))]
    Usage {
        #[command(flatten)]
        target: data::ProjectRef,
    },
    #[command(about = t!(cmd_apps_cap))]
    Cap {
        #[command(flatten)]
        target: data::ProjectRef,
        #[arg(help = t!(arg_cap_amount))]
        amount: String,
    },
    #[command(about = t!(cmd_apps_balance))]
    Balance,
}

impl Run for AppsCommand {
    async fn run<'a>(self, ctx: &'a mut Context<'a>) -> Result<ProgramRes> {
        let yes = ctx.yes;
        let client = ctx.client()?;
        match self {
            Self::List => list(client).await.map(ProgramRes::from),
            Self::Deploy {
                target,
                subdir,
                url,
                no_wait,
            } => {
                let file = Manifest::load(&target.dir)?;
                let app = resolve(client, &target, &file).await?;
                let subdir = subdir.or(file.app.subdir).unwrap_or_else(|| ".".into());
                let dep = match url {
                    Some(url) => {
                        let req = DeployUrlReq {
                            service: app.id,
                            source_url: url,
                            source_subdir: subdir,
                        };
                        client.send_json::<DeployUrl>(req, ()).await?
                    }
                    None => upload(client, &app, &target.dir, &subdir).await?,
                };
                finish(client, dep, no_wait).await
            }
            Self::Deployments { target } => {
                let app = resolve(client, &target, &Manifest::load(&target.dir)?).await?;
                client
                    .send::<Deployments>(app.id)
                    .await
                    .map(ProgramRes::from)
            }
            Self::Redeploy {
                deployment,
                no_wait,
            } => {
                let dep = client.send::<Redeploy>(deployment).await?;
                finish(client, dep, no_wait).await
            }
            Self::Logs {
                target,
                since,
                search,
                follow,
            } => {
                let app = resolve(client, &target, &Manifest::load(&target.dir)?).await?;
                logs(client, app.id, since, search, follow).await?;
                Ok(ProgramRes::Idle)
            }
            Self::Env(cmd) => data::env(client, cmd).await,
            Self::Db(cmd) => data::db(client, cmd, yes).await,
            Self::Kv(cmd) => data::kv(client, cmd, yes).await,
            Self::Usage { target } => data::usage(client, target).await,
            Self::Cap { target, amount } => data::cap(client, target, &amount).await,
            Self::Balance => client
                .send::<ComputeBillingGet>(())
                .await
                .map(ProgramRes::from),
        }
    }
}

#[derive(Deserialize, Default)]
struct Manifest {
    #[serde(default)]
    app: ManifestApp,
}

#[derive(Deserialize, Default)]
struct ManifestApp {
    project: Option<String>,
    name: Option<String>,
    subdir: Option<String>,
}

impl Manifest {
    /// `webshield.toml` in the directory; a missing file is an empty manifest.
    fn load(dir: &Path) -> Result<Self> {
        let path = dir.join("webshield.toml");
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text)
                .with_context(|| t!(manifest_invalid, &path.display().to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| t!(manifest_invalid, &path.display().to_string())),
        }
    }
}

async fn list(client: &Client<'_>) -> Result<AppList> {
    let projects = client.send::<Projects>(()).await?;
    let mut apps = client.send::<AppsByName>((None, None)).await?;
    apps.project_names = projects
        .results
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    Ok(apps)
}

/// The app named by flags or `webshield.toml`; with a single app on the account names are optional.
async fn resolve(client: &Client<'_>, target: &AppRef, file: &Manifest) -> Result<App> {
    let project = target.project.clone().or_else(|| file.app.project.clone());
    let name = target.app.clone().or_else(|| file.app.name.clone());
    let found = client
        .send::<AppsByName>((project.clone(), name.clone()))
        .await?;
    match found.results.len() {
        1 => Ok(found.results.into_iter().next().unwrap()),
        0 => bail!(t!(
            app_not_found,
            project.as_deref().unwrap_or("*"),
            name.as_deref().unwrap_or("*")
        )),
        _ => bail!(t!(app_ambiguous)),
    }
}

/// Files to upload: `.gitignore`/`.webshieldignore` rules apply, hidden files stay
/// (`.npmrc`, `.python-version`…), `.git` never leaves the machine.
fn collect(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        bail!(t!(not_a_dir, &dir.display().to_string()));
    }
    let mut files = Vec::new();
    let walker = ignore::WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(true)
        .require_git(false)
        .add_custom_ignore_filename(".webshieldignore")
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();
    for entry in walker {
        let entry = entry?;
        if entry.file_type().is_some_and(|t| t.is_file()) {
            files.push(entry.into_path());
        }
    }
    files.sort();
    Ok(files)
}

/// `.tar.gz` of the files, paths relative to `dir`.
fn pack(dir: &Path, files: &[PathBuf]) -> Result<Vec<u8>> {
    let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    tar.follow_symlinks(false);
    for path in files {
        let rel = path.strip_prefix(dir).unwrap_or(path);
        tar.append_path_with_name(path, rel)
            .with_context(|| path.display().to_string())?;
        if tar.get_ref().get_ref().len() > MAX_ARCHIVE_BYTES {
            bail!(t!(
                archive_too_large,
                &(MAX_ARCHIVE_BYTES / 1024 / 1024).to_string()
            ));
        }
    }
    let mut gz = tar.into_inner()?;
    gz.flush()?;
    let bytes = gz.finish()?;
    if bytes.len() > MAX_ARCHIVE_BYTES {
        bail!(t!(
            archive_too_large,
            &(MAX_ARCHIVE_BYTES / 1024 / 1024).to_string()
        ));
    }
    Ok(bytes)
}

async fn upload(client: &Client<'_>, app: &App, dir: &Path, subdir: &str) -> Result<Deployment> {
    let files = collect(dir)?;
    let archive = pack(dir, &files)?;
    info(t!(
        archive_packed,
        &files.len().to_string(),
        &format!("{:.1}", archive.len() as f64 / 1048576.0)
    ));
    let form = Form::new()
        .part(
            "archive",
            Part::bytes(archive)
                .file_name("source.tar.gz")
                .mime_str("application/gzip")?,
        )
        .text("subdir", subdir.to_string());
    client.send_multipart::<DeployUpload>(app.id, form).await
}

/// Waits for the deployment unless told not to; a failed one is an error (non-zero exit for CI).
async fn finish(client: &Client<'_>, dep: Deployment, no_wait: bool) -> Result<ProgramRes> {
    info(t!(deployment_started, &dep.id.to_string()));
    if no_wait {
        return Ok(ProgramRes::from(dep));
    }
    let mut last = dep.status.clone();
    info(t!(deployment_status, &last));
    let mut current = dep;
    while current.in_progress() {
        tokio::time::sleep(POLL).await;
        current = client.send::<DeploymentGet>(current.id).await?;
        if current.status != last {
            last = current.status.clone();
            info(t!(deployment_status, &last));
        }
    }
    if current.status == "active" {
        success(t!(deployment_live, &current.id.to_string()));
        return Ok(ProgramRes::from(current));
    }
    let full = client.send::<DeploymentGet>(current.id).await?;
    if let Some(log) = full.build_log.as_deref().filter(|l| !l.is_empty()) {
        let lines: Vec<&str> = log.lines().collect();
        for line in &lines[lines.len().saturating_sub(LOG_TAIL)..] {
            eprintln!("  {line}");
        }
    }
    bail!(t!(deployment_failed, &full.id.to_string(), &full.error))
}

fn print_lines(lines: &[LogLine]) {
    for line in lines {
        if line.stream == "stderr" {
            warn(&line.message);
        } else {
            println!("{}", line.message);
        }
    }
}

async fn logs(
    client: &Client<'_>,
    app: i64,
    since: String,
    search: Option<String>,
    follow: bool,
) -> Result<()> {
    let first = client
        .send::<Logs>((app, since.clone(), search.clone(), None))
        .await?;
    print_lines(&first.lines);
    if !follow {
        return Ok(());
    }
    let mut last = first.lines.last().map(|l| l.ts.clone());
    loop {
        tokio::time::sleep(FOLLOW).await;
        let next = client
            .send::<Logs>((app, since.clone(), search.clone(), last.clone()))
            .await?;
        if let Some(line) = next.lines.last() {
            last = Some(line.ts.clone());
        }
        print_lines(&next.lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn collect_honours_ignores_and_skips_git() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".git/objects")).unwrap();
        fs::write(root.join(".git/config"), "x").unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), "x").unwrap();
        fs::write(root.join(".gitignore"), "node_modules/\n*.log\n").unwrap();
        fs::write(root.join(".webshieldignore"), "secret.txt\n").unwrap();
        fs::write(root.join("app.log"), "x").unwrap();
        fs::write(root.join("secret.txt"), "x").unwrap();
        fs::write(root.join(".npmrc"), "x").unwrap();
        fs::write(root.join("Dockerfile"), "FROM scratch").unwrap();

        let names: Vec<String> = collect(root)
            .unwrap()
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(
            names,
            vec![".gitignore", ".npmrc", ".webshieldignore", "Dockerfile"]
        );
    }

    #[test]
    fn pack_makes_gzip_with_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.py"), "print(1)").unwrap();
        let files = collect(dir.path()).unwrap();
        let bytes = pack(dir.path(), &files).unwrap();
        assert_eq!(&bytes[..2], b"\x1f\x8b");
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(&bytes[..]));
        let paths: Vec<String> = archive
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(paths, vec!["src/main.py"]);
    }

    #[test]
    fn manifest_is_optional_and_parsed() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Manifest::load(dir.path()).unwrap().app.name.is_none());
        fs::write(
            dir.path().join("webshield.toml"),
            "[app]\nproject = \"shop\"\nname = \"web\"\n",
        )
        .unwrap();
        let m = Manifest::load(dir.path()).unwrap();
        assert_eq!(
            (m.app.project.as_deref(), m.app.name.as_deref()),
            (Some("shop"), Some("web"))
        );
    }
}

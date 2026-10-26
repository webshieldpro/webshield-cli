//! Functions (Workers): D1 and KV operations, data restore to a point in time.
//!
//! D1 and KV live inside the function nodes, so every operation runs on the server next to
//! them: the CLI packs the project directory (celld reads `wrangler.jsonc`, the
//! `migrations` directory and KV files from it), uploads it and waits for the result. The
//! storage of the function is never reachable from here.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{collect, pack, resolve, AppRef, Manifest};
use crate::api::models::apps::{
    App, FleetOperation, FunctionRecoveryWindow, FunctionRestore, OperationCreate, OperationGet,
    Operations, RestoreReq,
};
use crate::api::Client;
use crate::commands::prog_res::ProgramRes;
use crate::t;
use crate::util::input::confirm;
use crate::util::output::{info, success};
use anyhow::{bail, Context as _, Result};
use clap::Subcommand;
use reqwest::multipart::{Form, Part};

const POLL: Duration = Duration::from_secs(2);
/// Server-side limit on the SQL of one `d1 execute`.
const MAX_SQL_BYTES: usize = 100 * 1024;

#[derive(Subcommand)]
pub enum D1Command {
    #[command(about = t!(cmd_d1_execute))]
    Execute {
        #[command(flatten)]
        target: AppRef,
        #[arg(help = t!(arg_d1_database))]
        database: String,
        #[arg(long, conflicts_with = "file", required_unless_present = "file", help = t!(arg_d1_command))]
        command: Option<String>,
        #[arg(long, help = t!(arg_d1_file))]
        file: Option<PathBuf>,
    },
    #[command(about = t!(cmd_d1_migrate))]
    Migrate {
        #[command(flatten)]
        target: AppRef,
        #[arg(help = t!(arg_d1_database))]
        database: String,
    },
    #[command(about = t!(cmd_d1_migrations))]
    Migrations {
        #[command(flatten)]
        target: AppRef,
        #[arg(help = t!(arg_d1_database))]
        database: String,
    },
}

/// Function KV: bulk put and delete in the Wrangler file format.
pub enum KvBulk {
    Put,
    Delete,
}

/// The function named by flags or `webshield.toml`; an app here is a mistake, not a no-op.
async fn function(client: &Client<'_>, target: &AppRef) -> Result<App> {
    let app = resolve(client, target, &Manifest::load(&target.dir)?).await?;
    if !app.is_function() {
        bail!(t!(not_a_function, &app.name));
    }
    Ok(app)
}

/// Path of `file` inside the archive of `dir`: forward slashes, no way out of the directory.
/// The file must be packed — an ignored one would be missing on the server.
fn archive_path(dir: &Path, file: &Path, packed: &[PathBuf]) -> Result<String> {
    let full = if file.is_absolute() {
        file.to_path_buf()
    } else {
        dir.join(file)
    };
    let full = full
        .canonicalize()
        .with_context(|| file.display().to_string())?;
    let root = dir.canonicalize()?;
    let rel = full
        .strip_prefix(&root)
        .map_err(|_| anyhow::anyhow!(t!(file_outside_dir, &file.display().to_string())))?;
    let packed = packed
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .any(|p| p == full);
    if !packed {
        bail!(t!(file_not_packed, &file.display().to_string()));
    }
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Ok(parts.join("/"))
}

async fn run(
    client: &Client<'_>,
    app: &App,
    dir: &Path,
    kind: &str,
    target: &str,
    sql: Option<String>,
    file: Option<&Path>,
) -> Result<ProgramRes> {
    let files = collect(dir)?;
    let file = file.map(|f| archive_path(dir, f, &files)).transpose()?;
    let archive = pack(dir, &files)?;
    let mut form = Form::new()
        .text("kind", kind.to_string())
        .text("target", target.to_string())
        .part(
            "archive",
            Part::bytes(archive)
                .file_name("source.tar.gz")
                .mime_str("application/gzip")?,
        );
    if let Some(sql) = sql {
        form = form.text("sql", sql);
    }
    if let Some(file) = file {
        form = form.text("file", file);
    }
    let op = client
        .send_multipart::<OperationCreate>(app.id, form)
        .await?;
    wait(client, op).await
}

/// Waits for the operation, prints its output; a failed one is a non-zero exit.
async fn wait(client: &Client<'_>, op: FleetOperation) -> Result<ProgramRes> {
    info(t!(operation_started, &op.id.to_string()));
    let mut op = op;
    while op.in_progress() {
        tokio::time::sleep(POLL).await;
        op = client.send::<OperationGet>(op.id).await?;
    }
    if !op.output.is_empty() {
        print!("{}", op.output);
        if !op.output.ends_with('\n') {
            println!();
        }
    }
    if op.status != "done" {
        bail!(t!(operation_failed, &op.id.to_string(), &op.error));
    }
    success(t!(operation_done, &op.id.to_string()));
    Ok(ProgramRes::Idle)
}

pub async fn d1(client: &Client<'_>, cmd: D1Command) -> Result<ProgramRes> {
    match cmd {
        D1Command::Execute {
            target,
            database,
            command,
            file,
        } => {
            let sql = match (command, file) {
                (Some(sql), _) => sql,
                (None, Some(path)) => {
                    std::fs::read_to_string(&path).with_context(|| path.display().to_string())?
                }
                (None, None) => unreachable!("clap requires --command or --file"),
            };
            if sql.trim().is_empty() {
                bail!(t!(sql_empty));
            }
            if sql.len() > MAX_SQL_BYTES {
                bail!(t!(sql_too_large, &(MAX_SQL_BYTES / 1024).to_string()));
            }
            let app = function(client, &target).await?;
            run(
                client,
                &app,
                &target.dir,
                "d1_execute",
                &database,
                Some(sql),
                None,
            )
            .await
        }
        D1Command::Migrate { target, database } => {
            let app = function(client, &target).await?;
            run(
                client,
                &app,
                &target.dir,
                "d1_migrate",
                &database,
                None,
                None,
            )
            .await
        }
        D1Command::Migrations { target, database } => {
            let app = function(client, &target).await?;
            run(
                client,
                &app,
                &target.dir,
                "d1_migrations",
                &database,
                None,
                None,
            )
            .await
        }
    }
}

pub async fn kv_bulk(
    client: &Client<'_>,
    target: AppRef,
    namespace: String,
    file: PathBuf,
    op: KvBulk,
) -> Result<ProgramRes> {
    let app = function(client, &target).await?;
    let kind = match op {
        KvBulk::Put => "kv_put",
        KvBulk::Delete => "kv_delete",
    };
    run(
        client,
        &app,
        &target.dir,
        kind,
        &namespace,
        None,
        Some(&file),
    )
    .await
}

pub async fn window(client: &Client<'_>, target: AppRef) -> Result<ProgramRes> {
    let app = function(client, &target).await?;
    Ok(client.send::<FunctionRecoveryWindow>(app.id).await?.into())
}

pub async fn restore(
    client: &Client<'_>,
    target: AppRef,
    time: String,
    yes: bool,
) -> Result<ProgramRes> {
    // Момент — RFC 3339 с поясом: иначе непонятно, чьё это «15:00».
    if !super::data::has_zone(&time) {
        bail!(t!(restore_bad_time, &time));
    }
    let app = function(client, &target).await?;
    confirm(yes, &t!(confirm_restore_function, &app.name, &time))?;
    let op = client
        .send_json::<FunctionRestore>(RestoreReq { time }, app.id)
        .await?;
    wait(client, op).await
}

pub async fn operations(client: &Client<'_>, target: AppRef) -> Result<ProgramRes> {
    let app = function(client, &target).await?;
    Ok(client.send::<Operations>(app.id).await?.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn archive_path_stays_inside_and_packed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("data")).unwrap();
        fs::write(root.join("data/keys.json"), "[]").unwrap();
        fs::write(root.join(".gitignore"), "secret.json\n").unwrap();
        fs::write(root.join("secret.json"), "[]").unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("x.json"), "[]").unwrap();
        let packed = collect(root).unwrap();

        assert_eq!(
            archive_path(root, Path::new("data/keys.json"), &packed).unwrap(),
            "data/keys.json"
        );
        assert_eq!(
            archive_path(root, Path::new("./data/../data/keys.json"), &packed).unwrap(),
            "data/keys.json"
        );
        // Вне каталога, обход через «..», игнорируемый и несуществующий файл — отказ.
        for bad in [
            outside.path().join("x.json"),
            PathBuf::from("../x.json"),
            PathBuf::from("secret.json"),
            PathBuf::from("nope.json"),
            PathBuf::from("data"),
        ] {
            assert!(
                archive_path(root, &bad, &packed).is_err(),
                "{}",
                bad.display()
            );
        }
    }
}

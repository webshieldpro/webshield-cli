//! App variables and project data services: Postgres databases and Valkey stores.
//!
//! A variable may hold a reference such as `${{db.url}}` — the server substitutes the
//! connection string on rollout, so secrets of the database never pass through here.

use std::io::Read;
use std::path::PathBuf;

use super::{finish, resolve, AppRef, Manifest};
use crate::api::models::apps::{
    Database, DatabaseCreateReq, DatabaseRecoveryWindow, EnvGet, EnvPut, KeyValue,
    KeyValueCreateReq, ProjectResource, ProjectSpendCap, ProjectUsageGet, ProjectsByName, Redeploy,
    ResourceCreate, ResourceDelete, ResourceList, SpendCapReq,
};
use crate::api::Client;
use crate::commands::prog_res::ProgramRes;
use crate::t;
use crate::util::input::confirm;
use crate::util::output::{info, success};
use anyhow::{bail, Result};
use clap::{Args, Subcommand};

#[derive(Args, Clone, Default)]
pub struct ProjectRef {
    #[arg(long, help = t!(arg_app_project))]
    project: Option<String>,
    #[arg(long, help = t!(arg_app_dir), default_value = ".")]
    dir: PathBuf,
}

#[derive(Subcommand)]
pub enum EnvCommand {
    #[command(about = t!(cmd_env_list))]
    List {
        #[command(flatten)]
        target: AppRef,
        #[arg(long, help = t!(arg_env_values))]
        values: bool,
    },
    #[command(about = t!(cmd_env_set))]
    Set {
        #[command(flatten)]
        target: AppRef,
        #[arg(required = true, help = t!(arg_env_pairs))]
        pairs: Vec<String>,
        #[arg(long, help = t!(arg_env_redeploy))]
        redeploy: bool,
    },
    #[command(about = t!(cmd_env_unset))]
    Unset {
        #[command(flatten)]
        target: AppRef,
        #[arg(required = true, help = t!(arg_env_names))]
        names: Vec<String>,
        #[arg(long, help = t!(arg_env_redeploy))]
        redeploy: bool,
    },
}

#[derive(Subcommand)]
pub enum DbCommand {
    #[command(about = t!(cmd_db_list))]
    List {
        #[command(flatten)]
        target: ProjectRef,
    },
    #[command(about = t!(cmd_db_create))]
    Create {
        #[command(flatten)]
        target: ProjectRef,
        #[arg(help = t!(arg_resource_name))]
        name: String,
        #[arg(long, default_value_t = 1, help = t!(arg_db_size))]
        size: i64,
        #[arg(long, help = t!(arg_db_replica))]
        replica: bool,
    },
    #[command(about = t!(cmd_db_restore))]
    Restore {
        #[command(flatten)]
        target: ProjectRef,
        #[arg(help = t!(arg_db_source))]
        source: String,
        #[arg(long = "as", help = t!(arg_db_restore_as))]
        name: String,
        #[arg(long, help = t!(arg_db_restore_time))]
        time: Option<String>,
    },
    #[command(about = t!(cmd_db_window))]
    Window {
        #[command(flatten)]
        target: ProjectRef,
        #[arg(help = t!(arg_resource_name))]
        name: String,
    },
    #[command(about = t!(cmd_db_delete))]
    Delete {
        #[command(flatten)]
        target: ProjectRef,
        #[arg(help = t!(arg_resource_name))]
        name: String,
    },
}

#[derive(Subcommand)]
pub enum KvCommand {
    #[command(about = t!(cmd_kv_list))]
    List {
        #[command(flatten)]
        target: ProjectRef,
    },
    #[command(about = t!(cmd_kv_create))]
    Create {
        #[command(flatten)]
        target: ProjectRef,
        #[arg(help = t!(arg_resource_name))]
        name: String,
        #[arg(long, default_value_t = 64, help = t!(arg_kv_memory))]
        memory: i64,
        #[arg(long, help = t!(arg_kv_persistent))]
        persistent: bool,
    },
    #[command(about = t!(cmd_kv_delete))]
    Delete {
        #[command(flatten)]
        target: ProjectRef,
        #[arg(help = t!(arg_resource_name))]
        name: String,
    },
}

/// `NAME=value` pairs; a bare `NAME` takes its value from stdin, so a secret stays out of
/// the shell history and the process list.
fn parse_pairs(
    pairs: &[String],
    stdin: impl FnOnce() -> Result<String>,
) -> Result<Vec<(String, String)>> {
    let mut stdin = Some(stdin);
    let mut out = Vec::new();
    for pair in pairs {
        let (name, value) = match pair.split_once('=') {
            Some((name, value)) => (name.to_string(), value.to_string()),
            None => match stdin.take() {
                Some(read) => (pair.clone(), read()?),
                None => bail!(t!(env_stdin_once)),
            },
        };
        if name.is_empty() {
            bail!(t!(env_bad_pair, pair));
        }
        out.push((name, value));
    }
    Ok(out)
}

/// `YYYY-MM-DDTHH:MM[:SS[.frac]]` with `Z` or `±HH:MM`; the rest the server checks.
fn has_zone(time: &str) -> bool {
    let b = time.as_bytes();
    let date_ok = b.len() >= 16
        && b[..16].iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T' || *c == b't' || *c == b' ',
            13 => *c == b':',
            _ => c.is_ascii_digit(),
        });
    let zone_ok = time.ends_with(['Z', 'z'])
        || (b.len() >= 22 && {
            let z = &b[b.len() - 6..];
            matches!(z[0], b'+' | b'-')
                && z[1..3].iter().all(u8::is_ascii_digit)
                && z[3] == b':'
                && z[4..].iter().all(u8::is_ascii_digit)
        });
    date_ok && zone_ok && time.is_ascii()
}

fn read_stdin() -> Result<String> {
    let mut value = String::new();
    std::io::stdin().read_to_string(&mut value)?;
    // `echo "$SECRET" |` adds a newline that is not part of the value.
    let trimmed = value.strip_suffix('\n').unwrap_or(&value);
    Ok(trimmed.strip_suffix('\r').unwrap_or(trimmed).to_string())
}

pub async fn env(client: &Client<'_>, cmd: EnvCommand) -> Result<ProgramRes> {
    let (target, redeploy) = match &cmd {
        EnvCommand::List { target, .. } => (target, false),
        EnvCommand::Set {
            target, redeploy, ..
        }
        | EnvCommand::Unset {
            target, redeploy, ..
        } => (target, *redeploy),
    };
    let app = resolve(client, target, &Manifest::load(&target.dir)?).await?;
    let mut env = client.send::<EnvGet>(app.id).await?;
    match cmd {
        EnvCommand::List { values, .. } => {
            env.show_values = values;
            return Ok(ProgramRes::from(env));
        }
        EnvCommand::Set { pairs, .. } => {
            for (name, value) in parse_pairs(&pairs, read_stdin)? {
                env.variables.insert(name, value);
            }
        }
        EnvCommand::Unset { names, .. } => {
            for name in &names {
                if env.variables.remove(name).is_none() {
                    bail!(t!(env_missing, name));
                }
            }
        }
    }
    let env = client.send_json::<EnvPut>(env, app.id).await?;
    success(t!(env_saved, &env.variables.len().to_string()));
    match (redeploy, app.active_deployment) {
        (true, Some(active)) => {
            let dep = client.send::<Redeploy>(active.id).await?;
            finish(client, dep, false).await
        }
        (true, None) => bail!(t!(env_nothing_to_redeploy)),
        (false, _) => {
            info(t!(env_applies_next));
            Ok(ProgramRes::Idle)
        }
    }
}

/// Spending of the project this calendar month, by day.
pub async fn usage(client: &Client<'_>, target: ProjectRef) -> Result<ProgramRes> {
    let project = project_id(client, &target).await?;
    Ok(client.send::<ProjectUsageGet>(project).await?.into())
}

/// Sets (`AMOUNT`) or removes (`off`) the monthly spending cap of the project.
pub async fn cap(client: &Client<'_>, target: ProjectRef, amount: &str) -> Result<ProgramRes> {
    let spend_cap = parse_cap(amount)?;
    let project = project_id(client, &target).await?;
    let saved = client
        .send_json::<ProjectSpendCap>(SpendCapReq { spend_cap }, project)
        .await?;
    match &saved.spend_cap {
        Some(cap) => success(t!(cap_set, &saved.name, cap)),
        None => success(t!(cap_removed, &saved.name)),
    }
    match saved.paused.as_deref() {
        Some("cap") => info(t!(project_paused_cap)),
        Some("debt") => info(t!(project_paused_debt)),
        _ => {}
    }
    Ok(ProgramRes::Idle)
}

/// `off` (or `none`) — no cap; otherwise a non-negative amount with up to two decimals.
fn parse_cap(value: &str) -> Result<Option<String>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("off") || value.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let normal = value.replace(',', ".");
    let (whole, frac) = normal.split_once('.').unwrap_or((&normal, ""));
    let digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
    let valid = !whole.is_empty()
        && whole.len() <= 8
        && digits(whole)
        && frac.len() <= 2
        && digits(frac)
        && !(normal.contains('.') && frac.is_empty());
    if !valid {
        bail!(t!(cap_bad, value));
    }
    Ok(Some(normal))
}

/// Project id from `--project`, `webshield.toml` or the only project on the account.
async fn project_id(client: &Client<'_>, target: &ProjectRef) -> Result<i64> {
    let file = Manifest::load(&target.dir)?;
    let name = target.project.clone().or(file.app.project);
    let found = client.send::<ProjectsByName>(name.clone()).await?;
    match found.results.len() {
        1 => Ok(found.results[0].id),
        0 => bail!(t!(project_not_found, name.as_deref().unwrap_or("*"))),
        _ => bail!(t!(project_ambiguous)),
    }
}

async fn find<T: ProjectResource>(client: &Client<'_>, project: i64, name: &str) -> Result<i64> {
    let list = client.send::<ResourceList<T>>(project).await?;
    match T::entries(&list).into_iter().find(|(_, n)| n == name) {
        Some((id, _)) => Ok(id),
        None => bail!(t!(resource_not_found, name)),
    }
}

async fn delete<T: ProjectResource>(
    client: &Client<'_>,
    target: &ProjectRef,
    name: &str,
    yes: bool,
) -> Result<ProgramRes> {
    let project = project_id(client, target).await?;
    let id = find::<T>(client, project, name).await?;
    confirm(yes, &t!(confirm_delete_resource, name))?;
    client.send::<ResourceDelete<T>>(id).await?;
    success(t!(resource_deleting, name));
    Ok(ProgramRes::Idle)
}

pub async fn db(client: &Client<'_>, cmd: DbCommand, yes: bool) -> Result<ProgramRes> {
    match cmd {
        DbCommand::List { target } => {
            let project = project_id(client, &target).await?;
            Ok(client.send::<ResourceList<Database>>(project).await?.into())
        }
        DbCommand::Create {
            target,
            name,
            size,
            replica,
        } => {
            let req = DatabaseCreateReq {
                project: project_id(client, &target).await?,
                name,
                size_gb: Some(size),
                instances: if replica { 2 } else { 1 },
                restore_from: None,
                restore_time: None,
            };
            let db = client
                .send_json::<ResourceCreate<Database>>(req, ())
                .await?;
            info(t!(resource_created, &db.reference));
            Ok(db.into())
        }
        DbCommand::Restore {
            target,
            source,
            name,
            time,
        } => {
            let project = project_id(client, &target).await?;
            let source = find::<Database>(client, project, &source).await?;
            // Момент — RFC 3339 с поясом: иначе непонятно, чьё это «15:00».
            if let Some(time) = &time {
                if !has_zone(time) {
                    bail!(t!(restore_bad_time, time));
                }
            }
            let req = DatabaseCreateReq {
                project,
                name,
                size_gb: None,
                instances: 1,
                restore_from: Some(source),
                restore_time: time,
            };
            let db = client
                .send_json::<ResourceCreate<Database>>(req, ())
                .await?;
            info(t!(resource_created, &db.reference));
            Ok(db.into())
        }
        DbCommand::Window { target, name } => {
            let project = project_id(client, &target).await?;
            let id = find::<Database>(client, project, &name).await?;
            Ok(client.send::<DatabaseRecoveryWindow>(id).await?.into())
        }
        DbCommand::Delete { target, name } => delete::<Database>(client, &target, &name, yes).await,
    }
}

pub async fn kv(client: &Client<'_>, cmd: KvCommand, yes: bool) -> Result<ProgramRes> {
    match cmd {
        KvCommand::List { target } => {
            let project = project_id(client, &target).await?;
            Ok(client.send::<ResourceList<KeyValue>>(project).await?.into())
        }
        KvCommand::Create {
            target,
            name,
            memory,
            persistent,
        } => {
            let req = KeyValueCreateReq {
                project: project_id(client, &target).await?,
                name,
                memory_mb: memory,
                persistent,
            };
            let kv = client
                .send_json::<ResourceCreate<KeyValue>>(req, ())
                .await?;
            info(t!(resource_created, &kv.reference));
            Ok(kv.into())
        }
        KvCommand::Delete { target, name } => delete::<KeyValue>(client, &target, &name, yes).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(args: &[&str], stdin: &str) -> Result<Vec<(String, String)>> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let stdin = stdin.to_string();
        parse_pairs(&args, move || Ok(stdin))
    }

    #[test]
    fn pairs_split_on_first_equals_and_keep_the_rest_verbatim() {
        let got = pairs(&["A=1", "URL=${{db.url}}", "Q=a=b", "EMPTY="], "").unwrap();
        assert_eq!(
            got,
            vec![
                ("A".into(), "1".into()),
                ("URL".into(), "${{db.url}}".into()),
                ("Q".into(), "a=b".into()),
                ("EMPTY".into(), "".into()),
            ]
        );
    }

    #[test]
    fn cap_accepts_money_and_off_only() {
        assert_eq!(parse_cap("off").unwrap(), None);
        assert_eq!(parse_cap("NONE").unwrap(), None);
        assert_eq!(parse_cap("500").unwrap().as_deref(), Some("500"));
        assert_eq!(parse_cap("12,5").unwrap().as_deref(), Some("12.5"));
        assert_eq!(parse_cap(" 0.99 ").unwrap().as_deref(), Some("0.99"));
        for bad in [
            "",
            "-1",
            "1.",
            ".5",
            "1.234",
            "1e3",
            "NaN",
            "1;DROP",
            "１",
            "123456789",
            "1 000",
            "0x10",
        ] {
            assert!(parse_cap(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn restore_time_needs_a_zone() {
        for ok in [
            "2026-10-21T15:00:00+03:00",
            "2026-10-21T12:00:00Z",
            "2026-10-21T12:00:00.5-01:30",
        ] {
            assert!(has_zone(ok), "{ok}");
        }
        for bad in [
            "2026-10-21T15:00:00",
            "2026-10-21",
            "yesterday",
            "2026-10-21T15:00:00+0300",
            "'; DROP TABLE x; --",
            "2026-10-21T15:00:00+03:0\n",
            "２026-10-21T15:00:00Z",
        ] {
            assert!(!has_zone(bad), "{bad}");
        }
    }

    #[test]
    fn bare_name_reads_stdin_once() {
        assert_eq!(
            pairs(&["SECRET"], "s3cr\"et'\n").unwrap(),
            vec![("SECRET".into(), "s3cr\"et'\n".into())]
        );
        assert!(pairs(&["A", "B"], "x").is_err());
        assert!(pairs(&["=value"], "").is_err());
    }
}

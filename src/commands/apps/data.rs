//! App variables and project data services: Postgres databases and Valkey stores.
//!
//! A variable may hold a reference such as `${{db.url}}` — the server substitutes the
//! connection string on rollout, so secrets of the database never pass through here.

use std::io::Read;
use std::path::PathBuf;

use super::{finish, resolve, AppRef, Manifest};
use crate::api::models::apps::{
    Database, DatabaseCreateReq, EnvGet, EnvPut, KeyValue, KeyValueCreateReq, ProjectResource,
    ProjectsByName, Redeploy, ResourceCreate, ResourceDelete, ResourceList,
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

async fn delete<T: ProjectResource>(
    client: &Client<'_>,
    target: &ProjectRef,
    name: &str,
    yes: bool,
) -> Result<ProgramRes> {
    let project = project_id(client, target).await?;
    let list = client.send::<ResourceList<T>>(project).await?;
    let Some((id, _)) = T::entries(&list).into_iter().find(|(_, n)| n == name) else {
        bail!(t!(resource_not_found, name));
    };
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
                size_gb: size,
                instances: if replica { 2 } else { 1 },
            };
            let db = client
                .send_json::<ResourceCreate<Database>>(req, ())
                .await?;
            info(t!(resource_created, &db.reference));
            Ok(db.into())
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
    fn bare_name_reads_stdin_once() {
        assert_eq!(
            pairs(&["SECRET"], "s3cr\"et'\n").unwrap(),
            vec![("SECRET".into(), "s3cr\"et'\n".into())]
        );
        assert!(pairs(&["A", "B"], "x").is_err());
        assert!(pairs(&["=value"], "").is_err());
    }
}

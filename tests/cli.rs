//! Black-box smoke tests of the compiled `webshield` binary: argument parsing,
//! exit codes and top-level UX. API behaviour is covered by unit tests with a
//! mock HTTP server inside `src/`.

use assert_cmd::Command;
use predicates::prelude::*;

/// A command with a hermetic environment: English output, an empty config
/// directory (no developer profiles leaking in) and no ambient credentials.
fn webshield(config_home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("webshield").unwrap();
    cmd.env("WS_LANG", "en")
        .env("XDG_CONFIG_HOME", config_home)
        .env_remove("WS_TOKEN")
        .env_remove("WS_API_URL")
        .env_remove("WS_PROFILE");
    cmd
}

#[test]
fn help_lists_command_groups() {
    let dir = tempfile::tempdir().unwrap();
    webshield(dir.path())
        .arg("--help")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("domains")
                .and(predicate::str::contains("dns"))
                .and(predicate::str::contains("sites")),
        );
}

#[test]
fn version_prints_crate_version() {
    let dir = tempfile::tempdir().unwrap();
    webshield(dir.path())
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn missing_token_is_a_clean_error() {
    let dir = tempfile::tempdir().unwrap();
    webshield(dir.path())
        .args(["domains", "list"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("no token for profile"));
}

#[test]
fn unknown_subcommand_fails_with_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    // Exit code 2 is clap's usage-error convention.
    webshield(dir.path()).arg("frobnicate").assert().code(2);
}

#[test]
fn completion_emits_a_bash_script() {
    let dir = tempfile::tempdir().unwrap();
    webshield(dir.path())
        .args(["completion", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("webshield"));
}

#[test]
fn russian_locale_switches_runtime_messages() {
    let dir = tempfile::tempdir().unwrap();
    webshield(dir.path())
        .env("WS_LANG", "ru")
        .args(["domains", "list"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("не найден токен"));
}

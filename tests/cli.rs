use assert_cmd::cargo;
use clap::{crate_name, crate_version};
use predicates::prelude::*; // Used for writing assertions // Run programs

#[test]
fn version_flag_working() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = cargo::cargo_bin_cmd!("git-mirror");

    cmd.arg("--version");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "{} {}",
            crate_name!(),
            crate_version!()
        )));

    Ok(())
}

#[test]
fn help_flag_working() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = cargo::cargo_bin_cmd!("git-mirror");
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("git-mirror"));
    Ok(())
}

#[test]
fn missing_group_argument_fails() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = cargo::cargo_bin_cmd!("git-mirror");
    cmd.arg("--dry-run");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--group"));
    Ok(())
}

#[test]
fn unknown_flag_fails() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = cargo::cargo_bin_cmd!("git-mirror");
    cmd.args(["--group", "test", "--nonexistent-flag"])
        .arg("--dry-run");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument").or(predicate::str::contains("Found argument")));
    Ok(())
}

#[test]
fn dry_run_with_group_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = cargo::cargo_bin_cmd!("git-mirror");
    cmd.args([
        "--group",
        "testgroup",
        "--dry-run",
        "--provider=GitHub",
        "--url",
        "https://api.github.com",
    ]);
    cmd.assert().success();
    Ok(())
}

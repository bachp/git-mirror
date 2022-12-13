use assert_cmd::prelude::*; // Add methods on commands
use clap::{crate_name, crate_version};
use predicates::prelude::*; // Used for writing assertions
use std::process::Command; // Run programs

#[test]
fn version_flag_working() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("git-mirror")?;

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

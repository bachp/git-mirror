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

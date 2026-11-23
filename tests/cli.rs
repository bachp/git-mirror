use assert_cmd::cargo;
use clap::{crate_name, crate_version};
use fs2::FileExt;
use predicates::prelude::*; // Used for writing assertions // Run programs
use std::fs;
use std::path::{Path, PathBuf};

struct TempMirrorDir(PathBuf);

impl TempMirrorDir {
    fn new() -> Self {
        for id in 0.. {
            let dir =
                std::env::temp_dir().join(format!("git-mirror-cli-{}-{id}", std::process::id()));
            match fs::create_dir(&dir) {
                Ok(()) => return Self(dir),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("Unable to create temporary mirror directory: {e}"),
            }
        }
        unreachable!()
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempMirrorDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

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
fn no_lock_does_not_create_lockfile() {
    let dir = TempMirrorDir::new();
    let mut cmd = cargo::cargo_bin_cmd!("git-mirror");
    cmd.args(["--group", "test", "--url", "not-a-url", "--no-lock"])
        .arg("--mirror-dir")
        .arg(dir.path());

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Unable to get mirror repos"));
    assert!(!dir.path().join("git-mirror.lock").exists());
}

#[test]
fn no_lock_bypasses_an_existing_lock() {
    let dir = TempMirrorDir::new();
    let lockfile = fs::File::create(dir.path().join("git-mirror.lock")).unwrap();
    lockfile.try_lock_exclusive().unwrap();

    let mut locked = cargo::cargo_bin_cmd!("git-mirror");
    locked
        .args(["--group", "test", "--url", "not-a-url"])
        .arg("--mirror-dir")
        .arg(dir.path());
    locked.assert().failure().stderr(predicate::str::contains(
        "Another instance is already running",
    ));

    let mut unlocked = cargo::cargo_bin_cmd!("git-mirror");
    unlocked
        .args(["--group", "test", "--url", "not-a-url", "--no-lock"])
        .arg("--mirror-dir")
        .arg(dir.path());
    unlocked
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unable to get mirror repos"));
}

#[test]
fn invalid_shards_are_rejected_before_creating_a_cache() {
    let dir = TempMirrorDir::new();
    let cache = dir.path().join("mirror");

    for shard in ["1", "0/2", "1/0", "3/2", "1/2/3", "a/2", "-1/2"] {
        let mut cmd = cargo::cargo_bin_cmd!("git-mirror");
        cmd.args(["--group", "test"])
            .arg(format!("--shard={shard}"))
            .arg("--mirror-dir")
            .arg(&cache);

        cmd.assert()
            .code(2)
            .stderr(predicate::str::contains("expected INDEX/TOTAL"));
        assert!(!cache.exists());
    }
}

#[test]
fn shards_lock_the_configured_mirror_directory() {
    let dir = TempMirrorDir::new();
    let first = dir.path().join("cache-a");
    let second = dir.path().join("cache-b");
    fs::create_dir_all(&first).unwrap();
    let lockfile = fs::File::create(first.join("git-mirror.lock")).unwrap();
    lockfile.try_lock_exclusive().unwrap();

    let args = ["--group", "test", "--url", "not-a-url", "--shard", "2/2"];
    let mut locked = cargo::cargo_bin_cmd!("git-mirror");
    locked.args(args).arg("--mirror-dir").arg(&first);
    locked.assert().failure().stderr(predicate::str::contains(
        "Another instance is already running",
    ));

    let mut unlocked = cargo::cargo_bin_cmd!("git-mirror");
    unlocked.args(args).arg("--mirror-dir").arg(&second);
    unlocked
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unable to get mirror repos"));
    assert!(second.join("git-mirror.lock").is_file());
}

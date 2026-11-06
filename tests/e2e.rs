//! End-to-end test: a local git registry + file:// assets, driven through the CLI.

use assert_cmd::Command;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn target() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}

fn git(dir: &Path, args: &[&str]) {
    let st = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
        .args(args)
        .status()
        .unwrap();
    assert!(st.success(), "git {args:?}");
}

/// tar.gz with `hello-1.0.0/hello` (a shell script) inside a wrapper dir.
fn make_tarball(path: &Path, msg: &str) -> String {
    let script = format!("#!/bin/sh\necho {msg}\n");
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut b = tar::Builder::new(gz);
    let mut h = tar::Header::new_gnu();
    h.set_size(script.len() as u64);
    h.set_mode(0o755);
    h.set_cksum();
    b.append_data(&mut h, "hello-1.0.0/hello", script.as_bytes()).unwrap();
    let bytes = b.into_inner().unwrap().finish().unwrap();
    std::fs::write(path, &bytes).unwrap();
    hex::encode(Sha256::digest(&bytes))
}

fn manifest(name: &str, versions: &[(&str, &Path, &str)]) -> String {
    let mut s = format!(
        "name = \"{name}\"\ndescription = \"test package {name}\"\nrepo = \"https://example.com/{name}\"\nbin = [\"hello\"]\n"
    );
    for (v, url, sha) in versions {
        s += &format!(
            "\n[versions.\"{v}\".targets.{}]\nurl = \"file://{}\"\nsha256 = \"{sha}\"\n",
            target(),
            url.display()
        );
    }
    s
}

struct Env {
    _tmp: tempfile::TempDir,
    home: PathBuf,
    reg: PathBuf,
    assets: PathBuf,
}

impl Env {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let reg = tmp.path().join("registry");
        let assets = tmp.path().join("assets");
        std::fs::create_dir_all(reg.join("packages")).unwrap();
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(reg.join("hako-registry.toml"), "schema = 1\n").unwrap();
        git(&reg, &["init", "-q"]);
        Env { _tmp: tmp, home, reg, assets }
    }

    fn publish(&self, name: &str, body: &str) {
        std::fs::write(self.reg.join("packages").join(format!("{name}.toml")), body).unwrap();
        git(&self.reg, &["add", "-A"]);
        git(&self.reg, &["commit", "-q", "-m", name]);
    }

    fn hako(&self) -> Command {
        let mut c = Command::cargo_bin("hako").unwrap();
        c.env("HAKO_HOME", &self.home)
            .env("HAKO_REGISTRY", &self.reg)
            .env("PATH", std::env::var("PATH").unwrap());
        c
    }
}

fn stdout(c: &mut Command) -> String {
    let out = c.output().unwrap();
    assert!(
        out.status.success(),
        "command failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn install_run_upgrade_uninstall() {
    let env = Env::new();
    let v1 = env.assets.join("hello-1.0.0.tar.gz");
    let sha1 = make_tarball(&v1, "hello-v1");
    env.publish("hello", &manifest("hello", &[("1.0.0", &v1, &sha1)]));

    assert!(stdout(env.hako().arg("update")).contains("1 packages"));
    assert!(stdout(env.hako().args(["search", "hello"])).contains("hello"));
    assert!(stdout(env.hako().args(["info", "hello"])).contains("1.0.0"));

    assert!(stdout(env.hako().args(["install", "hello"])).contains("installed hello 1.0.0"));
    let bin = env.home.join("bin/hello");
    let out = std::process::Command::new(&bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello-v1");
    assert!(stdout(env.hako().arg("list")).contains("1.0.0"));
    assert!(stdout(env.hako().args(["install", "hello"])).contains("already installed"));

    // Publish 1.1.0 and upgrade.
    let v2 = env.assets.join("hello-1.1.0.tar.gz");
    let sha2 = make_tarball(&v2, "hello-v2");
    env.publish(
        "hello",
        &manifest("hello", &[("1.0.0", &v1, &sha1), ("1.1.0", &v2, &sha2)]),
    );
    assert!(stdout(env.hako().arg("upgrade")).contains("1.0.0 -> 1.1.0"));
    let out = std::process::Command::new(&bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello-v2");
    assert!(!env.home.join("pkgs/hello/1.0.0").exists());

    stdout(env.hako().args(["uninstall", "hello"]));
    assert!(bin.symlink_metadata().is_err());
    assert!(!env.home.join("pkgs/hello").exists());
    assert!(stdout(env.hako().arg("list")).contains("no packages installed"));
}

#[test]
fn checksum_mismatch_is_rejected() {
    let env = Env::new();
    let a = env.assets.join("bad.tar.gz");
    make_tarball(&a, "x");
    env.publish("bad", &manifest("bad", &[("1.0.0", &a, &"0".repeat(64))]));

    let out = env.hako().args(["install", "bad"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("checksum mismatch"));
    assert!(!env.home.join("bin/hello").exists());
    assert!(!env.home.join("pkgs/bad").exists());
}

#[test]
fn bin_conflicts_are_rejected() {
    let env = Env::new();
    let a = env.assets.join("a.tar.gz");
    let sha = make_tarball(&a, "a");
    env.publish("one", &manifest("one", &[("1.0.0", &a, &sha)]));
    env.publish("two", &manifest("two", &[("1.0.0", &a, &sha)]));

    stdout(env.hako().args(["install", "one"]));
    let out = env.hako().args(["install", "two"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already provided by package `one`"));
}

#[test]
fn unknown_schema_is_rejected() {
    let env = Env::new();
    std::fs::write(env.reg.join("hako-registry.toml"), "schema = 99\n").unwrap();
    env.publish("x", "");
    let out = env.hako().arg("update").output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("schema 99"));
}

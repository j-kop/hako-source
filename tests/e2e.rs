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
    make_tool(path, "hello", msg)
}

/// tar.gz with `pkg-1.0.0/<bin>`, a shell script that prints `msg`.
fn make_tool(path: &Path, bin: &str, msg: &str) -> String {
    let script = format!("#!/bin/sh\necho {msg}\n");
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut b = tar::Builder::new(gz);
    let mut h = tar::Header::new_gnu();
    h.set_size(script.len() as u64);
    h.set_mode(0o755);
    h.set_cksum();
    b.append_data(&mut h, format!("pkg-1.0.0/{bin}"), script.as_bytes())
        .unwrap();
    let bytes = b.into_inner().unwrap().finish().unwrap();
    std::fs::write(path, &bytes).unwrap();
    hex::encode(Sha256::digest(&bytes))
}

fn manifest(name: &str, versions: &[(&str, &Path, &str)]) -> String {
    manifest_bin(name, "hello", versions)
}

fn manifest_bin(name: &str, bin: &str, versions: &[(&str, &Path, &str)]) -> String {
    let mut s = format!(
        "name = \"{name}\"\ndescription = \"test package {name}\"\nrepo = \"https://example.com/{name}\"\nbin = [\"{bin}\"]\n"
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
        init_registry(&reg);
        Env { _tmp: tmp, home, reg, assets }
    }

    /// Create an additional, empty registry repo next to the default one.
    fn new_registry(&self, dir_name: &str) -> PathBuf {
        let reg = self._tmp.path().join(dir_name);
        init_registry(&reg);
        reg
    }

    fn publish(&self, name: &str, body: &str) {
        publish_to(&self.reg, name, body);
    }

    /// Build a tool tarball and publish its manifest to `reg`.
    fn publish_tool(&self, reg: &Path, name: &str, bin: &str, version: &str, msg: &str) {
        let asset = self.assets.join(format!("{name}-{version}-{msg}.tar.gz"));
        let sha = make_tool(&asset, bin, msg);
        publish_to(reg, name, &manifest_bin(name, bin, &[(version, &asset, &sha)]));
    }

    fn hako(&self) -> Command {
        let mut c = Command::cargo_bin("hako").unwrap();
        c.env("HAKO_HOME", &self.home)
            .env("HAKO_REGISTRY", &self.reg)
            .env("PATH", std::env::var("PATH").unwrap());
        c
    }
}

fn init_registry(reg: &Path) {
    std::fs::create_dir_all(reg.join("packages")).unwrap();
    std::fs::write(reg.join("hako-registry.toml"), "schema = 1\n").unwrap();
    git(reg, &["init", "-q"]);
}

fn publish_to(reg: &Path, name: &str, body: &str) {
    std::fs::write(reg.join("packages").join(format!("{name}.toml")), body).unwrap();
    git(reg, &["add", "-A"]);
    git(reg, &["commit", "-q", "-m", name]);
}

/// Run a command that is expected to fail; return its stderr.
fn failure(c: &mut Command) -> String {
    let out = c.output().unwrap();
    assert!(
        !out.status.success(),
        "command unexpectedly succeeded:\nstdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn run_bin(path: &Path) -> String {
    let out = std::process::Command::new(path).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
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

#[test]
fn two_registries() {
    let env = Env::new();
    let team_b = env.new_registry("team-b");
    env.publish_tool(&env.reg, "alpha", "alpha", "1.0.0", "alpha-default");
    env.publish_tool(&env.reg, "shared", "shared", "1.0.0", "shared-default");
    env.publish_tool(&team_b, "beta", "beta", "1.0.0", "beta-v1");
    env.publish_tool(&team_b, "shared", "shared", "1.0.0", "shared-team-b");

    let team_b_url = team_b.to_string_lossy().into_owned();
    assert!(stdout(env.hako().args(["registry", "add", "team-b", &team_b_url]))
        .contains("added registry team-b (2 packages)"));
    let list = stdout(env.hako().args(["registry", "list"]));
    assert!(list.contains("default") && list.contains("team-b"), "{list}");

    // Search covers both registries; `shared` shows up once per registry.
    let found = stdout(env.hako().arg("search"));
    assert_eq!(found.lines().filter(|l| l.starts_with("shared ")).count(), 2, "{found}");
    assert!(found.contains("beta") && found.contains("team-b"), "{found}");

    // Unambiguous names install from whichever registry has them.
    assert!(stdout(env.hako().args(["install", "alpha", "beta"])).contains("from team-b"));
    assert_eq!(run_bin(&env.home.join("bin/alpha")), "alpha-default");
    assert_eq!(run_bin(&env.home.join("bin/beta")), "beta-v1");

    // Ambiguous names must be qualified.
    let err = failure(env.hako().args(["install", "shared"]));
    assert!(err.contains("default/shared, team-b/shared"), "{err}");
    stdout(env.hako().args(["install", "team-b/shared"]));
    assert_eq!(run_bin(&env.home.join("bin/shared")), "shared-team-b");

    // Once installed, the unqualified name resolves to where it came from...
    assert!(stdout(env.hako().args(["install", "shared"])).contains("already installed"));
    assert!(stdout(env.hako().args(["info", "shared"])).contains("registry:    team-b"));
    // ...and switching registries requires an explicit uninstall.
    let err = failure(env.hako().args(["install", "default/shared"]));
    assert!(err.contains("already installed from registry `team-b`"), "{err}");

    let listed = stdout(env.hako().arg("list"));
    assert!(listed.lines().any(|l| l.starts_with("alpha") && l.contains("default")), "{listed}");
    assert!(listed.lines().any(|l| l.starts_with("beta") && l.contains("team-b")), "{listed}");

    // Upgrade pulls each package from its own registry.
    env.publish_tool(&team_b, "beta", "beta", "2.0.0", "beta-v2");
    let out = stdout(env.hako().arg("upgrade"));
    assert!(out.contains("upgraded beta 1.0.0 -> 2.0.0"), "{out}");
    assert_eq!(run_bin(&env.home.join("bin/beta")), "beta-v2");

    // Removing a registry keeps its packages installed but stops upgrades for them.
    let out = stdout(env.hako().args(["registry", "remove", "team-b"]));
    assert!(out.contains("beta, shared"), "{out}");
    assert!(!env.home.join("registries/team-b").exists());
    assert!(!stdout(env.hako().args(["registry", "list"])).contains("team-b"));
    let out = env.hako().arg("upgrade").output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("registry `team-b` is no longer configured"));
    assert_eq!(run_bin(&env.home.join("bin/beta")), "beta-v2");
}

#[test]
fn bin_conflicts_across_registries() {
    let env = Env::new();
    let team_b = env.new_registry("team-b");
    env.publish_tool(&env.reg, "deploy-a", "deploy", "1.0.0", "a");
    env.publish_tool(&team_b, "deploy-b", "deploy", "1.0.0", "b");
    stdout(env.hako().args(["registry", "add", "team-b", &team_b.to_string_lossy()]));

    stdout(env.hako().args(["install", "deploy-a"]));
    let err = failure(env.hako().args(["install", "deploy-b"]));
    assert!(
        err.contains("`deploy` is already provided by package `deploy-a` (from registry `default`)"),
        "{err}"
    );
    assert_eq!(run_bin(&env.home.join("bin/deploy")), "a");
}

#[test]
fn bad_registry_url_is_not_saved() {
    let env = Env::new();
    let missing = env.home.join("does-not-exist");
    failure(env.hako().args(["registry", "add", "broken", &missing.to_string_lossy()]));
    assert!(!stdout(env.hako().args(["registry", "list"])).contains("broken"));
    failure(env.hako().args(["registry", "add", "Bad/Name", &env.reg.to_string_lossy()]));
}

#[test]
fn migrates_single_registry_setup() {
    // Simulate a hako 0.1 home: `registry = "<url>"` in config + a clone in `registry/`.
    let env = Env::new();
    env.publish_tool(&env.reg, "alpha", "alpha", "1.0.0", "alpha");
    std::fs::create_dir_all(&env.home).unwrap();
    std::fs::write(
        env.home.join("config.toml"),
        format!("registry = {:?}\n", env.reg.to_string_lossy()),
    )
    .unwrap();
    std::process::Command::new("git")
        .args(["clone", "-q"])
        .arg(&env.reg)
        .arg(env.home.join("registry"))
        .status()
        .unwrap();
    std::fs::write(
        env.home.join("installed.toml"),
        "[packages.old]\nversion = \"1.0.0\"\ntarget = \"x\"\nbins = []\n",
    )
    .unwrap();

    let mut c = Command::cargo_bin("hako").unwrap();
    c.env("HAKO_HOME", &env.home).env_remove("HAKO_REGISTRY");
    assert!(stdout(c.args(["registry", "list"])).contains("default"));
    assert!(env.home.join("registries/default/.git").exists());
    assert!(!env.home.join("registry").exists());

    let mut c = Command::cargo_bin("hako").unwrap();
    c.env("HAKO_HOME", &env.home).env_remove("HAKO_REGISTRY");
    let listed = stdout(c.arg("list"));
    assert!(listed.lines().any(|l| l.starts_with("old") && l.contains("default")), "{listed}");
}

/// tar.gz with `hako-<version>/hako` = a copy of the real hako binary under test.
fn package_hako(path: &Path, version: &str) -> String {
    let exe = std::fs::read(assert_cmd::cargo::cargo_bin("hako")).unwrap();
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut b = tar::Builder::new(gz);
    let mut h = tar::Header::new_gnu();
    h.set_size(exe.len() as u64);
    h.set_mode(0o755);
    h.set_cksum();
    b.append_data(&mut h, format!("hako-{version}/hako"), exe.as_slice())
        .unwrap();
    let bytes = b.into_inner().unwrap().finish().unwrap();
    std::fs::write(path, &bytes).unwrap();
    hex::encode(Sha256::digest(&bytes))
}

#[test]
fn hako_upgrades_itself() {
    let env = Env::new();
    let v1 = env.assets.join("hako-1.0.0.tar.gz");
    let sha1 = package_hako(&v1, "1.0.0");
    env.publish("hako", &manifest_bin("hako", "hako", &[("1.0.0", &v1, &sha1)]));

    // Bootstrap: the cargo-built hako installs hako (what install.sh does).
    stdout(env.hako().args(["install", "hako"]));
    let installed = env.home.join("bin/hako");
    let self_hako = || {
        let mut c = Command::new(&installed);
        c.env("HAKO_HOME", &env.home).env("HAKO_REGISTRY", &env.reg);
        c
    };
    assert!(stdout(self_hako().arg("list")).contains("hako"));

    // Publish a new version, then let the *installed* hako replace itself while running.
    let v2 = env.assets.join("hako-1.1.0.tar.gz");
    let sha2 = package_hako(&v2, "1.1.0");
    env.publish(
        "hako",
        &manifest_bin("hako", "hako", &[("1.0.0", &v1, &sha1), ("1.1.0", &v2, &sha2)]),
    );
    let out = stdout(self_hako().arg("upgrade"));
    assert!(out.contains("upgraded hako 1.0.0 -> 1.1.0"), "{out}");
    assert!(!env.home.join("pkgs/hako/1.0.0").exists());
    assert!(std::fs::read_link(&installed)
        .unwrap()
        .ends_with("pkgs/hako/1.1.0/hako"));

    // The new copy works, and hako can uninstall itself too.
    assert!(stdout(self_hako().arg("--version")).starts_with("hako "));
    assert!(stdout(self_hako().arg("upgrade")).contains("everything is up to date"));
    stdout(self_hako().args(["uninstall", "hako"]));
    assert!(installed.symlink_metadata().is_err());
}

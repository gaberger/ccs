//! Gate for ADR-2610271000 §2: `ccs upgrade`. The pure rules are tested
//! directly; the rest upgrades a copy of the real binary, with a fake `curl`
//! that serves release archives built here.

use ccs::domain::{checksum_for, newer_version, parse_command, release_asset, release_target, Command, VERSION};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

// ── Pure rules ───────────────────────────────────────────────────────────────

#[test]
fn versions_compare_by_number_not_by_text() {
    assert!(newer_version("0.10.0", "0.9.9"));
    assert!(newer_version("v1.0.0", "0.9.9"), "a leading v is ignored");
    assert!(newer_version("0.1.1", "0.1.0"));
    assert!(!newer_version("0.1.0", "0.1.0"));
    assert!(!newer_version("0.0.9", "0.1.0"));
    assert!(!newer_version("garbage", "0.1.0"), "what cannot be read is never newer");
}

#[test]
fn each_platform_has_one_archive_name() {
    assert_eq!(release_target("macos", "aarch64"), Some("aarch64-apple-darwin"));
    assert_eq!(release_target("macos", "x86_64"), Some("x86_64-apple-darwin"));
    assert_eq!(release_target("linux", "x86_64"), Some("x86_64-unknown-linux-gnu"));
    assert_eq!(release_target("linux", "aarch64"), None, "not built");
    assert_eq!(release_target("windows", "x86_64"), None);
    assert_eq!(release_asset("0.2.0", "aarch64-apple-darwin"), "ccs-0.2.0-aarch64-apple-darwin.tar.gz");
}

#[test]
fn a_checksum_is_read_for_one_file_only() {
    let sums = "aaa  ccs-0.2.0-x86_64-unknown-linux-gnu.tar.gz\nbbb *ccs-0.2.0-aarch64-apple-darwin.tar.gz\n";
    assert_eq!(checksum_for(sums, "ccs-0.2.0-aarch64-apple-darwin.tar.gz").as_deref(), Some("bbb"), "binary-mode * too");
    assert_eq!(checksum_for(sums, "ccs-0.2.0-x86_64-unknown-linux-gnu.tar.gz").as_deref(), Some("aaa"));
    assert_eq!(checksum_for(sums, "ccs-0.2.0-x86_64-apple-darwin.tar.gz"), None);
    assert_eq!(checksum_for(sums, "0.2.0-aarch64-apple-darwin.tar.gz"), None, "the whole name, not a suffix");
}

#[test]
fn upgrade_and_version_parse() {
    let p = |a: &[&str]| parse_command(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>(), |_| false).unwrap();
    assert_eq!(p(&["upgrade"]), Command::Upgrade { version: None, check: false });
    assert_eq!(p(&["upgrade", "--check"]), Command::Upgrade { version: None, check: true });
    assert_eq!(p(&["upgrade", "0.2.0"]), Command::Upgrade { version: Some("0.2.0".into()), check: false });
    assert_eq!(p(&["version"]), Command::Version);
    assert_eq!(p(&["--version"]), Command::Version);
}

// ── End to end ───────────────────────────────────────────────────────────────

static N: AtomicUsize = AtomicUsize::new(0);

/// This machine's archive name for `version`.
fn asset(version: &str) -> String {
    release_asset(version, release_target(std::env::consts::OS, std::env::consts::ARCH).expect("a built platform"))
}

struct Env {
    root: PathBuf,
}

impl Env {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ccs-upgrade-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&root);
        for d in ["bin", "home", "rel", "build", "tmp"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        // The binary under test is a copy, so the real one is never replaced.
        fs::copy(env!("CARGO_BIN_EXE_ccs"), root.join("bin/ccs")).unwrap();
        // A fake curl, as `ccs upgrade` calls it (no gh, no login; ADR-2610281600):
        //   -o /dev/null -w %{url_effective} .../releases/latest  -> prints .../releases/tag/<rel/latest>
        //   -o FILE .../releases/download/TAG/ASSET                -> copies rel/TAG/ASSET to FILE, else exit 22
        script(
            &root.join("curl"),
            r#"out=""; url=""
while [ $# -gt 0 ]; do case "$1" in -o) out=$2; shift 2 ;; -w) shift 2 ;; -*) shift ;; *) url=$1; shift ;; esac; done
case "$url" in
*/releases/latest) printf '%s' "${url%/latest}/tag/$(cat "$FAKE_REL/latest")" ;;
*/releases/download/*)
  rest=${url#*/releases/download/}; tag=${rest%%/*}; name=${rest#*/}
  [ -f "$FAKE_REL/$tag/$name" ] || { echo "curl: (22) 404 $url" >&2; exit 22; }
  cp "$FAKE_REL/$tag/$name" "$out" ;;
*) echo "fake curl: $url" >&2; exit 2 ;;
esac"#,
        );
        Env { root }
    }
    fn bin(&self) -> PathBuf {
        self.root.join("bin/ccs")
    }
    /// Publish release `version` whose binary prints `prints`, with a right or wrong checksum.
    fn publish(&self, version: &str, prints: &str, good_sum: bool) {
        let dir = self.root.join("rel").join(format!("v{version}"));
        fs::create_dir_all(&dir).unwrap();
        let build = self.root.join("build").join(version);
        fs::create_dir_all(&build).unwrap();
        script(&build.join("ccs"), prints);
        let name = asset(version);
        let ok = Proc::new("tar").args(["czf", dir.join(&name).to_str().unwrap(), "-C", build.to_str().unwrap(), "ccs"]).status().unwrap();
        assert!(ok.success());
        let sum = if good_sum { sha256(&dir.join(&name)) } else { "0".repeat(64) };
        fs::write(dir.join("SHA256SUMS.txt"), format!("{sum}  {name}\n")).unwrap();
    }
    fn latest(&self, version: &str) {
        fs::write(self.root.join("rel/latest"), format!("v{version}\n")).unwrap();
    }
    fn run_with(&self, curl: &Path, a: &[&str]) -> Output {
        Proc::new(self.bin())
            .args(a)
            .env("HOME", self.root.join("home"))
            .env("CCS_HOME", self.root.join("home/.claude-profiles"))
            .env("CCS_CURL_BIN", curl)
            .env("FAKE_REL", self.root.join("rel"))
            .env("TMPDIR", self.root.join("tmp"))
            .output()
            .unwrap()
    }
    fn run(&self, a: &[&str]) -> Output {
        self.run_with(&self.root.join("curl"), a)
    }
    fn says(&self, a: &[&str]) -> (bool, String) {
        let out = self.run(a);
        (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
    }
    /// What `ccs version` prints now.
    /// Nothing left in the temp folder: no download folder, kept or half-made.
    fn tmp_is_empty(&self) -> bool {
        fs::read_dir(self.root.join("tmp")).unwrap().next().is_none()
    }
    fn version(&self) -> String {
        String::from_utf8(self.run(&["version"]).stdout).unwrap().trim().to_string()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn sha256(p: &Path) -> String {
    let out = Proc::new("shasum").args(["-a", "256", p.to_str().unwrap()]).output().or_else(|_| Proc::new("sha256sum").arg(p).output()).unwrap();
    String::from_utf8(out.stdout).unwrap().split_whitespace().next().unwrap().to_string()
}

#[test]
fn version_prints_the_running_version() {
    let e = Env::new();
    assert_eq!(e.version(), format!("ccs {VERSION}"));
}

#[test]
fn upgrade_installs_a_newer_release_over_the_running_binary() {
    let e = Env::new();
    e.publish("9.9.9", "echo 'ccs 9.9.9'", true);
    e.latest("9.9.9");
    let (ok, out) = e.says(&["upgrade"]);
    assert!(ok, "{out}");
    assert!(out.contains(VERSION) && out.contains("9.9.9"), "{out}");
    assert_eq!(e.version(), "ccs 9.9.9", "the copy was replaced");
    assert!(fs::read_dir(e.root.join("bin")).unwrap().count() == 1, "no temporary file left beside it");
    assert!(fs::read_dir(e.root.join("tmp")).unwrap().next().is_none(), "the download folder is removed");
}

#[test]
fn check_says_what_is_available_and_changes_nothing() {
    let e = Env::new();
    e.publish("9.9.9", "echo 'ccs 9.9.9'", true);
    e.latest("9.9.9");
    let (ok, out) = e.says(&["upgrade", "--check"]);
    assert!(ok && out.contains("9.9.9") && out.contains("ccs upgrade"), "{out}");
    assert_eq!(e.version(), format!("ccs {VERSION}"));
}

#[test]
fn the_latest_release_when_already_installed_is_up_to_date() {
    let e = Env::new();
    e.latest(VERSION);
    let (ok, out) = e.says(&["upgrade"]);
    assert!(ok && out.contains("up to date"), "{out}");
    assert_eq!(e.version(), format!("ccs {VERSION}"));
}

#[test]
fn an_older_latest_release_is_not_a_downgrade() {
    let e = Env::new();
    e.publish("0.0.1", "echo 'ccs 0.0.1'", true);
    e.latest("0.0.1");
    let (ok, out) = e.says(&["upgrade"]);
    assert!(ok && out.contains("up to date"), "{out}");
    assert_eq!(e.version(), format!("ccs {VERSION}"));
}

#[test]
fn a_named_version_installs_even_when_older() {
    let e = Env::new();
    e.publish("0.0.9", "echo 'ccs 0.0.9'", true);
    e.latest(VERSION);
    let (ok, out) = e.says(&["upgrade", "0.0.9"]);
    assert!(ok, "{out}");
    assert_eq!(e.version(), "ccs 0.0.9");
}

#[test]
fn a_bad_checksum_installs_nothing() {
    let e = Env::new();
    e.publish("9.9.9", "echo 'ccs 9.9.9'", false);
    e.latest("9.9.9");
    let (ok, out) = e.says(&["upgrade"]);
    assert!(!ok && out.contains("checksum"), "{out}");
    assert_eq!(e.version(), format!("ccs {VERSION}"));
    assert!(e.tmp_is_empty(), "a failed upgrade removes its download folder");
}

#[test]
fn a_release_without_this_platform_says_so() {
    let e = Env::new();
    fs::create_dir_all(e.root.join("rel/v9.9.9")).unwrap();
    fs::write(e.root.join("rel/v9.9.9/SHA256SUMS.txt"), "").unwrap();
    e.latest("9.9.9");
    let (ok, out) = e.says(&["upgrade"]);
    assert!(!ok && out.contains(&asset("9.9.9")), "{out}");
    assert_eq!(e.version(), format!("ccs {VERSION}"));
    assert!(e.tmp_is_empty(), "a failed upgrade removes its download folder");
}

#[test]
fn a_new_binary_that_cannot_run_is_not_installed() {
    let e = Env::new();
    e.publish("9.9.9", "exit 3", true);
    e.latest("9.9.9");
    let (ok, out) = e.says(&["upgrade"]);
    assert!(!ok && out.contains("9.9.9"), "{out}");
    assert_eq!(e.version(), format!("ccs {VERSION}"));
    assert!(e.tmp_is_empty(), "a failed upgrade removes its download folder");
}

#[test]
fn a_new_binary_that_names_another_version_is_not_installed() {
    let e = Env::new();
    e.publish("9.9.9", "echo 'ccs 9.9.8'", true);
    e.latest("9.9.9");
    let (ok, out) = e.says(&["upgrade"]);
    assert!(!ok && out.contains("9.9.8"), "{out}");
    assert_eq!(e.version(), format!("ccs {VERSION}"));
    assert!(e.tmp_is_empty(), "a failed upgrade removes its download folder");
}

#[test]
fn a_new_binary_whose_version_only_starts_the_same_is_not_installed() {
    let e = Env::new();
    e.publish("9.9.9", "echo 'ccs 9.9.90'", true);
    e.latest("9.9.9");
    let (ok, out) = e.says(&["upgrade"]);
    assert!(!ok && out.contains("9.9.90"), "{out}");
    assert_eq!(e.version(), format!("ccs {VERSION}"));
    assert!(e.tmp_is_empty(), "a failed upgrade removes its download folder");
}

#[test]
fn upgrade_needs_curl_and_nothing_else() {
    let e = Env::new();
    let out = e.run_with(Path::new("/nonexistent/curl"), &["upgrade"]);
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && text.contains("curl") && !text.contains("gh"), "{text}");
    assert_eq!(e.version(), format!("ccs {VERSION}"));
}

#[test]
fn upgrade_never_runs_gh() {
    // A gh that records being called: the upgrade must not touch it.
    let e = Env::new();
    let marker = e.root.join("gh-was-called");
    script(&e.root.join("bin/gh"), &format!("touch '{}'; exit 1", marker.display()));
    e.publish("9.9.9", "echo 'ccs 9.9.9'", true);
    e.latest("9.9.9");
    let out = Proc::new(e.bin())
        .arg("upgrade")
        .env("HOME", e.root.join("home"))
        .env("CCS_HOME", e.root.join("home/.claude-profiles"))
        .env("CCS_CURL_BIN", e.root.join("curl"))
        .env("FAKE_REL", e.root.join("rel"))
        .env("TMPDIR", e.root.join("tmp"))
        .env("PATH", format!("{}:{}", e.root.join("bin").display(), std::env::var("PATH").unwrap()))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!marker.exists(), "gh was run");
}

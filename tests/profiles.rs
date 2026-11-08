//! Gate for profiles on disk (ADR-2610261212, ADR-2610261643): create, clone, list, doctor, pin.
//! Every test works in its own temp directory and never touches the real ~/.claude.

use ccs::domain::{CcsError, Finding, ProfileName};
use ccs::usecases;
use ccs::ports::ProfileStore as _;
use std::fs;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

/// A fresh sandbox: `profiles/` is CCS_HOME, `dotclaude/` stands in for ~/.claude.
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ccs-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("dotclaude")).unwrap();
        Sandbox { root }
    }
    fn home(&self) -> PathBuf {
        self.root.join("profiles")
    }
    fn store(&self) -> impl ccs::ports::ProfileStore {
        ccs::profile_store(self.home())
    }
    /// `doctor` for a profile, with this sandbox's kits, library and old layout.
    fn doctor(&self, store: &dyn ccs::ports::ProfileStore, n: &str, claude: bool) -> Vec<Finding> {
        let (kits, lib, old) = (ccs::kit_store(&self.home()), ccs::library(&self.home()), ccs::legacy(&self.home(), self.root.join("dotclaude")));
        usecases::doctor(store, &kits, &lib, &old, &name(n), claude).unwrap()
    }
    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let p = self.root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, text).unwrap();
        p
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn name(s: &str) -> ProfileName {
    ProfileName::parse(s).unwrap()
}

#[test]
fn a_new_profile_has_an_env_file_and_a_config_dir() {
    let sb = Sandbox::new();
    let store = sb.store();
    usecases::new_profile(&store, &name("work"), None).unwrap();

    let dir = sb.home().join("work");
    assert!(dir.join("profile.env").is_file());
    assert_eq!(fs::read_dir(dir.join("config")).unwrap().count(), 0, "a login starts with nothing in it");

    let p = usecases::load(&store, &name("work")).unwrap();
    assert_eq!(p.config_dir, dir.join("config"));
    assert!(p.env.is_empty(), "the template is all comments");
}

#[test]
fn creating_an_existing_profile_is_refused() {
    let sb = Sandbox::new();
    let store = sb.store();
    usecases::new_profile(&store, &name("work"), None).unwrap();
    let err = usecases::new_profile(&store, &name("work"), None).unwrap_err();
    assert_eq!(err, CcsError::Exists("work".into()));
}

#[test]
fn cloning_copies_variables_kits_and_settings_never_login_or_items() {
    let sb = Sandbox::new();
    let store = sb.store();
    usecases::new_profile(&store, &name("work"), None).unwrap();
    sb.write("profiles/work/profile.env", "CLAUDE_CODE_USE_BEDROCK=1\nAWS_REGION=us-east-1\n");
    sb.write("profiles/work/kits", "net\n");
    sb.write("profiles/work/config/settings.json", "{\"model\":\"x\"}");
    sb.write("profiles/work/config/skills/own/SKILL.md", "o");
    sb.write("profiles/work/config/.claude.json", "{\"secret\":\"login\"}");

    usecases::new_profile(&store, &name("prod"), Some(&name("work"))).unwrap();

    let cfg = sb.home().join("prod/config");
    assert_eq!(usecases::load(&store, &name("prod")).unwrap().env.len(), 2);
    assert_eq!(store.kits(&name("prod")), ["net"]);
    assert_eq!(fs::read_to_string(cfg.join("settings.json")).unwrap(), "{\"model\":\"x\"}");
    assert!(!cfg.join("skills").exists(), "items travel in kits, not in config dirs");
    assert!(!cfg.join(".claude.json").exists(), "login state must never be cloned");
}
#[test]
fn cloning_from_a_missing_profile_fails_and_creates_nothing() {
    let sb = Sandbox::new();
    let store = sb.store();
    let err = usecases::new_profile(&store, &name("prod"), Some(&name("ghost"))).unwrap_err();
    assert_eq!(err, CcsError::NoProfile("ghost".into()));
    assert!(!sb.home().join("prod").exists());
}

#[test]
fn a_failed_create_keeps_what_the_folder_held_before() {
    // A folder with no profile.env is not a profile, but it can hold a login.
    let sb = Sandbox::new();
    let login = sb.write("profiles/work/config/.claude.json", "{}");
    let store = sb.store();
    assert!(store.create(&name("work"), Some(&name("ghost"))).is_err(), "ghost has no profile.env to copy");
    assert!(login.is_file(), "the login that was there before is kept");
    assert!(!sb.home().join("work/profile.env").exists(), "what this call wrote is removed");
}

#[test]
fn a_failed_create_of_a_new_folder_leaves_nothing() {
    let sb = Sandbox::new();
    let store = sb.store();
    assert!(store.create(&name("fresh"), Some(&name("ghost"))).is_err());
    assert!(!sb.home().join("fresh").exists());
}

#[test]
fn list_marks_current_and_default_and_names_kits() {
    let sb = Sandbox::new();
    sb.write("profiles/library/skills/x/SKILL.md", "x");
    sb.write("profiles/kits/net/uses", "");
    let store = sb.store();
    usecases::new_profile(&store, &name("lab"), None).unwrap();
    usecases::new_profile(&store, &name("work"), None).unwrap();
    usecases::set_default(&store, &name("lab")).unwrap();
    store.set_kits(&name("work"), &["net".into()]).unwrap();

    let rows = usecases::list(&store, Some("work"));
    let names: Vec<_> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["lab", "work"], "library/ and kits/ are not profiles");
    assert!(rows[0].is_default && !rows[0].current && rows[0].kits.is_empty());
    assert!(rows[1].current && !rows[1].is_default && rows[1].kits == ["net"]);
}
#[test]
fn set_default_requires_an_existing_profile() {
    let sb = Sandbox::new();
    let store = sb.store();
    assert_eq!(usecases::set_default(&store, &name("nope")), Err(CcsError::NoProfile("nope".into())));
    usecases::new_profile(&store, &name("work"), None).unwrap();
    usecases::set_default(&store, &name("work")).unwrap();
    assert_eq!(usecases::default_name(&store), Some("work".into()));
}

#[test]
fn a_bad_profile_env_names_the_file_line() {
    let sb = Sandbox::new();
    let store = sb.store();
    usecases::new_profile(&store, &name("work"), None).unwrap();
    sb.write("profiles/work/profile.env", "A=1\noops\n");
    assert_eq!(usecases::load(&store, &name("work")).unwrap_err(), CcsError::EnvSyntax { line: 2 });
}

#[test]
fn the_pin_file_is_found_from_a_subdirectory() {
    let sb = Sandbox::new();
    sb.write("repo/.claude-profile", "  work\n");
    let deep = sb.root.join("repo/src/deep");
    fs::create_dir_all(&deep).unwrap();
    assert_eq!(ccs::pin_locator(deep).pinned(), Some("work".into()));
    assert_eq!(ccs::pin_locator(sb.root.join("dotclaude")).pinned(), None);
}

#[test]
fn doctor_fails_when_claude_is_not_installed() {
    let sb = Sandbox::new();
    let store = sb.store();
    usecases::new_profile(&store, &name("work"), None).unwrap();
    let found = sb.doctor(&store, "work", false);
    assert!(found.iter().any(|f| matches!(f, Finding::Fail(m) if m.contains("claude"))), "{found:?}");
}

use ccs::ports::PinLocator as _;

// ── Status line and AWS import against the store ────────────────────────────

use ccs::domain::ImportOutcome;
use ccs::ports::{AwsProfiles, CommandRunner};

#[test]
fn a_fresh_profile_can_be_given_the_ccs_statusline_once() {
    let sb = Sandbox::new();
    let store = sb.store();
    usecases::new_profile(&store, &name("work"), None).unwrap();
    assert!(usecases::seed_statusline(&store, &name("work"), "/bin/ccs").unwrap());
    let path = sb.home().join("work/config/settings.json");
    assert!(fs::read_to_string(&path).unwrap().contains("'/bin/ccs' statusline"));

    // An existing settings.json is never touched.
    fs::write(&path, "{\"mine\":true}").unwrap();
    assert!(!usecases::seed_statusline(&store, &name("work"), "/bin/ccs").unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), "{\"mine\":true}");
}

struct FakeRunner(Option<&'static str>);
impl CommandRunner for FakeRunner {
    fn run(&self, argv: &[String], stdin: &[u8]) -> Option<String> {
        assert_eq!(argv, ["their.sh"]);
        assert_eq!(stdin, b"{\"model\":{}}", "stdin must pass through unchanged");
        self.0.map(String::from)
    }
}

#[test]
fn statusline_wraps_the_existing_command_with_the_same_stdin() {
    let vars = vec![("CCS_PROFILE".to_string(), "w".to_string())];
    let wrap = vec!["their.sh".to_string()];
    let out = usecases::statusline(&FakeRunner(Some("opus · ~/x\n")), &vars, b"{\"model\":{}}", &wrap);
    assert_eq!(out, "⬡ w · anthropic │ opus · ~/x");
    let failed = usecases::statusline(&FakeRunner(None), &vars, b"{\"model\":{}}", &wrap);
    assert_eq!(failed, "⬡ w · anthropic │ ⚠ statusline failed: their.sh");
}

struct FakeAws;
impl AwsProfiles for FakeAws {
    fn names(&self) -> Result<Vec<String>, CcsError> {
        Ok(vec!["bed".into(), "default".into(), "noregion".into(), "bad/name".into()])
    }
    fn region(&self, profile: &str) -> Option<String> {
        match profile {
            "bed" => Some("us-west-2".into()),
            "default" => Some("us-east-1".into()),
            _ => None,
        }
    }
}

#[test]
fn listing_aws_profiles_changes_nothing_and_shows_what_is_imported() {
    let sb = Sandbox::new();
    let store = sb.store();
    usecases::import_aws(&store, &FakeAws, &["bed".into()], false, None, "/bin/ccs").unwrap();

    let rows = usecases::aws_status(&store, &FakeAws).unwrap();
    let view: Vec<_> = rows.iter().map(|r| (r.aws.as_str(), r.imported_as.as_deref())).collect();
    assert_eq!(view, [("bed", Some("bed")), ("default", None), ("noregion", None), ("bad/name", None)]);
    assert_eq!(store.names().len(), 1);
}

#[test]
fn importing_named_aws_profiles_makes_bedrock_profiles() {
    let sb = Sandbox::new();
    let store = sb.store();
    let out = usecases::import_aws(&store, &FakeAws, &["bed".into(), "default".into()], false, None, "/bin/ccs").unwrap();
    assert_eq!(
        out,
        [
            ImportOutcome::Created { name: "bed".into(), region: Some("us-west-2".into()) },
            ImportOutcome::Created { name: "aws-default".into(), region: Some("us-east-1".into()) },
        ]
    );
    let p = usecases::load(&store, &name("aws-default")).unwrap();
    assert!(p.env.contains(&("AWS_PROFILE".into(), "default".into())));
    assert!(p.env.contains(&("CLAUDE_CODE_USE_BEDROCK".into(), "1".into())));
    assert!(fs::read_to_string(sb.home().join("bed/config/settings.json")).unwrap().contains("statusline"));
}

#[test]
fn the_region_flag_fills_only_a_missing_region() {
    let sb = Sandbox::new();
    let store = sb.store();
    usecases::import_aws(&store, &FakeAws, &["bed".into(), "noregion".into()], false, Some("eu-west-1"), "/bin/ccs").unwrap();
    let region = |n: &str| {
        usecases::load(&store, &name(n)).unwrap().env.into_iter().find(|(k, _)| k == "AWS_REGION").map(|(_, v)| v)
    };
    assert_eq!(region("bed").as_deref(), Some("us-west-2"), "the profile's own region wins");
    assert_eq!(region("noregion").as_deref(), Some("eu-west-1"));
}

#[test]
fn a_profile_without_any_region_is_made_but_fails_doctor() {
    let sb = Sandbox::new();
    let store = sb.store();
    let out = usecases::import_aws(&store, &FakeAws, &["noregion".into()], false, None, "/bin/ccs").unwrap();
    assert_eq!(out, [ImportOutcome::Created { name: "noregion".into(), region: None }]);
    let found = sb.doctor(&store, "noregion", true);
    assert!(found.iter().any(|f| matches!(f, Finding::Fail(m) if m.contains("AWS_REGION"))));
}

#[test]
fn existing_unknown_and_unusable_profiles_are_skipped_not_overwritten() {
    let sb = Sandbox::new();
    let store = sb.store();
    usecases::new_profile(&store, &name("bed"), None).unwrap();
    sb.write("profiles/bed/profile.env", "MINE=1\n");

    let out = usecases::import_aws(&store, &FakeAws, &["bed".into(), "ghost".into(), "bad/name".into()], false, None, "/bin/ccs").unwrap();

    assert!(out.iter().all(|o| matches!(o, ImportOutcome::Skipped { .. })), "{out:?}");
    assert_eq!(fs::read_to_string(sb.home().join("bed/profile.env")).unwrap(), "MINE=1\n");
    assert_eq!(store.names().len(), 1);
}

#[test]
fn all_imports_every_usable_aws_profile() {
    let sb = Sandbox::new();
    let store = sb.store();
    usecases::import_aws(&store, &FakeAws, &[], true, None, "/bin/ccs").unwrap();
    let names: Vec<_> = store.names().iter().map(|n| n.as_str().to_string()).collect();
    assert_eq!(names, ["aws-default", "bed", "noregion"]);
}

#[test]
fn importing_nothing_without_all_is_a_usage_error() {
    let sb = Sandbox::new();
    let store = sb.store();
    let err = usecases::import_aws(&store, &FakeAws, &[], false, None, "/bin/ccs").unwrap_err();
    assert!(matches!(err, CcsError::Usage(_)));
}



/// `t` in the TUI (ADR-2610261643 §14): a shell in the profile's folder, with
/// the variables a launch sets and none leaking from the caller.
#[test]
fn a_shell_plan_starts_in_the_profile_folder_with_its_variables() {
    let sb = Sandbox::new();
    let store = sb.store();
    usecases::new_profile(&store, &name("work"), None).unwrap();
    fs::write(sb.home().join("work/profile.env"), "AWS_PROFILE=sso\n").unwrap();
    let plan = usecases::shell_plan(&store, &name("work")).unwrap();
    assert_eq!(plan.dir.as_deref(), Some(sb.home().join("work").as_path()));
    assert!(plan.set.contains(&("AWS_PROFILE".into(), "sso".into())));
    assert!(plan.set.contains(&("CCS_PROFILE".into(), "work".into())));
    assert!(plan.set.iter().any(|(k, v)| k == "CLAUDE_CONFIG_DIR" && v.ends_with("work/config")));
    assert!(plan.unset.contains(&"CLAUDE_CODE_USE_BEDROCK".to_string()));
    assert!(plan.args.is_empty(), "a shell gets no claude arguments");
    assert!(matches!(usecases::shell_plan(&store, &name("ghost")), Err(CcsError::NoProfile(_))));
}

/// A real store whose `write_env` or `write_settings_if_absent` fails, to stop an import halfway.
/// `only` limits the failure to one profile; `None` fails them all.
struct Failing<S> {
    inner: S,
    env: bool,
    settings: bool,
    only: Option<&'static str>,
}
impl<S> Failing<S> {
    fn hits(&self, n: &ProfileName) -> bool { self.only.is_none_or(|o| o == n.as_str()) }
}
impl<S: ccs::ports::ProfileStore> ccs::ports::ProfileStore for Failing<S> {
    fn names(&self) -> Vec<ProfileName> { self.inner.names() }
    fn exists(&self, n: &ProfileName) -> bool { self.inner.exists(n) }
    fn load(&self, n: &ProfileName) -> Result<ccs::domain::Profile, CcsError> { self.inner.load(n) }
    fn create(&self, n: &ProfileName, from: Option<&ProfileName>) -> Result<(), CcsError> { self.inner.create(n, from) }
    fn discard(&self, n: &ProfileName) -> Result<(), CcsError> { self.inner.discard(n) }
    fn default_name(&self) -> Option<String> { self.inner.default_name() }
    fn set_default(&self, n: &ProfileName) -> Result<(), CcsError> { self.inner.set_default(n) }
    fn clear_default(&self) -> Result<(), CcsError> { self.inner.clear_default() }
    fn folder(&self, n: &ProfileName) -> PathBuf { self.inner.folder(n) }
    fn env_file(&self, n: &ProfileName) -> PathBuf { self.inner.env_file(n) }
    fn write_env(&self, n: &ProfileName, text: &str) -> Result<(), CcsError> {
        if self.env && self.hits(n) { Err(CcsError::Io("disk full".into())) } else { self.inner.write_env(n, text) }
    }
    fn write_settings_if_absent(&self, n: &ProfileName, text: &str) -> Result<bool, CcsError> {
        if self.settings && self.hits(n) { Err(CcsError::Io("disk full".into())) } else { self.inner.write_settings_if_absent(n, text) }
    }
    fn read_kits(&self, n: &ProfileName) -> Result<Vec<String>, CcsError> { self.inner.read_kits(n) }
    fn set_kits(&self, n: &ProfileName, kits: &[String]) -> Result<(), CcsError> { self.inner.set_kits(n, kits) }
    fn permissions(&self, n: &ProfileName) -> Result<ccs::domain::Permissions, CcsError> { self.inner.permissions(n) }
    fn set_permissions(&self, n: &ProfileName, p: &ccs::domain::Permissions) -> Result<(), CcsError> { self.inner.set_permissions(n, p) }
}

#[test]
fn an_import_that_fails_halfway_leaves_no_profile_and_can_run_again() {
    for (env, settings) in [(true, false), (false, true)] {
        let sb = Sandbox::new();
        // A login already in the folder is not the import's to remove.
        sb.write("profiles/bed/config/.claude.json", "{}");
        let broken = Failing { inner: sb.store(), env, settings, only: None };
        let out = usecases::import_aws(&broken, &FakeAws, &["bed".into()], false, None, "/bin/ccs").unwrap();
        assert!(matches!(&out[..], [ImportOutcome::Skipped { aws, reason }] if aws == "bed" && reason.contains("disk full")));
        assert!(!broken.exists(&name("bed")), "a half-made import must not count as a profile (env={env})");
        assert!(sb.home().join("bed/config/.claude.json").is_file(), "the login stays");

        let store = sb.store();
        let out = usecases::import_aws(&store, &FakeAws, &["bed".into()], false, None, "/bin/ccs").unwrap();
        assert_eq!(out, [ImportOutcome::Created { name: "bed".into(), region: Some("us-west-2".into()) }]);
        assert!(usecases::load(&store, &name("bed")).unwrap().env.contains(&("AWS_PROFILE".into(), "bed".into())));
        assert!(fs::read_to_string(sb.home().join("bed/config/settings.json")).unwrap().contains("statusline"));
    }
}

/// One profile failing does not lose the ones the same run already made.
#[test]
fn an_import_that_fails_for_one_profile_still_reports_the_others() {
    let sb = Sandbox::new();
    let broken = Failing { inner: sb.store(), env: true, settings: false, only: Some("aws-default") };
    let wanted = ["bed".into(), "default".into(), "noregion".into()];
    let out = usecases::import_aws(&broken, &FakeAws, &wanted, false, None, "/bin/ccs").unwrap();
    assert_eq!(out[0], ImportOutcome::Created { name: "bed".into(), region: Some("us-west-2".into()) });
    assert!(matches!(&out[1], ImportOutcome::Skipped { aws, reason } if aws == "default" && reason.contains("disk full")));
    assert_eq!(out[2], ImportOutcome::Created { name: "noregion".into(), region: None });
    assert!(broken.exists(&name("bed")) && broken.exists(&name("noregion")));
    assert!(!broken.exists(&name("aws-default")), "the failed one is undone");
}

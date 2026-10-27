//! ccs — Claude Code profile switcher (ADR-2610261212).
//!
//! `lib.rs` is the **composition root**: the only place allowed to name a
//! concrete adapter, read configuration from the environment, or print.
//! Adapters touch the environment only for their own mechanics (`PATH`,
//! `temp_dir`, `current_exe`).
//!
//! ```text
//!   domain  ←  ports  ←  usecases
//!                ↑
//!           adapters (secondary): FsProfileStore, FsPinLocator, ExecLauncher,
//!                                 ProcessRunner, AwsCli, FsPathInput, ProcessEditor,
//!                                 FsLibrary, FsKitStore, FsLegacy, GitFetcher, GhReleases, FsText
//!                ↑
//!        lib.rs — wires them, once
//! ```
//!
//! Check it with `hexa analyze .`.

pub mod adapters;
pub mod domain;
pub mod ports;
pub mod usecases;

use adapters::secondary::{
    aws_cli::AwsCli, exec_launcher::ExecLauncher, fs_kits::FsKitStore, fs_legacy::FsLegacy, fs_library::FsLibrary,
    fs_paths::FsPathInput, fs_pins::FsPinLocator, fs_store::FsProfileStore, fs_text::FsText, gh_releases::GhReleases, git_fetcher::GitFetcher,
    process_editor::ProcessEditor, process_runner::ProcessRunner,
};
use adapters::primary::tui;
use domain::{
    library_tree, parse_command, CcsError, Command, Finding, Kind, KitCmd, LibraryCmd, ProfileName, HELP,
};
use ports::{Launcher, ProfileStore};
use std::io::{IsTerminal, Read};
use std::path::PathBuf;

/// Profiles under `home` (`$CCS_HOME`).
pub fn profile_store(home: PathBuf) -> impl ports::ProfileStore {
    FsProfileStore::new(home)
}

/// The layout earlier versions left in `home`, and the stock `claude_home` (read only).
pub fn legacy(home: &std::path::Path, claude_home: PathBuf) -> impl ports::Legacy {
    FsLegacy::new(home.to_path_buf(), claude_home, home.join(".trash"))
}

/// The library under `$CCS_HOME/library`; removed entries go to `trash`.
pub fn library(home: &std::path::Path) -> impl ports::Library {
    FsLibrary::new(home.join("library"), home.join(".trash"))
}

/// Kits under `$CCS_HOME/kits`, built into `$CCS_HOME/.build`.
pub fn kit_store(home: &std::path::Path) -> impl ports::KitStore {
    FsKitStore::new(home.join("kits"), home.join(".build"), home.join(".trash"))
}

/// Paths typed into the TUI: `~` is `home`, relative paths start in `cwd`.
pub fn path_input(home: PathBuf, cwd: PathBuf) -> impl ports::PathInput {
    FsPathInput::new(home, cwd)
}

/// `git`, or `$CCS_GIT_BIN`.
pub fn fetcher() -> impl ports::Fetcher {
    GitFetcher::new(var("CCS_GIT_BIN").unwrap_or_else(|| "git".into()))
}

/// The editor `ccs … edit` and the TUI's `e` open: `$VISUAL`, else `$EDITOR`, else `vi`.
pub fn editor() -> impl ports::Editor {
    ProcessEditor::new(&var("VISUAL").or_else(|| var("EDITOR")).unwrap_or_else(|| "vi".into()))
}

/// Looks for `.claude-profile` in `start` and its parents.
pub fn pin_locator(start: PathBuf) -> impl ports::PinLocator {
    FsPinLocator::new(start)
}

/// Run `ccs` with the arguments after the program name. Returns the exit code.
pub fn run(args: Vec<String>) -> i32 {
    match dispatch(args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("ccs: {e}");
            1
        }
    }
}

fn var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

/// This binary's own path, for the status line command written into a profile.
fn ccs_bin() -> String {
    std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "ccs".into())
}

fn dispatch(args: Vec<String>) -> Result<i32, CcsError> {
    let user_home = var("HOME").map(PathBuf::from).ok_or_else(|| CcsError::Io("HOME is not set".into()))?;
    let ccs_home = var("CCS_HOME").map(PathBuf::from).unwrap_or_else(|| user_home.join(".claude-profiles"));
    let lib = library(&ccs_home);
    let kits = kit_store(&ccs_home);
    let old = legacy(&ccs_home, user_home.join(".claude"));
    let store = profile_store(ccs_home);
    let cwd = std::env::current_dir().map_err(|e| CcsError::Io(format!("cannot read working directory: {e}")))?;
    let pins = pin_locator(cwd);
    let launcher = ExecLauncher::new(var("CCS_CLAUDE_BIN").unwrap_or_else(|| "claude".into()));
    let resolve = |explicit: Option<String>| usecases::resolve_name(&store, &pins, explicit, var("CCS_PROFILE"));

    let hint_migrate = || {
        if usecases::needs_migrate(&old) {
            eprintln!("ccs: an older layout is here (shared/, native, or ccs links in a profile); run: ccs migrate");
        }
    };
    let paths = path_input(user_home.clone(), std::env::current_dir().map_err(|e| CcsError::Io(format!("current dir: {e}")))?);
    let ed = editor();
    let aws = AwsCli::new(var("CCS_AWS_BIN").unwrap_or_else(|| "aws".into()));
    let fetch = fetcher();
    let claude_available = launcher.available();
    let app = |current: Option<String>| usecases::App {
        store: &store,
        kits: &kits,
        lib: &lib,
        legacy: &old,
        paths: &paths,
        editor: &ed,
        aws: &aws,
        fetch: &fetch,
        ccs_bin: ccs_bin(),
        claude_available,
        current,
    };
    // Home and its pages (ADR-2610261643 §18); `args` go to claude on a launch.
    let ui = |current: Option<String>, args: Vec<String>| -> Result<i32, CcsError> {
        if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
            return Err(CcsError::Usage("ccs ui needs a terminal".into()));
        }
        match tui::run(&app(current)).map_err(|e| CcsError::Io(format!("terminal: {e}")))? {
            tui::Exit::Quit => Ok(0),
            // The terminal is restored by now; launch exactly as `ccs <name>` does.
            tui::Exit::Launch(n) => {
                let name = usecases::existing(&store, &n)?;
                launch(&usecases::prepare_launch(&store, &kits, &lib, &name, None, args, &ccs_bin())?, &launcher)
            }
            tui::Exit::LaunchWith { profile, kits: picked } => {
                let name = usecases::existing(&store, &profile)?;
                launch(&usecases::prepare_launch(&store, &kits, &lib, &name, Some(picked), args, &ccs_bin())?, &launcher)
            }
            tui::Exit::Shell(n) => {
                let plan = usecases::shell_plan(&store, &usecases::existing(&store, &n)?)?;
                let shell = var("SHELL").unwrap_or_else(|| "/bin/sh".into());
                eprintln!("ccs: {shell} in {}, with profile {n}'s variables; exit returns. ccs {n} launches with its kits.", plan.dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default());
                launch(&(plan, vec![]), &ExecLauncher::new(shell))
            }
        }
    };

    let command = parse_command(&args, |s| ProfileName::parse(s).is_ok_and(|n| usecases::existing(&store, n.as_str()).is_ok()))?;
    match command {
        Command::Launch { profile, kits: chosen, args } => {
            // A bare `ccs` on a terminal opens Home, the start screen (ADR-2610261643 §18);
            // with nothing resolved it never falls back to ~/.claude (§5).
            if profile.is_none() && chosen.is_none() && std::io::stdout().is_terminal() && std::io::stdin().is_terminal() {
                hint_migrate();
                return ui(resolve(None).ok().map(|n| n.as_str().to_string()), args);
            }
            let name = resolve(profile)?;
            launch(&usecases::prepare_launch(&store, &kits, &lib, &name, chosen, args, &ccs_bin())?, &launcher)
        }
        Command::List => {
            let current = resolve(None).ok();
            let rows = usecases::list(&store, current.as_ref().map(ProfileName::as_str));
            hint_migrate();
            if rows.is_empty() {
                println!("No profiles yet. Make one with: ccs new <name>, or ccs import aws");
            }
            for r in rows {
                let mark = if r.current { "* " } else { "  " };
                let def = if r.is_default { " (default)" } else { "" };
                let with: String = r.kits.iter().map(|k| format!(" +{k}")).collect();
                println!("{mark}{}{with}{def}", r.name.as_str());
            }
            Ok(0)
        }
        Command::New { name, from } => {
            let from = from.map(|f| usecases::existing(&store, &f)).transpose()?;
            let name = usecases::make_profile(&store, &name, from.as_ref(), &ccs_bin())?;
            let n = name.as_str();
            println!("Created profile '{n}'. Edit its variables in profile.env, then run: ccs {n}");
            println!("MCP servers belong to the profile. Add one with: ccs {n} mcp add -s user <name> -- <command>");
            Ok(0)
        }
        Command::Import => {
            for n in usecases::import(&lib, &kits, &old)? {
                println!("  {n}");
            }
            println!("~/.claude is in the library and in kit imported; nothing in ~/.claude changed.");
            println!("Use it with: ccs <profile> +imported, or ccs kit attach imported <profile>");
            Ok(0)
        }
        Command::Migrate => {
            let notes = usecases::migrate(&store, &lib, &kits, &old)?;
            if notes.is_empty() {
                println!("Nothing to migrate.");
            }
            for n in notes {
                println!("  {n}");
            }
            Ok(0)
        }
        Command::Default(None) => {
            if let Some(d) = usecases::default_name(&store) {
                println!("{d}");
            }
            Ok(0)
        }
        Command::Default(Some(n)) => {
            usecases::set_default(&store, &usecases::existing(&store, &n)?)?;
            Ok(0)
        }
        Command::Env(n) => {
            print!("{}", usecases::exports(&store, &resolve(n)?)?);
            Ok(0)
        }
        Command::Doctor(n) => {
            let name = resolve(n)?;
            println!("Profile: {}", name.as_str());
            let mut failed = false;
            for f in usecases::doctor(&store, &kits, &lib, &old, &name, launcher.available())? {
                match f {
                    Finding::Pass(m) => println!("  ✓ {m}"),
                    Finding::Warn(m) => println!("  ! {m}"),
                    Finding::Fail(m) => {
                        failed = true;
                        println!("  ✗ {m}");
                    }
                }
            }
            Ok(i32::from(failed))
        }
        Command::Which => {
            println!("{}", resolve(None)?.as_str());
            Ok(0)
        }
        Command::Version => {
            println!("ccs {}", domain::VERSION);
            Ok(0)
        }
        Command::Upgrade { version, check } => {
            let rel = GhReleases::new(var("CCS_GH_BIN").unwrap_or_else(|| "gh".into()), var("CCS_REPO").unwrap_or_else(|| "gaberger/ccs".into()));
            let target = domain::release_target(std::env::consts::OS, std::env::consts::ARCH);
            match usecases::upgrade(&rel, domain::VERSION, target, version.as_deref(), check)? {
                usecases::Upgraded::UpToDate(v) => println!("ccs {v} is up to date"),
                usecases::Upgraded::Available { current, latest } => println!("ccs {latest} is available (this is {current}); install it with: ccs upgrade"),
                usecases::Upgraded::Installed { from, to, at } => println!("✓ ccs {from} → {to} ({})", at.display()),
            }
            Ok(0)
        }
        Command::StatuslineAuto => {
            let mut stdin = Vec::new();
            if !std::io::stdin().is_terminal() {
                let _ = std::io::stdin().read_to_end(&mut stdin);
            }
            let vars: Vec<(String, String)> = std::env::vars().collect();
            println!("{}", usecases::statusline_auto(&ProcessRunner, &FsText, &vars, &stdin));
            Ok(0)
        }
        Command::Statusline { wrap } => {
            let mut stdin = Vec::new();
            if !std::io::stdin().is_terminal() {
                // A status line with no payload still shows the segment.
                let _ = std::io::stdin().read_to_end(&mut stdin);
            }
            let vars: Vec<(String, String)> = std::env::vars().collect();
            println!("{}", usecases::statusline(&ProcessRunner, &vars, &stdin, &wrap));
            Ok(0)
        }
        Command::ImportAws { profiles, all, region } => {
            if profiles.is_empty() && !all {
                println!("AWS CLI profiles (import with: ccs import aws <name>… or --all):");
                for row in usecases::aws_status(&store, &aws)? {
                    match row.imported_as {
                        Some(n) => println!("  {} → {n}", row.aws),
                        None => println!("  {}", row.aws),
                    }
                }
                return Ok(0);
            }
            for o in usecases::import_aws(&store, &aws, &profiles, all, region.as_deref(), &ccs_bin())? {
                println!("  {o}");
            }
            Ok(0)
        }
        Command::Ui => {
            hint_migrate();
            ui(resolve(None).ok().map(|n| n.as_str().to_string()), vec![])
        }
        Command::Library(cmd) => library_command(&lib, &kits, &editor(), &fetch, cmd),
        Command::Kit(cmd) => kit_command(&store, &kits, &lib, &editor(), cmd),
        Command::Help => {
            print!("{HELP}");
            Ok(0)
        }
    }
}

/// Print what the library no longer has, then become `claude`. Returns only on failure.
fn launch(prepared: &(domain::LaunchPlan, Vec<String>), launcher: &dyn Launcher) -> Result<i32, CcsError> {
    for w in &prepared.1 {
        eprintln!("ccs: {w}");
    }
    Err(usecases::launch(launcher, &prepared.0))
}

/// `ccs library …` (ADR-2610261643).
fn library_command(
    lib: &dyn ports::Library,
    kits: &dyn ports::KitStore,
    editor: &dyn ports::Editor,
    fetch: &dyn ports::Fetcher,
    cmd: LibraryCmd,
) -> Result<i32, CcsError> {
    match cmd {
        LibraryCmd::List(only) => {
            for kind in [Kind::Skill, Kind::Agent, Kind::Plugin].into_iter().filter(|k| only.is_none_or(|o| o == *k)) {
                let (items, folders) = usecases::library_list(lib, kind);
                println!("{}", kind.dir());
                if items.is_empty() && folders.is_empty() {
                    match kind {
                        Kind::Plugin => println!("  (none; add with: ccs library add <owner/repo | git URL | path>)"),
                        _ => println!("  (none; add with: ccs library add <path> [--in <folder>])"),
                    }
                }
                let rows = library_tree(&items, &folders);
                let width = rows.iter().filter(|r| r.item.is_some()).map(|r| 2 * r.depth + r.leaf().len()).max().unwrap_or(0);
                for r in &rows {
                    let indent = "  ".repeat(r.depth + 1);
                    match &r.item {
                        None => println!("{indent}{}/", r.leaf()),
                        Some(i) => {
                            let pad = width.saturating_sub(2 * r.depth);
                            println!("{indent}{:pad$}  {}", r.leaf(), shorten(i.description.as_deref().unwrap_or(""), 70));
                        }
                    }
                }
            }
        }
        LibraryCmd::Add { source, folder } => {
            let (kind, name) = usecases::library_add_source(lib, fetch, &source, std::path::Path::new(&source), &folder)?;
            let at = if folder.is_empty() { String::new() } else { format!(" in {folder}/") };
            println!("✓ added {}/{name}{at}", kind.dir());
        }
        LibraryCmd::Mkdir(kind, folder) => {
            usecases::library_mkdir(lib, kind, &folder)?;
            println!("✓ made {}/{folder}/", kind.dir());
        }
        LibraryCmd::Move { what, to } => {
            usecases::library_move(lib, kits, &what, &to)?;
            let to = if to.is_empty() { "the top".to_string() } else { format!("{to}/") };
            println!("✓ moved {} to {to}; kits follow it", what.line());
        }
        LibraryCmd::Rename { folder, to } => {
            if let domain::Use::Folder(kind, f) = &folder {
                usecases::library_rename_folder(lib, kits, *kind, f, &to)?;
            }
            println!("✓ renamed {} to {to}; kits follow it", folder.line());
        }
        LibraryCmd::Remove(what) => {
            let r = usecases::library_remove(lib, kits, &what)?;
            println!("✓ moved {} to {}", what.line(), r.trashed.display());
            if !r.kits.is_empty() {
                println!("  kits that used it: {}; they leave it out from the next launch", r.kits.join(", "));
            }
        }
        LibraryCmd::Show(what) => print!("{}", usecases::library_show(lib, &what)?),
        LibraryCmd::Update(which) => {
            for line in usecases::plugin_update(lib, fetch, which.as_ref())? {
                println!("  {line}");
            }
        }
        LibraryCmd::Edit(what) => {
            usecases::library_edit(lib, editor, &what)?;
            println!("✓ edited {}; every kit using it gets the change", what.line());
        }
    }
    Ok(0)
}

/// `ccs kit …` (ADR-2610261643).
fn kit_command(
    store: &dyn ProfileStore,
    kits: &dyn ports::KitStore,
    lib: &dyn ports::Library,
    editor: &dyn ports::Editor,
    cmd: KitCmd,
) -> Result<i32, CcsError> {
    match cmd {
        KitCmd::List => {
            let names = kits.names();
            if names.is_empty() {
                println!("No kits yet. Make one with: ccs kit new <name>");
            }
            for k in names {
                println!("  {}", k.as_str());
            }
        }
        KitCmd::New(k) => {
            let k = usecases::kit_new(kits, &k)?;
            println!("Created kit '{0}'. Add to it with: ccs kit use {0} skills/<name> | skills/<folder>/", k.as_str());
        }
        KitCmd::Edit(k) => {
            usecases::kit_edit(kits, editor, &k)?;
            println!("✓ edited the instructions of kit {k}");
        }
        KitCmd::Rename(old, new) => {
            usecases::kit_rename(store, kits, &old, &new)?;
            println!("✓ kit {old} is now {new}, in every profile that had it");
        }
        KitCmd::Remove(k) => println!("✓ moved kit {k} to {}; no profile uses it now", usecases::kit_remove(store, kits, &k)?.display()),
        KitCmd::Show(k) => {
            let (uses, contents) = usecases::kit_show(kits, lib, &k)?;
            println!("kit {k}");
            println!("uses:");
            for u in &uses {
                println!("  {}", u.line());
            }
            println!("gives:");
            for i in &contents.items {
                println!("  {}  {}", i.kind.dir(), i.path());
            }
            if !contents.missing.is_empty() {
                println!("not in the library:");
                for m in &contents.missing {
                    println!("  {m}");
                }
            }
        }
        KitCmd::Use(k, entries) => {
            usecases::kit_use(kits, lib, &k, &entries)?;
            println!("✓ kit {k} uses {}", entries.join(" "));
        }
        KitCmd::Drop(k, entries) => {
            usecases::kit_drop(kits, &k, &entries)?;
            println!("✓ kit {k} no longer uses {}", entries.join(" "));
        }
        KitCmd::Attach { kit, profile } => {
            usecases::kit_attach(store, kits, &kit, &profile, true)?;
            println!("✓ {profile} launches with kit {kit}");
        }
        KitCmd::Detach { kit, profile } => {
            usecases::kit_attach(store, kits, &kit, &profile, false)?;
            println!("✓ {profile} no longer launches with kit {kit}");
        }
    }
    Ok(0)
}

fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}

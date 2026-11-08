//! Use cases — one function per `ccs` command.
//!
//! Rule 3: `usecases/` imports `domain/` and `ports/` only. Each takes its
//! ports as parameters and never picks the adapter.

use crate::domain::{
    aws_profile_env, ccs_name_for_aws, checksum_for, git_source, newer_version, release_asset, launch_settings, next_statusline, settings_statusline, statusline_dir, check_env, compose_status, default_asset_name, default_settings, item_name,
    kit_args, kit_name_for_profile, plan_launch, provider_label, rewrite_folder, render_exports, resolve, resolve_kit, status_segment, with_kits, AwsRow,
    agent_name_clashes, load_plan as plan_kits, resolve_lines, CcsError, LineGives, PluginContents, PluginPart, Finding, ImportOutcome, Kind, KitName, LaunchPlan, LegacyItem, LibItem, LibraryRemoved, LoadPlan,
    ListEntry, Overview, Profile, ProfileName, Use, Wrapped,
    permission_preset, with_rules, PermCmd, Permissions,
};
use crate::ports::{
    AwsProfiles, Browser, CommandRunner, Editor, Fetcher, KitStore, Releases, TextFiles, Launcher, Legacy, Library, PathInput, PinLocator, ProfileStore,
};
use std::path::{Path, PathBuf};

/// Turn the four sources of a profile name into a profile that exists.
pub fn resolve_name(
    store: &dyn ProfileStore,
    pins: &dyn PinLocator,
    explicit: Option<String>,
    env_profile: Option<String>,
) -> Result<ProfileName, CcsError> {
    let raw = resolve(explicit, pins.pinned(), env_profile, store.default_name()).ok_or(CcsError::NoneResolved)?;
    existing(store, &raw)
}

/// Parse a name and require the profile to be there.
pub fn existing(store: &dyn ProfileStore, raw: &str) -> Result<ProfileName, CcsError> {
    let name = ProfileName::parse(raw).map_err(|_| CcsError::NoProfile(raw.into()))?;
    if store.exists(&name) {
        Ok(name)
    } else {
        Err(CcsError::NoProfile(raw.into()))
    }
}

pub fn load(store: &dyn ProfileStore, name: &ProfileName) -> Result<Profile, CcsError> {
    store.load(name)
}

/// Replace this process with `claude` under the profile. Returns only on failure.
pub fn launch(launcher: &dyn Launcher, plan: &LaunchPlan) -> CcsError {
    launcher.launch(plan)
}

/// What a profile loads (ADR-2610281305 R2): `chosen` kits, else the profile's
/// own, resolved against the library. Reads only; builds nothing.
pub fn load_plan(
    store: &dyn ProfileStore,
    kits: &dyn KitStore,
    lib: &dyn Library,
    name: &ProfileName,
    chosen: Option<Vec<String>>,
) -> Result<LoadPlan, CcsError> {
    let wanted = chosen.unwrap_or_else(|| store.kits(name));
    let names = wanted.iter().map(|k| existing_kit(kits, k)).collect::<Result<Vec<_>, _>>()?;
    let picked = names
        .into_iter()
        .map(|k| {
            let uses = kits.uses(&k)?;
            let instructions = kits.instructions(&k);
            Ok((k, uses, instructions))
        })
        .collect::<Result<Vec<_>, CcsError>>()?;
    Ok(plan_kits(picked, &library_items(lib)))
}

/// The launch for a plan, naming the paths `build` and `write_prompt` use
/// without touching them.
fn launch_from(kits: &dyn KitStore, profile: &Profile, plan: &LoadPlan, args: Vec<String>, ccs_bin: &str) -> LaunchPlan {
    let names = plan.kit_names();
    let dirs: Vec<_> = names.iter().map(|k| kits.build_dir(k)).chain(plan.plugins.iter().map(|p| p.source.clone())).collect();
    let prompt = plan.prompt().map(|_| kits.prompt_path(&names));
    // The launch's own status line ranks above a project's, so the profile always shows.
    let extra = ["--settings".to_string(), launch_settings(ccs_bin)].into_iter().chain(kit_args(&dirs, prompt.as_deref())).collect();
    with_kits(plan_launch(profile, args), &names, extra)
}

/// The launch `prepare_launch` would make, with nothing built or written.
pub fn planned_launch(
    store: &dyn ProfileStore,
    kits: &dyn KitStore,
    lib: &dyn Library,
    name: &ProfileName,
    chosen: Option<Vec<String>>,
    args: Vec<String>,
    ccs_bin: &str,
) -> Result<(LaunchPlan, Vec<String>), CcsError> {
    let profile = store.load(name)?;
    let plan = load_plan(store, kits, lib, name, chosen)?;
    Ok((launch_from(kits, &profile, &plan, args, ccs_bin), plan.warnings))
}

/// What to run for a profile with kits (ADR-2610261643): the plan, with each
/// kit rebuilt from the library and the instructions written. Returns the plan,
/// and a warning for each kit entry the library no longer has; those do not
/// stop the launch.
pub fn prepare_launch(
    store: &dyn ProfileStore,
    kits: &dyn KitStore,
    lib: &dyn Library,
    name: &ProfileName,
    chosen: Option<Vec<String>>,
    args: Vec<String>,
    ccs_bin: &str,
) -> Result<(LaunchPlan, Vec<String>), CcsError> {
    let profile = store.load(name)?;
    let plan = load_plan(store, kits, lib, name, chosen)?;
    for k in &plan.kits {
        kits.build(&k.kit, &k.build)?;
    }
    if let Some(text) = plan.prompt() {
        kits.write_prompt(&plan.kit_names(), &text)?;
    }
    Ok((launch_from(kits, &profile, &plan, args, ccs_bin), plan.warnings))
}

pub fn list(store: &dyn ProfileStore, current: Option<&str>) -> Vec<ListEntry> {
    let default = store.default_name();
    store
        .names()
        .into_iter()
        .map(|name| ListEntry {
            current: current == Some(name.as_str()),
            is_default: default.as_deref() == Some(name.as_str()),
            kits: store.kits(&name),
            provider: store.load(&name).map(|p| provider_label(&p.env)).unwrap_or_else(|e| e.to_string()),
            name,
        })
        .collect()
}

pub fn new_profile(store: &dyn ProfileStore, name: &ProfileName, from: Option<&ProfileName>) -> Result<(), CcsError> {
    if store.exists(name) {
        return Err(CcsError::Exists(name.as_str().into()));
    }
    if let Some(src) = from {
        if !store.exists(src) {
            return Err(CcsError::NoProfile(src.as_str().into()));
        }
    }
    store.create(name, from)
}

/// `ccs new`, from a typed name: an empty profile gets the status line segment;
/// a copy gets `from`'s settings instead.
pub fn make_profile(store: &dyn ProfileStore, raw: &str, from: Option<&ProfileName>, ccs_bin: &str) -> Result<ProfileName, CcsError> {
    let name = ProfileName::parse(raw)?;
    new_profile(store, &name, from)?;
    if from.is_none() {
        seed_statusline(store, &name, ccs_bin)?;
    }
    Ok(name)
}

/// A shell in the profile's folder, with the variables a launch sets (ADR-2610261643 §14).
pub fn shell_plan(store: &dyn ProfileStore, name: &ProfileName) -> Result<LaunchPlan, CcsError> {
    let profile = load(store, &existing(store, name.as_str())?)?;
    Ok(LaunchPlan { dir: Some(store.folder(name)), ..plan_launch(&profile, vec![]) })
}

/// Open a profile's `profile.env` in the editor.
fn profile_edit(store: &dyn ProfileStore, editor: &dyn Editor, name: &ProfileName) -> Result<(), CcsError> {
    existing(store, name.as_str())?;
    editor.edit(&store.env_file(name))
}

pub fn default_name(store: &dyn ProfileStore) -> Option<String> {
    store.default_name()
}

pub fn set_default(store: &dyn ProfileStore, name: &ProfileName) -> Result<(), CcsError> {
    if !store.exists(name) {
        return Err(CcsError::NoProfile(name.as_str().into()));
    }
    store.set_default(name)
}

/// Check a profile and its kits. `claude_available` comes from the launcher port.
pub fn doctor(
    store: &dyn ProfileStore,
    kits: &dyn KitStore,
    lib: &dyn Library,
    legacy: &dyn Legacy,
    name: &ProfileName,
    claude_available: bool,
) -> Result<Vec<Finding>, CcsError> {
    let profile = store.load(name)?;
    let mut out = check_env(&profile.env);
    out.push(Finding::Pass(format!("config dir: {}", profile.config_dir.display())));
    let library = library_items(lib);
    for k in store.kits(name) {
        match existing_kit(kits, &k).map(|kit| kits.uses(&kit)) {
            Err(_) => out.push(Finding::Fail(format!("kit '{k}' does not exist (see: ccs kit list)"))),
            Ok(Err(e)) => out.push(Finding::Fail(format!("kit {k}: {e}"))),
            Ok(Ok(uses)) => {
                let contents = resolve_kit(&uses, &library);
                out.extend(contents.missing.iter().map(|m| Finding::Warn(format!("kit {k}: {m} is not in the library"))));
                out.push(Finding::Pass(format!("kit {k}: {} items", contents.items.len())));
            }
        }
    }
    let present: Vec<String> = store.kits(name).into_iter().filter(|k| existing_kit(kits, k).and_then(|kit| kits.uses(&kit)).is_ok()).collect();
    if let Ok(plan) = load_plan(store, kits, lib, name, Some(present)) {
        out.extend(agent_name_clashes(&plan.kits).into_iter().map(Finding::Warn));
    }
    let layout = legacy.layout();
    if !layout.is_empty() {
        out.push(Finding::Warn("an older layout is here (shared/, native, or ccs links); run: ccs migrate".into()));
    }
    for p in layout.profiles.iter().filter(|p| p.name == *name) {
        out.extend(p.own.iter().map(|i| Finding::Warn(format!("{}/{} sits in the config dir, outside any kit", i.kind.dir(), legacy_path(i)))));
    }
    if !claude_available {
        out.push(Finding::Fail("claude is not on PATH".into()));
    }
    Ok(out)
}

/// The profile as shell exports, for `eval "$(ccs env work)"`.
pub fn exports(store: &dyn ProfileStore, name: &ProfileName) -> Result<String, CcsError> {
    Ok(render_exports(&plan_launch(&store.load(name)?, vec![])))
}

/// `ccs permissions`: the lines to print. Rules are checked before the file is touched.
pub fn permissions(store: &dyn ProfileStore, name: &ProfileName, cmd: &PermCmd) -> Result<Vec<String>, CcsError> {
    let have = store.permissions(name)?;
    let shown = |p: &Permissions| {
        let mut out = vec![format!("mode: {}", p.mode.as_deref().unwrap_or("(not set; claude asks)")), format!("allow ({}):", p.allow.len())];
        out.extend(p.allow.iter().map(|r| format!("  {r}")));
        out
    };
    match cmd {
        PermCmd::Show => Ok(shown(&have)),
        PermCmd::Add { rules, preset } => {
            let (mode, from_preset): (Option<String>, Vec<String>) = match preset.as_deref().and_then(permission_preset) {
                Some((m, r)) => (Some(m.to_string()), r.iter().map(|s| s.to_string()).collect()),
                None => (None, vec![]),
            };
            let wanted: Vec<String> = from_preset.into_iter().chain(rules.iter().cloned()).collect();
            let allow = with_rules(&have.allow, &wanted);
            let added = allow.len() - have.allow.len();
            let next = Permissions { mode: mode.or(have.mode.clone()), allow };
            store.set_permissions(name, &next)?;
            let mut out = vec![format!("Added {added} rule(s) to {}; {} already there.", name.as_str(), wanted.len() - added)];
            if let Some(m) = &next.mode {
                if have.mode.as_ref() != Some(m) {
                    out.push(format!("Mode is now {m}."));
                }
            }
            Ok(out)
        }
        PermCmd::Rm(rules) => {
            let mut out = vec![];
            for r in rules.iter().filter(|r| !have.allow.contains(r)) {
                out.push(format!("{r}: not in the allow list"));
            }
            let allow: Vec<String> = have.allow.iter().filter(|r| !rules.contains(r)).cloned().collect();
            let removed = have.allow.len() - allow.len();
            if removed > 0 {
                store.set_permissions(name, &Permissions { mode: None, allow })?;
            }
            out.push(format!("Removed {removed} rule(s) from {}.", name.as_str()));
            Ok(out)
        }
        PermCmd::Mode(m) => {
            store.set_permissions(name, &Permissions { mode: Some(m.clone()), allow: have.allow })?;
            Ok(vec![format!("Mode of {} is now {m}.", name.as_str())])
        }
    }
}

/// What `ccs upgrade` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Upgraded {
    UpToDate(String),
    Available { current: String, latest: String },
    Installed { from: String, to: String, at: std::path::PathBuf },
}

/// `ccs upgrade` (ADR-2610271000 §2): the latest release, or `wanted`, checked
/// against `SHA256SUMS.txt` and run once before it replaces the running binary.
pub fn upgrade(rel: &dyn Releases, current: &str, target: Option<&str>, wanted: Option<&str>, check: bool) -> Result<Upgraded, CcsError> {
    let target = target.ok_or_else(|| {
        CcsError::Usage("no release is built for this platform; build from source: ./start.sh --install".into())
    })?;
    let tag = match wanted {
        Some(v) => format!("v{}", v.trim_start_matches('v')),
        None => rel.latest()?,
    };
    let version = tag.trim_start_matches('v').to_string();
    if wanted.is_none() && !newer_version(&version, current) {
        return Ok(Upgraded::UpToDate(current.into()));
    }
    if check {
        return Ok(Upgraded::Available { current: current.into(), latest: version });
    }
    let asset = release_asset(&version, target);
    let dir = rel.download(&tag, &[asset.clone(), "SHA256SUMS.txt".into()])?;
    let installed = (|| {
        let want = checksum_for(&rel.read(&dir.join("SHA256SUMS.txt"))?, &asset)
            .ok_or_else(|| CcsError::Io(format!("SHA256SUMS.txt of {tag} has no line for {asset}; nothing installed")))?;
        let got = rel.sha256(&dir.join(&asset))?;
        if got != want {
            return Err(CcsError::Io(format!("{asset}: checksum {got} is not the published {want}; nothing installed")));
        }
        let binary = rel.unpack(&dir.join(&asset))?;
        let said = rel.probe(&binary).map_err(|e| CcsError::Io(format!("ccs {version}: {e}; nothing installed")))?;
        // The version word itself: `ccs 0.5.10` is not 0.5.1.
        if said.split_whitespace().last().map(|w| w.trim_start_matches('v')) != Some(version.as_str()) {
            return Err(CcsError::Io(format!("the downloaded ccs says '{said}', not {version}; nothing installed")));
        }
        rel.install(&binary)
    })();
    // Success or not, the download folder goes (drift report 2026-10-27).
    rel.discard(&dir);
    Ok(Upgraded::Installed { from: current.into(), to: version, at: installed? })
}

/// `ccs statusline --auto`, the status line every launch sets: the segment,
/// then the profile's own status line, then the project's (its local settings
/// before its shared ones), each run with the same stdin. A command both name
/// runs once.
pub fn statusline_auto(runner: &dyn CommandRunner, files: &dyn TextFiles, vars: &[(String, String)], stdin: &[u8]) -> String {
    let payload = String::from_utf8_lossy(stdin);
    let command = |p: PathBuf| files.read(&p).and_then(|t| settings_statusline(&t));
    let project = statusline_dir(&payload).and_then(|dir| {
        let dir = Path::new(&dir).join(".claude");
        next_statusline(&[command(dir.join("settings.local.json")), command(dir.join("settings.json"))])
    });
    let profile = vars
        .iter()
        .find(|(k, v)| k == "CLAUDE_CONFIG_DIR" && !v.is_empty())
        .and_then(|(_, cfg)| next_statusline(&[command(Path::new(cfg).join("settings.json"))]));
    let mut commands: Vec<String> = profile.into_iter().collect();
    commands.extend(project.filter(|p| !commands.contains(p)));
    let wrapped: Vec<Wrapped> = commands
        .into_iter()
        .map(|cmd| match runner.run(&["/bin/sh".into(), "-c".into(), cmd.clone()], stdin) {
            Some(out) => Wrapped::Output(out),
            None => Wrapped::Failed(cmd),
        })
        .collect();
    compose_status(status_segment(vars), &wrapped)
}

/// The status line (ADR-2610261237): the segment, in front of a wrapped command's output.
pub fn statusline(runner: &dyn CommandRunner, vars: &[(String, String)], stdin: &[u8], wrap: &[String]) -> String {
    let wrapped = if wrap.is_empty() {
        Wrapped::Nothing
    } else {
        match runner.run(wrap, stdin) {
            Some(out) => Wrapped::Output(out),
            None => Wrapped::Failed(wrap.join(" ")),
        }
    };
    compose_status(status_segment(vars), &[wrapped])
}

/// Give a profile the `ccs` status line, unless it already has a `settings.json`.
pub fn seed_statusline(store: &dyn ProfileStore, name: &ProfileName, ccs_bin: &str) -> Result<bool, CcsError> {
    store.write_settings_if_absent(name, &default_settings(ccs_bin))
}

/// Which `ccs` profile, if any, already runs on the AWS profile `aws`.
fn imported_as(store: &dyn ProfileStore, aws: &str) -> Option<String> {
    store.names().into_iter().find_map(|n| {
        let p = store.load(&n).ok()?;
        p.env.iter().any(|(k, v)| k == "AWS_PROFILE" && v == aws).then(|| n.as_str().to_string())
    })
}

/// Every AWS CLI profile and whether a `ccs` profile uses it. Changes nothing.
pub fn aws_status(store: &dyn ProfileStore, aws: &dyn AwsProfiles) -> Result<Vec<AwsRow>, CcsError> {
    Ok(aws.names()?.into_iter().map(|a| AwsRow { imported_as: imported_as(store, &a), aws: a }).collect())
}

/// Make one Bedrock profile per AWS profile (ADR-2610261238). Skips, never overwrites.
pub fn import_aws(
    store: &dyn ProfileStore,
    aws: &dyn AwsProfiles,
    wanted: &[String],
    all: bool,
    region: Option<&str>,
    ccs_bin: &str,
) -> Result<Vec<ImportOutcome>, CcsError> {
    if wanted.is_empty() && !all {
        return Err(CcsError::Usage("ccs import aws <aws-profile>… | ccs import aws --all".into()));
    }
    let known = aws.names()?;
    let targets = if all { known.clone() } else { wanted.to_vec() };
    let mut out = Vec::new();
    for a in targets {
        let skip = |reason: String| ImportOutcome::Skipped { aws: a.clone(), reason };
        if !known.contains(&a) {
            out.push(skip("no such AWS profile".into()));
            continue;
        }
        let name = match ccs_name_for_aws(&a) {
            Ok(n) => n,
            Err(_) => {
                out.push(skip("not a usable ccs profile name; make it by hand with ccs new".into()));
                continue;
            }
        };
        if store.exists(&name) {
            out.push(skip(format!("ccs profile '{}' already exists", name.as_str())));
            continue;
        }
        let r = aws.region(&a).or_else(|| region.map(String::from));
        store.create(&name, None)?;
        // Undo a half-made profile: left behind, it would count as existing, and
        // every later import would skip it instead of finishing it. Report the
        // failure as this profile's outcome, so the ones already made still show.
        let finish = store
            .write_env(&name, &aws_profile_env(&a, r.as_deref()))
            .and_then(|_| seed_statusline(store, &name, ccs_bin));
        if let Err(e) = finish {
            let _ = store.discard(&name);
            out.push(skip(format!("could not finish ccs profile '{}': {e}", name.as_str())));
            continue;
        }
        out.push(ImportOutcome::Created { name: name.as_str().into(), region: r });
    }
    Ok(out)
}

// ── Start with nothing: import and migrate (ADR-2610261643 §5, §8) ──────────

fn legacy_path(i: &LegacyItem) -> String {
    if i.folder.is_empty() {
        i.name.clone()
    } else {
        format!("{}/{}", i.folder, i.name)
    }
}

/// Copy old items into the library under `base`. An item whose name the library
/// already has elsewhere stays where it is, and a note names it. Returns which
/// kinds are in the library at `base`, and the originals that are now copied
/// there. `retire` only words the notes: the caller retires those originals,
/// after the kit that points at the copies is written.
fn adopt(
    lib: &dyn Library,
    items: &[LegacyItem],
    base: &str,
    owner: &str,
    retire: bool,
    notes: &mut Vec<String>,
) -> Result<([bool; 2], Vec<PathBuf>), CcsError> {
    let mut landed = [false, false];
    let mut copied = Vec::new();
    for i in items {
        let what = format!("{}/{}", i.kind.dir(), legacy_path(i));
        let folder = if i.folder.is_empty() { base.to_string() } else { format!("{base}/{}", i.folder) };
        if let Some(have) = lib.items(i.kind).into_iter().find(|x| x.name == i.name) {
            let have = format!("{}/{}", i.kind.dir(), have.path());
            let ours = have == format!("{}/{folder}/{}", i.kind.dir(), i.name);
            if ours {
                // A run that stopped before its kit was written: finish it.
                landed[usize::from(i.kind == Kind::Agent)] = true;
                copied.push(i.source.clone());
            }
            notes.push(match (ours, retire) {
                (true, _) => format!("{owner}: {what} already in the library"),
                // A copied source (~/.claude) keeps everything; nothing is "left" anywhere.
                (false, false) => format!("{owner}: {what} not imported: the library already has {have}"),
                (false, true) => format!("{owner}: {what} left in place: the library already has {have}"),
            });
            continue;
        }
        lib.add(i.kind, &folder, &i.name, &i.source)?;
        landed[usize::from(i.kind == Kind::Agent)] = true;
        copied.push(i.source.clone());
        notes.push(format!("{owner}: {what} → library/{}/{folder}/", i.kind.dir()));
    }
    Ok((landed, copied))
}

/// Make the kit if needed, add folder entries for what landed, and take
/// instructions when the kit has none.
fn fill_kit(kits: &dyn KitStore, kit: &KitName, folder: &str, landed: [bool; 2], instructions: Option<String>) -> Result<(), CcsError> {
    if !kits.exists(kit) {
        kits.create(kit)?;
    }
    let mut uses = kits.uses(kit)?;
    for (kind, ok) in [(Kind::Skill, landed[0]), (Kind::Agent, landed[1])] {
        let u = Use::Folder(kind, folder.into());
        if ok && !uses.contains(&u) {
            uses.push(u);
        }
    }
    kits.set_uses(kit, &uses)?;
    if let (Some(text), None) = (instructions, kits.instructions(kit)) {
        kits.set_instructions(kit, &text)?;
    }
    Ok(())
}

fn attach_last(store: &dyn ProfileStore, profile: &ProfileName, kit: &KitName) -> Result<(), CcsError> {
    let mut mine = store.read_kits(profile)?;
    if !mine.iter().any(|k| k == kit.as_str()) {
        mine.push(kit.as_str().into());
        store.set_kits(profile, &mine)?;
    }
    Ok(())
}

/// Copy `~/.claude`'s skills, agents and CLAUDE.md into the library and the kit
/// `imported`. `~/.claude` is only read. Returns a line per item.
pub fn import(lib: &dyn Library, kits: &dyn KitStore, legacy: &dyn Legacy) -> Result<Vec<String>, CcsError> {
    let (items, md) = legacy.stock();
    let mut notes = Vec::new();
    let (landed, _) = adopt(lib, &items, "imported", "~/.claude", false, &mut notes)?;
    let kit = KitName::parse("imported")?;
    let text = md.map(|m| legacy.read(&m)).transpose()?;
    fill_kit(kits, &kit, "imported", landed, text)?;
    Ok(notes)
}

/// Convert the layout earlier versions left (ADR-2610261643 §5). Returns notes;
/// empty when there was nothing to do. `~/.claude` is only read.
pub fn migrate(store: &dyn ProfileStore, lib: &dyn Library, kits: &dyn KitStore, legacy: &dyn Legacy) -> Result<Vec<String>, CcsError> {
    let layout = legacy.layout();
    if layout.is_empty() {
        return Ok(vec![]);
    }
    let mut notes = Vec::new();
    if let Some((dir, items, md)) = &layout.shared {
        let kit = KitName::parse("shared")?;
        let (landed, _) = adopt(lib, items, "shared", "shared", false, &mut notes)?;
        let text = md.as_ref().map(|m| legacy.read(m)).transpose()?;
        // Earlier versions always made shared/, often empty: no kit for nothing.
        if landed.contains(&true) || text.is_some() {
            fill_kit(kits, &kit, "shared", landed, text)?;
            for p in &layout.profiles {
                attach_last(store, &p.name, &kit)?;
                for u in &p.unshared {
                    notes.push(format!("{}: had opted out of {u} from shared; kit shared now gives it (ccs kit detach shared {})", p.name.as_str(), p.name.as_str()));
                }
            }
        }
        if let Some(at) = legacy.retire(dir)? {
            notes.push(format!("shared/ → {}", at.display()));
        }
    }
    for p in &layout.profiles {
        let kit = kit_name_for_profile(p.name.as_str());
        let (landed, copied) = adopt(lib, &p.own, kit.as_str(), p.name.as_str(), true, &mut notes)?;
        let text = p.claude_md.as_ref().map(|m| legacy.read(m)).transpose()?;
        if landed.contains(&true) || text.is_some() {
            fill_kit(kits, &kit, kit.as_str(), landed, text)?;
            attach_last(store, &p.name, &kit)?;
            // Only now does a kit load the copies; retiring earlier strands
            // them if the kit cannot be written.
            for src in &copied {
                legacy.retire(src)?;
            }
            if let Some(m) = &p.claude_md {
                legacy.retire(m)?;
                notes.push(format!("{}: CLAUDE.md → kit {}", p.name.as_str(), kit.as_str()));
            }
        }
        // Last: these mark the old layout, so a rerun after a failure above
        // still finds this profile.
        for link in &p.links {
            legacy.retire(link)?;
        }
        if !p.unshared.is_empty() {
            legacy.retire(&p.dir.join("unshared"))?;
        }
    }
    if let Some(dir) = &layout.native {
        notes.extend(import(lib, kits, legacy)?);
        notes.push("native: no longer a profile; ~/.claude is in kit imported (attach it: ccs kit attach imported <profile>)".into());
        let native = dir.file_name().map(|f| f.to_string_lossy().into_owned());
        if store.default_name() == native {
            store.clear_default()?;
            notes.push("the default was native; none is set now".into());
        }
        legacy.retire(dir)?;
    }
    Ok(notes)
}

/// Whether an earlier version's layout is still here.
pub fn needs_migrate(legacy: &dyn Legacy) -> bool {
    !legacy.layout().is_empty()
}

// ── The library and kits (ADR-2610261643) ───────────────────────────────────

fn library_items(lib: &dyn Library) -> Vec<LibItem> {
    [Kind::Skill, Kind::Agent, Kind::Plugin].into_iter().flat_map(|k| lib.items(k)).collect()
}

/// The item an entry names, by name, wherever its folder is.
fn library_item(lib: &dyn Library, what: &Use) -> Result<LibItem, CcsError> {
    let name = match what {
        Use::Item(_, n) => n,
        Use::Folder(..) => return Err(CcsError::Usage(format!("{} is a folder, not an item", what.line()))),
    };
    lib.items(what.kind()).into_iter().find(|i| i.name == *name).ok_or_else(|| CcsError::NoAsset(format!("{} in the library", what.line())))
}

fn existing_kit(kits: &dyn KitStore, raw: &str) -> Result<KitName, CcsError> {
    let k = KitName::parse(raw).map_err(|_| CcsError::NoKit(raw.into()))?;
    if kits.exists(&k) {
        Ok(k)
    } else {
        Err(CcsError::NoKit(raw.into()))
    }
}

/// Items and folders of one kind.
pub fn library_list(lib: &dyn Library, kind: Kind) -> (Vec<LibItem>, Vec<String>) {
    (lib.items(kind), lib.folders(kind))
}

/// Copy a skill folder (or its `SKILL.md`) or an agent `.md` into `folder`.
/// The kind comes from the source. Names are unique in the library.
fn library_add(lib: &dyn Library, source: &Path, folder: &str) -> Result<(Kind, String), CcsError> {
    let (kind, dir) = lib.classify(source)?;
    let name = if kind == Kind::Plugin {
        if !folder.is_empty() {
            return Err(no_folders());
        }
        // Claude Code knows a plugin by its manifest name, so the library does too.
        lib.plugin_name(&dir)?
    } else {
        default_asset_name(kind, &dir.to_string_lossy())
            .ok_or_else(|| CcsError::Usage(format!("cannot name a {} from {}", kind.label(), source.display())))?
    };
    let name = item_name(kind, &name)?;
    if let Some(i) = lib.items(kind).into_iter().find(|i| i.name == name) {
        return Err(CcsError::AssetExists(format!("{}/{name} (library/{}/{})", kind.dir(), kind.dir(), i.path())));
    }
    lib.add(kind, folder, &name, &dir)?;
    Ok((kind, name))
}

/// `ccs library add` and the TUI's `a`: a local path when one exists (`local`
/// is `raw` expanded), else a git source, cloned and then added like a path.
pub fn library_add_source(lib: &dyn Library, fetch: &dyn Fetcher, raw: &str, local: &Path, folder: &str) -> Result<(Kind, String), CcsError> {
    if local.exists() {
        return library_add(lib, local, folder);
    }
    let Some(url) = git_source(raw) else { return library_add(lib, local, folder) };
    let tmp = fetch.clone_temp(&url)?;
    let added = library_add(lib, &tmp, folder).map_err(|e| match e {
        CcsError::Usage(_) if lib.plugin_name(&tmp).is_err() => {
            CcsError::Usage(format!("{url} is not a plugin: its root has no .claude-plugin/plugin.json"))
        }
        e => e,
    });
    fetch.discard(&tmp);
    added
}

/// `git pull` every plugin, or one. One line per plugin.
pub fn plugin_update(lib: &dyn Library, fetch: &dyn Fetcher, which: Option<&Use>) -> Result<Vec<String>, CcsError> {
    let plugins = match which {
        Some(u) => vec![library_item(lib, u)?],
        None => lib.items(Kind::Plugin),
    };
    if plugins.is_empty() {
        return Ok(vec!["no plugins in the library; add one with: ccs library add <owner/repo>".into()]);
    }
    Ok(plugins
        .iter()
        .map(|p| match fetch.pull(&p.source) {
            Ok(Some(change)) => format!("plugins/{}: {change}", p.name),
            Ok(None) => format!("plugins/{}: not a git clone; add it again from git to update it", p.name),
            Err(e) => format!("plugins/{}: {e}", p.name),
        })
        .collect())
}

fn no_folders() -> CcsError {
    CcsError::Usage("plugins have no folders: a plugin is plugins/<name>".into())
}

pub fn library_mkdir(lib: &dyn Library, kind: Kind, folder: &str) -> Result<(), CcsError> {
    if kind == Kind::Plugin {
        return Err(no_folders());
    }
    lib.make_folder(kind, folder)
}

/// One kit whose lines a folder move changes: the kit, its new lines, its old lines.
type KitRewrite = (KitName, Vec<Use>, Vec<Use>);

/// Every kit line under folder `from`, pointed at `to` (ADR-2610261643 §9).
/// Reads every kit before the folder moves: a kit that cannot be read stops
/// the move while a retry can still find the folder.
/// Each entry keeps the kit's old lines too, so a failed write can put it back.
fn plan_kit_rewrites(kits: &dyn KitStore, kind: Kind, from: &str, to: &str) -> Result<Vec<KitRewrite>, CcsError> {
    let mut out = Vec::new();
    for k in kits.names() {
        let uses = kits.uses(&k)?;
        let moved = rewrite_folder(&uses, kind, from, to);
        if moved != uses {
            out.push((k, uses, moved));
        }
    }
    Ok(out)
}

/// The folder has moved. If one kit cannot be written, put back the kits
/// already written and move the folder back (`undo_move`), so no kit is left
/// on a folder that is gone and a retry can still find the folder.
fn write_kit_rewrites(kits: &dyn KitStore, plan: &[(KitName, Vec<Use>, Vec<Use>)], undo_move: impl FnOnce() -> Result<(), CcsError>) -> Result<(), CcsError> {
    for (i, (k, _, moved)) in plan.iter().enumerate() {
        if let Err(e) = kits.set_uses(k, moved) {
            for (k, was, _) in &plan[..i] {
                let _ = kits.set_uses(k, was);
            }
            let _ = undo_move();
            return Err(e);
        }
    }
    Ok(())
}

/// Move an item or folder into `to`. Kits pick items by name, so an item move
/// changes no kit; a folder move rewrites the kit lines under it.
pub fn library_move(lib: &dyn Library, kits: &dyn KitStore, what: &Use, to: &str) -> Result<(), CcsError> {
    let kind = what.kind();
    if kind == Kind::Plugin {
        return Err(no_folders());
    }
    let from = match what {
        Use::Item(..) => {
            let item = library_item(lib, what)?;
            if item.folder == to {
                return Ok(());
            }
            item.path()
        }
        Use::Folder(_, f) => {
            if !lib.folders(kind).contains(f) {
                return Err(CcsError::NoAsset(format!("{} in the library", what.line())));
            }
            if to == f || to.starts_with(&format!("{f}/")) {
                return Err(CcsError::Usage(format!("cannot move {} into itself", what.line())));
            }
            f.clone()
        }
    };
    let Use::Folder(_, f) = what else { return lib.move_to(kind, &from, to) };
    let leaf = f.rsplit('/').next().unwrap_or(f);
    let at = if to.is_empty() { leaf.to_string() } else { format!("{to}/{leaf}") };
    let plan = plan_kit_rewrites(kits, kind, f, &at)?;
    lib.move_to(kind, &from, to)?;
    let parent = f.rsplit_once('/').map_or("", |(p, _)| p);
    write_kit_rewrites(kits, &plan, || lib.move_to(kind, &at, parent))
}

/// Rename a folder where it is, and the kit lines under it.
pub fn library_rename_folder(lib: &dyn Library, kits: &dyn KitStore, kind: Kind, folder: &str, name: &str) -> Result<(), CcsError> {
    if kind == Kind::Plugin {
        return Err(no_folders());
    }
    let name = item_name(kind, name).map_err(|_| CcsError::Usage(format!("'{name}' is not a folder name (letters, digits, . _ -)")))?;
    if !lib.folders(kind).iter().any(|f| f == folder) {
        return Err(CcsError::NoAsset(format!("{}/{folder}/ in the library", kind.dir())));
    }
    let to = match folder.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/{name}"),
        None => name.clone(),
    };
    let plan = plan_kit_rewrites(kits, kind, folder, &to)?;
    lib.rename_folder(kind, folder, &name)?;
    let leaf = folder.rsplit('/').next().unwrap_or(folder);
    write_kit_rewrites(kits, &plan, || lib.rename_folder(kind, &to, leaf))
}

/// Rename a kit, keeping its place in every profile's list.
/// A kit is built as a plugin named after it, so a kit that takes a library
/// plugin's name loads two plugins with one name (ADR-2610281305 R4).
fn refuse_plugin_name(lib: &dyn Library, name: &str) -> Result<(), CcsError> {
    if lib.items(Kind::Plugin).iter().any(|p| p.name == name) {
        return Err(CcsError::Usage(format!("{name} is a plugin in the library; two plugins would load with one name. Choose another name.")));
    }
    Ok(())
}

pub fn kit_rename(store: &dyn ProfileStore, kits: &dyn KitStore, lib: &dyn Library, old: &str, new: &str) -> Result<(), CcsError> {
    let from = existing_kit(kits, old)?;
    let to = KitName::parse(new)?;
    refuse_plugin_name(lib, new)?;
    if kits.exists(&to) {
        return Err(CcsError::Usage(format!("kit '{new}' already exists")));
    }
    // Profiles first, folder last: a failure part way puts back the profiles
    // already rewritten, so every profile still names a kit that exists.
    let mut done: Vec<(ProfileName, Vec<String>)> = Vec::new();
    let undo = |done: &[(ProfileName, Vec<String>)]| {
        for (p, was) in done {
            let _ = store.set_kits(p, was);
        }
    };
    for p in store.names() {
        let mine = store.kits(&p);
        if mine.iter().any(|k| k == old) {
            let renamed: Vec<String> = mine.iter().map(|k| if k == old { new.to_string() } else { k.clone() }).collect();
            if let Err(e) = store.set_kits(&p, &renamed) {
                undo(&done);
                return Err(e);
            }
            done.push((p, mine));
        }
    }
    if let Err(e) = kits.rename(&from, &to) {
        undo(&done);
        return Err(e);
    }
    Ok(())
}

pub fn library_remove(lib: &dyn Library, kits: &dyn KitStore, what: &Use) -> Result<LibraryRemoved, CcsError> {
    let kind = what.kind();
    let library = library_items(lib);
    let (path, gone): (String, Vec<&LibItem>) = match what {
        Use::Item(..) => {
            let item = library_item(lib, what)?;
            let gone = library.iter().filter(|i| i.kind == kind && i.name == item.name).collect();
            (item.path(), gone)
        }
        Use::Folder(_, f) => {
            if !lib.folders(kind).contains(f) {
                return Err(CcsError::NoAsset(format!("{} in the library", what.line())));
            }
            (f.clone(), library.iter().filter(|i| i.kind == kind && i.in_folder(f)).collect())
        }
    };
    // A kit that cannot be read stops the remove before anything is trashed:
    // read as empty, it would be left out of `kits` with a dangling line.
    let mut users = Vec::new();
    for k in kits.names() {
        let uses = kits.uses(&k)?;
        if uses.contains(what) || resolve_kit(&uses, &library).items.iter().any(|i| gone.contains(&i)) {
            users.push(k.as_str().to_string());
        }
    }
    Ok(LibraryRemoved { trashed: lib.trash(kind, &path)?, kits: users })
}

/// A skill's or agent's text; for a plugin, what it holds (ADR-2610281305 R5).
pub fn library_show(lib: &dyn Library, what: &Use) -> Result<String, CcsError> {
    let item = library_item(lib, what)?;
    if item.kind != Kind::Plugin {
        return lib.read(&item);
    }
    let c = lib.plugin_contents(&item);
    let row = |p: &PluginPart| {
        let shown = p.shown_name.as_deref().filter(|n| *n != p.file).map(|n| format!(" (shown as {n})")).unwrap_or_default();
        let about = p.description.as_deref().map(|d| format!("  {d}")).unwrap_or_default();
        format!("  {}{shown}{about}\n", p.file)
    };
    let mut out = format!("{}\n", item.description.as_deref().unwrap_or(&item.name));
    out += &format!("\nSKILLS ({})\n", c.skills.len());
    out.extend(c.skills.iter().map(row));
    out += &format!("\nAGENTS ({})\n", c.agents.len());
    out.extend(c.agents.iter().map(row));
    Ok(out)
}

pub fn library_edit(lib: &dyn Library, editor: &dyn Editor, what: &Use) -> Result<(), CcsError> {
    editor.edit(&lib.main_file(&library_item(lib, what)?))
}

pub fn kit_new(kits: &dyn KitStore, lib: &dyn Library, raw: &str) -> Result<KitName, CcsError> {
    let k = KitName::parse(raw)?;
    refuse_plugin_name(lib, raw)?;
    if kits.exists(&k) {
        return Err(CcsError::Usage(format!("kit '{raw}' already exists")));
    }
    kits.create(&k)?;
    Ok(k)
}

/// Trash a kit and detach it from every profile.
pub fn kit_remove(store: &dyn ProfileStore, kits: &dyn KitStore, raw: &str) -> Result<std::path::PathBuf, CcsError> {
    let k = existing_kit(kits, raw)?;
    // Profiles first, folder last, as in kit_rename: a failure part way puts
    // back the profiles already detached, so a failed remove changes nothing.
    let mut done: Vec<(ProfileName, Vec<String>)> = Vec::new();
    let undo = |done: &[(ProfileName, Vec<String>)]| {
        for (p, was) in done {
            let _ = store.set_kits(p, was);
        }
    };
    for p in store.names() {
        let mine = store.kits(&p);
        if mine.iter().any(|m| m == k.as_str()) {
            let kept: Vec<String> = mine.iter().filter(|m| *m != k.as_str()).cloned().collect();
            if let Err(e) = store.set_kits(&p, &kept) {
                undo(&done);
                return Err(e);
            }
            done.push((p, mine));
        }
    }
    kits.remove(&k).inspect_err(|_| undo(&done))
}

/// Everything a profile loads, kit by kit and line by line, for `ccs loads`.
pub fn loads(profile: &ProfileName, plan: &LoadPlan) -> String {
    let names: Vec<&str> = plan.kits.iter().map(|k| k.kit.as_str()).collect();
    let mut out = if names.is_empty() {
        format!("{} loads no kits\n", profile.as_str())
    } else {
        format!("{} loads {} kit{}: {}\n", profile.as_str(), names.len(), if names.len() == 1 { "" } else { "s" }, names.join(" "))
    };
    for k in &plan.kits {
        out += &format!("\nkit {}\n", k.kit.as_str());
        if k.instructions.is_some() {
            out += "  instructions (CLAUDE.md)\n";
        }
        out += &lines_text(&k.lines);
    }
    let mut seen: Vec<&LibItem> = Vec::new();
    for i in plan.kits.iter().flat_map(|k| &k.build) {
        if !seen.iter().any(|s| s.kind == i.kind && s.path() == i.path()) {
            seen.push(i);
        }
    }
    let count = |kind| seen.iter().filter(|i| i.kind == kind).count();
    let plural = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
    out += &format!(
        "\ntotal: {} · {} · {}\n",
        plural(count(Kind::Skill), "skill"),
        plural(count(Kind::Agent), "agent"),
        plural(plan.plugins.len(), "plugin")
    );
    if !plan.warnings.is_empty() {
        out += "\nwarnings:\n";
        out.extend(plan.warnings.iter().map(|w| format!("  {w}\n")));
    }
    out
}

/// A kit's lines, each with the items it gives, as `ccs loads` and `ccs kit show` print them.
pub fn lines_text(lines: &[LineGives]) -> String {
    if lines.is_empty() {
        return "  (picks nothing)\n".into();
    }
    let mut out = String::new();
    for g in lines {
        let what = if g.missing {
            "not in the library".to_string()
        } else {
            match &g.line {
                Use::Folder(kind, _) => format!("folder · {} {}{}", g.items.len(), kind.label(), if g.items.len() == 1 { "" } else { "s" }),
                Use::Item(kind, _) => kind.label().to_string(),
            }
        };
        out += &format!("  {:<32} {what}\n", g.line.line());
        if let Use::Folder(..) = g.line {
            out.extend(g.items.iter().map(|i| format!("    {}\n", i.path())));
        } else if let Some(i) = g.items.first().filter(|i| i.kind != Kind::Plugin) {
            out += &format!("    {}\n", i.path());
        }
    }
    out
}

/// A kit's lines, each with what it gives (ADR-2610281305): `ccs kit show`
/// and the screen's column 3.
pub fn kit_show(kits: &dyn KitStore, lib: &dyn Library, raw: &str) -> Result<Vec<LineGives>, CcsError> {
    let k = existing_kit(kits, raw)?;
    Ok(resolve_lines(&kits.uses(&k)?, &library_items(lib)))
}

/// Add entries to a kit. Each must name something in the library now.
pub fn kit_use(kits: &dyn KitStore, lib: &dyn Library, raw: &str, entries: &[String]) -> Result<(), CcsError> {
    let k = existing_kit(kits, raw)?;
    let library = library_items(lib);
    let mut uses = kits.uses(&k)?;
    for e in entries {
        let u = Use::parse(e)?;
        if !resolve_kit(std::slice::from_ref(&u), &library).missing.is_empty() {
            return Err(CcsError::NoAsset(format!("{} in the library (see: ccs library list)", u.line())));
        }
        if !uses.contains(&u) {
            uses.push(u);
        }
    }
    kits.set_uses(&k, &uses)
}

pub fn kit_drop(kits: &dyn KitStore, raw: &str, entries: &[String]) -> Result<(), CcsError> {
    let k = existing_kit(kits, raw)?;
    let mut uses = kits.uses(&k)?;
    // Check against the kit as it was, so an entry named twice is not refused
    // the second time because the first already removed it.
    let had = uses.clone();
    for e in entries {
        let u = Use::parse(e)?;
        if !had.contains(&u) {
            return Err(CcsError::NoAsset(format!("{} in kit {raw}", u.line())));
        }
        uses.retain(|x| *x != u);
    }
    kits.set_uses(&k, &uses)
}

/// Set a profile's default kits, in order. Each must exist.
fn set_profile_kits(store: &dyn ProfileStore, kits: &dyn KitStore, profile: &ProfileName, chosen: &[String]) -> Result<(), CcsError> {
    for k in chosen {
        existing_kit(kits, k)?;
    }
    store.set_kits(&existing(store, profile.as_str())?, chosen)
}

pub fn kit_attach(store: &dyn ProfileStore, kits: &dyn KitStore, kit: &str, profile: &str, attach: bool) -> Result<(), CcsError> {
    let k = existing_kit(kits, kit)?;
    let p = existing(store, profile)?;
    let mut mine: Vec<String> = store.read_kits(&p)?.into_iter().filter(|m| m != k.as_str()).collect();
    if attach {
        mine.push(k.as_str().into());
    }
    store.set_kits(&p, &mine)
}

pub fn kit_edit(kits: &dyn KitStore, editor: &dyn Editor, raw: &str) -> Result<(), CcsError> {
    editor.edit(&kits.instructions_file(&existing_kit(kits, raw)?))
}

// ── The TUI's driving port (ADR-2610261255, ADR-2610261643 §8) ───────────────

/// One profile's overview: segment, variables, kits and doctor findings.
pub fn overview(app: &App, name: &ProfileName) -> Result<Overview, CcsError> {
    let profile = app.store.load(name)?;
    let plan = plan_launch(&profile, vec![]);
    Ok(Overview {
        name: name.clone(),
        segment: status_segment(&plan.set).unwrap_or_default(),
        findings: doctor(app.store, app.kits, app.lib, app.legacy, name, app.claude_available)?,
        is_default: app.store.default_name().as_deref() == Some(name.as_str()),
        kits: app.store.kits(name),
        dir: app.store.folder(name),
        env: profile.env,
    })
}

/// The application as the TUI sees it: each method is one existing use case.
pub struct App<'a> {
    pub store: &'a dyn ProfileStore,
    pub kits: &'a dyn KitStore,
    pub lib: &'a dyn Library,
    pub legacy: &'a dyn Legacy,
    pub paths: &'a dyn PathInput,
    pub editor: &'a dyn Editor,
    pub aws: &'a dyn AwsProfiles,
    pub fetch: &'a dyn Fetcher,
    /// This program, for the status line command a new profile gets.
    pub ccs_bin: String,
    pub claude_available: bool,
    /// The resolved profile, marked in the list.
    pub current: Option<String>,
}

impl Browser for App<'_> {
    fn profiles(&self) -> Vec<ListEntry> {
        list(self.store, self.current.as_deref())
    }
    fn overview(&self, name: &ProfileName) -> Result<Overview, CcsError> {
        overview(self, name)
    }
    fn set_default(&self, name: &ProfileName) -> Result<(), CcsError> {
        set_default(self.store, name)
    }
    fn kits(&self) -> Vec<String> {
        self.kits.names().into_iter().map(|k| k.as_str().to_string()).collect()
    }
    fn attach(&self, kit: &str, profile: &ProfileName, on: bool) -> Result<(), CcsError> {
        kit_attach(self.store, self.kits, kit, profile.as_str(), on)
    }
    fn new_kit(&self, kit: &str) -> Result<(), CcsError> {
        kit_new(self.kits, self.lib, kit).map(|_| ())
    }
    fn remove_kit(&self, kit: &str) -> Result<std::path::PathBuf, CcsError> {
        kit_remove(self.store, self.kits, kit)
    }
    fn rename_kit(&self, kit: &str, to: &str) -> Result<(), CcsError> {
        kit_rename(self.store, self.kits, self.lib, kit, to)
    }
    fn kit_uses(&self, kit: &str) -> Result<Vec<Use>, CcsError> {
        self.kits.uses(&existing_kit(self.kits, kit)?)
    }
    fn set_use(&self, kit: &str, entry: &Use, on: bool) -> Result<(), CcsError> {
        if on {
            kit_use(self.kits, self.lib, kit, &[entry.line()])
        } else {
            kit_drop(self.kits, kit, &[entry.line()])
        }
    }
    fn kit_lines(&self, kit: &str) -> Result<Vec<LineGives>, CcsError> {
        kit_show(self.kits, self.lib, kit)
    }
    fn load_plan(&self, profile: &ProfileName) -> Result<LoadPlan, CcsError> {
        load_plan(self.store, self.kits, self.lib, profile, None)
    }
    fn plugin_contents(&self, plugin: &str) -> Result<PluginContents, CcsError> {
        Ok(self.lib.plugin_contents(&library_item(self.lib, &Use::Item(Kind::Plugin, plugin.into()))?))
    }
    fn kit_instructions(&self, kit: &str) -> Option<String> {
        self.kits.instructions(&existing_kit(self.kits, kit).ok()?)
    }
    fn edit_kit(&self, kit: &str) -> Result<(), CcsError> {
        kit_edit(self.kits, self.editor, kit)
    }
    fn library(&self, kind: Kind) -> (Vec<LibItem>, Vec<String>) {
        library_list(self.lib, kind)
    }
    fn read(&self, entry: &Use) -> Result<String, CcsError> {
        library_show(self.lib, entry)
    }
    fn edit(&self, entry: &Use) -> Result<(), CcsError> {
        library_edit(self.lib, self.editor, entry)
    }
    fn add(&self, path: &str, folder: &str) -> Result<(Kind, String), CcsError> {
        library_add_source(self.lib, self.fetch, path, &self.paths.expand(path), folder)
    }
    fn update_plugins(&self, which: Option<&str>) -> Result<Vec<String>, CcsError> {
        plugin_update(self.lib, self.fetch, which.map(|n| Use::Item(Kind::Plugin, n.into())).as_ref())
    }
    fn mkdir(&self, kind: Kind, folder: &str) -> Result<(), CcsError> {
        library_mkdir(self.lib, kind, folder)
    }
    fn move_entry(&self, entry: &Use, to: &str) -> Result<(), CcsError> {
        library_move(self.lib, self.kits, entry, to)
    }
    fn rename_folder(&self, kind: Kind, folder: &str, name: &str) -> Result<(), CcsError> {
        library_rename_folder(self.lib, self.kits, kind, folder, name)
    }
    fn remove(&self, entry: &Use) -> Result<LibraryRemoved, CcsError> {
        library_remove(self.lib, self.kits, entry)
    }
    fn set_kits(&self, profile: &ProfileName, kits: &[String]) -> Result<(), CcsError> {
        set_profile_kits(self.store, self.kits, profile, kits)
    }
    fn complete_path(&self, typed: &str) -> Vec<String> {
        self.paths.complete(typed)
    }
    fn new_profile(&self, name: &str, from: Option<&ProfileName>) -> Result<(), CcsError> {
        make_profile(self.store, name, from, &self.ccs_bin).map(|_| ())
    }
    fn edit_profile(&self, name: &ProfileName) -> Result<(), CcsError> {
        profile_edit(self.store, self.editor, name)
    }
    fn needs_migrate(&self) -> bool {
        needs_migrate(self.legacy)
    }
    fn migrate(&self) -> Result<Vec<String>, CcsError> {
        migrate(self.store, self.lib, self.kits, self.legacy)
    }
    fn import_claude(&self) -> Result<Vec<String>, CcsError> {
        import(self.lib, self.kits, self.legacy)
    }
    fn aws_profiles(&self) -> Result<Vec<AwsRow>, CcsError> {
        aws_status(self.store, self.aws)
    }
    fn import_aws(&self, names: &[String], all: bool) -> Result<Vec<ImportOutcome>, CcsError> {
        import_aws(self.store, self.aws, names, all, None, &self.ccs_bin)
    }
}

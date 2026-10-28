//! Use cases — one function per `ccs` command.
//!
//! Rule 3: `usecases/` imports `domain/` and `ports/` only. Each takes its
//! ports as parameters and never picks the adapter.

use crate::domain::{
    aws_profile_env, ccs_name_for_aws, checksum_for, git_source, newer_version, release_asset, launch_settings, next_statusline, settings_statusline, statusline_dir, check_env, compose_status, default_asset_name, default_settings, item_name,
    kit_args, kit_name_for_profile, plan_launch, provider_label, rewrite_folder, render_exports, resolve, resolve_kit, status_segment, with_kits, AwsRow,
    CcsError, Finding, ImportOutcome, Kind, KitContents, KitName, LaunchPlan, LegacyItem, LibItem, LibraryRemoved,
    ListEntry, Overview, Profile, ProfileName, Use, Wrapped,
};
use crate::ports::{
    AwsProfiles, Browser, CommandRunner, Editor, Fetcher, KitStore, Releases, TextFiles, Launcher, Legacy, Library, PathInput, PinLocator, ProfileStore,
};
use std::path::Path;

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

/// What to run for a profile with kits (ADR-2610261643): `chosen` kits, else the
/// profile's own. Each kit is rebuilt from the library. Returns the plan, and a
/// warning for each kit entry the library no longer has; those do not stop the launch.
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
    let wanted = chosen.unwrap_or_else(|| store.kits(name));
    let names = wanted.iter().map(|k| existing_kit(kits, k)).collect::<Result<Vec<_>, _>>()?;
    let library = library_items(lib);
    let (mut dirs, mut plugins, mut warnings, mut prompt) = (Vec::new(), Vec::<LibItem>::new(), Vec::new(), String::new());
    for k in &names {
        let contents = resolve_kit(&kits.uses(k)?, &library);
        warnings.extend(contents.missing.iter().map(|m| format!("kit {}: {m} is not in the library; left out", k.as_str())));
        // Skills and agents are built into the kit; a picked plugin loads as itself (§15).
        let (picked, own): (Vec<LibItem>, Vec<LibItem>) = contents.items.into_iter().partition(|i| i.kind == Kind::Plugin);
        dirs.push(kits.build(k, &own)?);
        for p in picked {
            if !plugins.iter().any(|x| x.name == p.name) {
                plugins.push(p);
            }
        }
        if let Some(text) = kits.instructions(k) {
            prompt += &format!("<!-- kit {} -->\n{}\n", k.as_str(), text.trim_end());
        }
    }
    for p in plugins.iter().filter(|p| names.iter().any(|k| k.as_str() == p.name)) {
        let n = &p.name;
        warnings.push(format!("kit {n} and plugin {n} both name their skills {n}:…; rename the kit: ccs kit rename {n} <new>"));
    }
    dirs.extend(plugins.into_iter().map(|p| p.source));
    let prompt = if prompt.is_empty() { None } else { Some(kits.write_prompt(&names, &prompt)?) };
    // The launch's own status line ranks above a project's, so the profile always shows.
    let extra = ["--settings".to_string(), launch_settings(ccs_bin)].into_iter().chain(kit_args(&dirs, prompt.as_deref())).collect();
    let plan = with_kits(plan_launch(&profile, args), &names, extra);
    Ok((plan, warnings))
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
        match existing_kit(kits, &k) {
            Err(_) => out.push(Finding::Fail(format!("kit '{k}' does not exist (see: ccs kit list)"))),
            Ok(kit) => {
                let contents = resolve_kit(&kits.uses(&kit)?, &library);
                out.extend(contents.missing.iter().map(|m| Finding::Warn(format!("kit {k}: {m} is not in the library"))));
                out.push(Finding::Pass(format!("kit {k}: {} items", contents.items.len())));
            }
        }
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

/// `ccs statusline --auto`, the status line every launch sets: the segment, then
/// the first other status line of the project's local settings, its shared
/// settings, or the profile's settings, run with the same stdin.
pub fn statusline_auto(runner: &dyn CommandRunner, files: &dyn TextFiles, vars: &[(String, String)], stdin: &[u8]) -> String {
    let payload = String::from_utf8_lossy(stdin);
    let mut places = Vec::new();
    if let Some(dir) = statusline_dir(&payload) {
        let dir = Path::new(&dir).join(".claude");
        places.push(dir.join("settings.local.json"));
        places.push(dir.join("settings.json"));
    }
    if let Some((_, cfg)) = vars.iter().find(|(k, v)| k == "CLAUDE_CONFIG_DIR" && !v.is_empty()) {
        places.push(Path::new(cfg).join("settings.json"));
    }
    let commands: Vec<Option<String>> = places.iter().map(|p| files.read(p).and_then(|t| settings_statusline(&t))).collect();
    let wrapped = match next_statusline(&commands) {
        None => Wrapped::Nothing,
        Some(cmd) => match runner.run(&["/bin/sh".into(), "-c".into(), cmd.clone()], stdin) {
            Some(out) => Wrapped::Output(out),
            None => Wrapped::Failed(cmd),
        },
    };
    compose_status(status_segment(vars), wrapped)
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
    compose_status(status_segment(vars), wrapped)
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
        store.write_env(&name, &aws_profile_env(&a, r.as_deref()))?;
        seed_statusline(store, &name, ccs_bin)?;
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
/// already has stays where it is, and a note names it. Returns which kinds landed.
/// `retire` moves each copied original to the trash.
fn adopt(
    lib: &dyn Library,
    legacy: &dyn Legacy,
    items: &[LegacyItem],
    base: &str,
    owner: &str,
    retire: bool,
    notes: &mut Vec<String>,
) -> Result<[bool; 2], CcsError> {
    let mut landed = [false, false];
    for i in items {
        let what = format!("{}/{}", i.kind.dir(), legacy_path(i));
        let folder = if i.folder.is_empty() { base.to_string() } else { format!("{base}/{}", i.folder) };
        if let Some(have) = lib.items(i.kind).into_iter().find(|x| x.name == i.name) {
            let have = format!("{}/{}", i.kind.dir(), have.path());
            notes.push(match (have.ends_with(&format!("{folder}/{}", i.name)), retire) {
                (true, _) => format!("{owner}: {what} already in the library"),
                // A copied source (~/.claude) keeps everything; nothing is "left" anywhere.
                (false, false) => format!("{owner}: {what} not imported: the library already has {have}"),
                (false, true) => format!("{owner}: {what} left in place: the library already has {have}"),
            });
            continue;
        }
        lib.add(i.kind, &folder, &i.name, &i.source)?;
        if retire {
            legacy.retire(&i.source)?;
        }
        landed[usize::from(i.kind == Kind::Agent)] = true;
        notes.push(format!("{owner}: {what} → library/{}/{folder}/", i.kind.dir()));
    }
    Ok(landed)
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
    let mut mine = store.kits(profile);
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
    adopt(lib, legacy, &items, "imported", "~/.claude", false, &mut notes)?;
    let kit = KitName::parse("imported")?;
    let kinds = [items.iter().any(|i| i.kind == Kind::Skill), items.iter().any(|i| i.kind == Kind::Agent)];
    let text = md.map(|m| legacy.read(&m)).transpose()?;
    fill_kit(kits, &kit, "imported", kinds, text)?;
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
        let landed = adopt(lib, legacy, items, "shared", "shared", false, &mut notes)?;
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
        for link in &p.links {
            legacy.retire(link)?;
        }
        if !p.unshared.is_empty() {
            legacy.retire(&p.dir.join("unshared"))?;
        }
        let kit = kit_name_for_profile(p.name.as_str());
        let landed = adopt(lib, legacy, &p.own, kit.as_str(), p.name.as_str(), true, &mut notes)?;
        let text = p.claude_md.as_ref().map(|m| legacy.read(m)).transpose()?;
        if landed.contains(&true) || text.is_some() {
            fill_kit(kits, &kit, kit.as_str(), landed, text)?;
            attach_last(store, &p.name, &kit)?;
            if let Some(m) = &p.claude_md {
                legacy.retire(m)?;
                notes.push(format!("{}: CLAUDE.md → kit {}", p.name.as_str(), kit.as_str()));
            }
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

/// Point every kit line under folder `from` at `to` (ADR-2610261643 §9).
fn rewrite_kits(kits: &dyn KitStore, kind: Kind, from: &str, to: &str) -> Result<(), CcsError> {
    for k in kits.names() {
        let uses = kits.uses(&k)?;
        let moved = rewrite_folder(&uses, kind, from, to);
        if moved != uses {
            kits.set_uses(&k, &moved)?;
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
    lib.move_to(kind, &from, to)?;
    if let Use::Folder(_, f) = what {
        let leaf = f.rsplit('/').next().unwrap_or(f);
        rewrite_kits(kits, kind, f, &if to.is_empty() { leaf.to_string() } else { format!("{to}/{leaf}") })?;
    }
    Ok(())
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
    lib.rename_folder(kind, folder, &name)?;
    let to = match folder.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/{name}"),
        None => name,
    };
    rewrite_kits(kits, kind, folder, &to)
}

/// Rename a kit, keeping its place in every profile's list.
pub fn kit_rename(store: &dyn ProfileStore, kits: &dyn KitStore, old: &str, new: &str) -> Result<(), CcsError> {
    let from = existing_kit(kits, old)?;
    let to = KitName::parse(new)?;
    if kits.exists(&to) {
        return Err(CcsError::Usage(format!("kit '{new}' already exists")));
    }
    kits.rename(&from, &to)?;
    for p in store.names() {
        let mine = store.kits(&p);
        if mine.iter().any(|k| k == old) {
            store.set_kits(&p, &mine.into_iter().map(|k| if k == old { new.to_string() } else { k }).collect::<Vec<_>>())?;
        }
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
    let users = kits
        .names()
        .into_iter()
        .filter(|k| {
            let uses = kits.uses(k).unwrap_or_default();
            uses.contains(what) || resolve_kit(&uses, &library).items.iter().any(|i| gone.contains(&i))
        })
        .map(|k| k.as_str().to_string())
        .collect();
    Ok(LibraryRemoved { trashed: lib.trash(kind, &path)?, kits: users })
}

pub fn library_show(lib: &dyn Library, what: &Use) -> Result<String, CcsError> {
    lib.read(&library_item(lib, what)?)
}

pub fn library_edit(lib: &dyn Library, editor: &dyn Editor, what: &Use) -> Result<(), CcsError> {
    editor.edit(&lib.main_file(&library_item(lib, what)?))
}

pub fn kit_new(kits: &dyn KitStore, raw: &str) -> Result<KitName, CcsError> {
    let k = KitName::parse(raw)?;
    if kits.exists(&k) {
        return Err(CcsError::Usage(format!("kit '{raw}' already exists")));
    }
    kits.create(&k)?;
    Ok(k)
}

/// Trash a kit and detach it from every profile.
pub fn kit_remove(store: &dyn ProfileStore, kits: &dyn KitStore, raw: &str) -> Result<std::path::PathBuf, CcsError> {
    let k = existing_kit(kits, raw)?;
    for p in store.names() {
        let mine = store.kits(&p);
        if mine.iter().any(|m| m == k.as_str()) {
            store.set_kits(&p, &mine.into_iter().filter(|m| m != k.as_str()).collect::<Vec<_>>())?;
        }
    }
    kits.remove(&k)
}

pub fn kit_show(kits: &dyn KitStore, lib: &dyn Library, raw: &str) -> Result<(Vec<Use>, KitContents), CcsError> {
    let k = existing_kit(kits, raw)?;
    let uses = kits.uses(&k)?;
    let contents = resolve_kit(&uses, &library_items(lib));
    Ok((uses, contents))
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
    for e in entries {
        let u = Use::parse(e)?;
        if !uses.contains(&u) {
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
    let mut mine: Vec<String> = store.kits(&p).into_iter().filter(|m| m != k.as_str()).collect();
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
        kit_new(self.kits, kit).map(|_| ())
    }
    fn remove_kit(&self, kit: &str) -> Result<std::path::PathBuf, CcsError> {
        kit_remove(self.store, self.kits, kit)
    }
    fn rename_kit(&self, kit: &str, to: &str) -> Result<(), CcsError> {
        kit_rename(self.store, self.kits, kit, to)
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

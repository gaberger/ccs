//! Domain — the rules of a profile switch. Pure data, no I/O.
//!
//! Rule 1: `domain/` imports only `domain/` and `std`. Everything here is
//! decided without touching a disk or a process, so all of it is testable
//! without wiring anything up.

use std::fmt;
use std::path::PathBuf;

/// Variables cleared before a profile's own apply (ADR-2610261212, rule 2), so a
/// Bedrock export in the parent shell cannot leak into a personal profile.
/// Secrets are deliberately absent: they belong to the user's credential setup.
pub const PROVIDER_VARS: &[&str] = &[
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
    "AWS_PROFILE",
    "AWS_REGION",
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_MODEL",
    "ANTHROPIC_SMALL_FAST_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "CLAUDE_CONFIG_DIR",
];

/// Words that are commands, retired commands, and ccs's own dirs. None can be a profile.
const RESERVED: &[&str] = &[
    "list", "ls", "new", "import", "default", "env", "link", "doctor", "which", "help", "statusline", "skill", "skills",
    "agent", "agents", "claude-md", "claudemd", "ui", "shared", "kit", "kits", "library", "lib", "migrate",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CcsError {
    BadName(String),
    EnvSyntax { line: usize },
    NoProfile(String),
    Exists(String),
    NoneResolved,
    Usage(String),
    Io(String),
    /// e.g. "skill 'demo'"
    AssetExists(String),
    /// e.g. "skill 'demo' in work"
    NoAsset(String),
    NoKit(String),
    /// A retired command, with what replaced it.
    Gone(String),
}

impl fmt::Display for CcsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CcsError::BadName(n) => write!(
                f,
                "bad profile name '{n}' (letters, digits, . _ - ; must not start with . or -; not a command word)"
            ),
            CcsError::EnvSyntax { line } => write!(f, "profile.env line {line}: expected KEY=VALUE"),
            CcsError::NoProfile(n) => write!(f, "no profile '{n}' (see: ccs list)"),
            CcsError::Exists(n) => write!(f, "profile '{n}' already exists"),
            CcsError::NoneResolved => {
                write!(f, "no profile chosen (pick one: ccs ui, or ccs <profile>; make one: ccs new <name>)")
            }
            CcsError::Usage(m) => write!(f, "usage: {m}"),
            CcsError::Io(m) => write!(f, "{m}"),
            CcsError::AssetExists(a) => write!(f, "{a} already exists there; remove it first (ccs … rm)"),
            CcsError::NoAsset(a) => write!(f, "no {a}"),
            CcsError::NoKit(k) => write!(f, "no kit '{k}' (see: ccs kit list)"),
            CcsError::Gone(m) => write!(f, "{m}"),
        }
    }
}

/// A profile name that is safe to use as one path segment under the profile root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProfileName(String);

impl ProfileName {
    pub fn parse(s: &str) -> Result<Self, CcsError> {
        let first_ok = s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric());
        let rest_ok = s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if first_ok && rest_ok && !RESERVED.contains(&s) {
            Ok(ProfileName(s.to_string()))
        } else {
            Err(CcsError::BadName(s.to_string()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A profile as loaded: its variables, and its config dir (login state only, ADR-2610261643).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: ProfileName,
    pub env: Vec<(String, String)>,
    pub config_dir: PathBuf,
}

/// Parse `profile.env`. It is data: lines are split, never evaluated (ADR rule 3).
pub fn parse_env(text: &str) -> Result<Vec<(String, String)>, CcsError> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let bad = || CcsError::EnvSyntax { line: i + 1 };
        let (key, val) = line.split_once('=').ok_or_else(bad)?;
        let key_ok = key.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !key_ok {
            return Err(bad());
        }
        out.push((key.to_string(), unquote(val).to_string()));
    }
    Ok(out)
}

fn unquote(v: &str) -> &str {
    for q in ['"', '\''] {
        if v.len() >= 2 && v.starts_with(q) && v.ends_with(q) {
            return &v[1..v.len() - 1];
        }
    }
    v
}

/// Resolution order (ADR rule 1): argument, pin file, `$CCS_PROFILE`, default.
/// A blank value falls through rather than naming a profile "".
pub fn resolve(
    explicit: Option<String>,
    pinned: Option<String>,
    env: Option<String>,
    default: Option<String>,
) -> Option<String> {
    [explicit, pinned, env, default]
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
}

/// What to do to the environment before `claude` replaces this process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub unset: Vec<String>,
    pub set: Vec<(String, String)>,
    pub args: Vec<String>,
}

pub fn plan_launch(profile: &Profile, args: Vec<String>) -> LaunchPlan {
    let mut set = profile.env.clone();
    set.push(("CLAUDE_CONFIG_DIR".into(), profile.config_dir.to_string_lossy().into_owned()));
    set.push(("CCS_PROFILE".into(), profile.name.as_str().into()));
    LaunchPlan { unset: PROVIDER_VARS.iter().map(|s| s.to_string()).collect(), set, args }
}

/// The plan as POSIX shell, for `eval "$(ccs env work)"`.
pub fn render_exports(plan: &LaunchPlan) -> String {
    let mut out = String::new();
    for k in &plan.unset {
        out += &format!("unset {k}\n");
    }
    for (k, v) in &plan.set {
        out += &format!("export {k}={}\n", shell_quote(v));
    }
    out
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// One row of `ccs list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEntry {
    pub name: ProfileName,
    pub current: bool,
    pub is_default: bool,
    /// The kits it launches with by default.
    pub kits: Vec<String>,
    /// e.g. `bedrock sso@us-east-1`, or `anthropic`.
    pub provider: String,
}

/// What the TUI shows for one profile (ADR-2610261255).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overview {
    pub name: ProfileName,
    /// The status line segment, e.g. `⬡ work · bedrock sso@us-east-1`.
    pub segment: String,
    pub env: Vec<(String, String)>,
    pub findings: Vec<Finding>,
    pub is_default: bool,
    pub kits: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Ui,
    /// `kits` is `Some` when `+kit` words chose them for this launch (ADR-2610261643).
    Launch { profile: Option<String>, kits: Option<Vec<String>>, args: Vec<String> },
    List,
    New { name: String, from: Option<String> },
    Import,
    Default(Option<String>),
    Env(Option<String>),
    Doctor(Option<String>),
    Which,
    Help,
    Statusline { wrap: Vec<String> },
    ImportAws { profiles: Vec<String>, all: bool, region: Option<String> },
    /// Convert the layout earlier versions left (ADR-2610261643 §5).
    Migrate,
    Library(LibraryCmd),
    Kit(KitCmd),
}

/// Leading `+kit` words choose the kits; everything after goes to `claude`.
fn launch_with_kits(profile: Option<String>, rest: &[String]) -> Command {
    let n = rest.iter().take_while(|a| a.len() > 1 && a.starts_with('+')).count();
    let kits = (n > 0).then(|| rest[..n].iter().map(|a| a[1..].to_string()).collect());
    Command::Launch { profile, kits, args: rest[n..].to_vec() }
}

// ── Skills and agents ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Skill,
    Agent,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Skill => "skill",
            Kind::Agent => "agent",
        }
    }

    /// `skills` or `agents`: the library dir, and the first part of an entry.
    pub fn dir(self) -> &'static str {
        match self {
            Kind::Skill => "skills",
            Kind::Agent => "agents",
        }
    }
}

fn segment_ok(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A path-safe item name: one segment of letters, digits, `.`, `_`, `-`.
pub fn item_name(kind: Kind, s: &str) -> Result<String, CcsError> {
    if segment_ok(s) {
        Ok(s.to_string())
    } else {
        Err(CcsError::Usage(format!("bad {} name '{s}' (letters, digits, . _ -)", kind.label())))
    }
}

/// The name an added item gets: its folder (a skill) or its file without `.md` (an agent).
pub fn default_asset_name(kind: Kind, source: &str) -> Option<String> {
    let mut parts: Vec<&str> = source.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if kind == Kind::Skill && parts.last() == Some(&"SKILL.md") {
        parts.pop();
    }
    let last = *parts.last()?;
    Some(match kind {
        Kind::Agent => last.strip_suffix(".md").unwrap_or(last).to_string(),
        _ => last.to_string(),
    })
}

/// The `description:` from YAML front matter: plain, quoted, or a `|` / `>` block.
pub fn front_matter_description(text: &str) -> Option<String> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let body: Vec<&str> = lines.take_while(|l| l.trim() != "---").collect();
    let i = body.iter().position(|l| l.starts_with("description:"))?;
    let value = body[i]["description:".len()..].trim();
    let desc = if value.starts_with('|') || value.starts_with('>') {
        body[i + 1..]
            .iter()
            .take_while(|l| l.is_empty() || l.starts_with(char::is_whitespace))
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        unquote(value).to_string()
    };
    (!desc.is_empty()).then_some(desc)
}

/// Parse the command line. A first word that names a profile launches it; any
/// other word that is not a command goes to `claude` untouched.
pub fn parse_command(args: &[String], is_profile: impl Fn(&str) -> bool) -> Result<Command, CcsError> {
    let first = args.first().map(String::as_str);
    let second = || args.get(1).cloned();
    Ok(match first {
        Some("list" | "ls") => Command::List,
        Some("import") => match args.get(1).map(String::as_str) {
            None => Command::Import,
            Some("aws") => parse_import_aws(&args[2..])?,
            Some(_) => return Err(CcsError::Usage("ccs import | ccs import aws [<aws-profile>…] [--all] [--region <r>]".into())),
        },
        Some("statusline") => match args.get(1).map(String::as_str) {
            None => Command::Statusline { wrap: vec![] },
            Some("--") if args.len() > 2 => Command::Statusline { wrap: args[2..].to_vec() },
            Some(_) => return Err(CcsError::Usage("ccs statusline [-- <command> [args…]]".into())),
        },
        Some("skill" | "skills" | "agent" | "agents") => return Err(CcsError::Gone(format!(
            "ccs {} is gone: skills and agents live in the library (ccs library …) and reach a profile through a kit (ccs kit …)",
            args[0]
        ))),
        Some("claude-md" | "claudemd") => {
            return Err(CcsError::Gone("ccs claude-md is gone: instructions belong to a kit; write them with: ccs kit edit <kit>".into()))
        }
        Some("link") => return Err(CcsError::Gone("ccs link is gone: kits are built at launch, so there is nothing to link".into())),
        Some("migrate") => Command::Migrate,
        Some("which") => Command::Which,
        Some("ui") => Command::Ui,
        Some("help" | "-h" | "--help") => Command::Help,
        Some("default") => Command::Default(second()),
        Some("env") => Command::Env(second()),
        Some("doctor") => Command::Doctor(second()),
        Some("new") => {
            let usage = || CcsError::Usage("ccs new <name> [--from <profile>]".into());
            let name = second().ok_or_else(usage)?;
            let from = match args.get(2).map(String::as_str) {
                None => None,
                Some("--from") => Some(args.get(3).cloned().ok_or_else(usage)?),
                Some(_) => return Err(usage()),
            };
            Command::New { name, from }
        }
        Some("library" | "lib") => parse_library(&args[1..])?,
        Some("kit" | "kits") => parse_kit(&args[1..])?,
        Some(p) if is_profile(p) => launch_with_kits(Some(p.into()), &args[1..]),
        _ => launch_with_kits(None, args),
    })
}

fn parse_import_aws(rest: &[String]) -> Result<Command, CcsError> {
    let usage = || CcsError::Usage("ccs import aws [<aws-profile>…] [--all] [--region <r>]".into());
    let (mut profiles, mut all, mut region) = (Vec::new(), false, None);
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--all" => all = true,
            "--region" => region = Some(it.next().ok_or_else(usage)?.clone()),
            s if s.starts_with('-') => return Err(usage()),
            s => profiles.push(s.to_string()),
        }
    }
    Ok(Command::ImportAws { profiles, all, region })
}

fn truthy(v: &str) -> bool {
    matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

// ── The library and kits (ADR-2610261643) ───────────────────────────────────

/// One line of a kit's `uses`, or an argument naming a library entry:
/// `skills/net/` is a folder (everything under it), `skills/humanize` an item, by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Use {
    Folder(Kind, String),
    Item(Kind, String),
}

impl Use {
    pub fn parse(s: &str) -> Result<Use, CcsError> {
        let bad = || CcsError::Usage(format!("'{s}' is not a library entry: skills/<name>, agents/<name>, or a folder like skills/net/"));
        let (kind, rest) = match s.split_once('/') {
            Some(("skills", r)) => (Kind::Skill, r),
            Some(("agents", r)) => (Kind::Agent, r),
            _ => return Err(bad()),
        };
        match rest.strip_suffix('/') {
            Some(folder) if folder_ok(folder) => Ok(Use::Folder(kind, folder.into())),
            None if segment_ok(rest) => Ok(Use::Item(kind, rest.into())),
            _ => Err(bad()),
        }
    }

    pub fn kind(&self) -> Kind {
        match self {
            Use::Folder(k, _) | Use::Item(k, _) => *k,
        }
    }

    pub fn line(&self) -> String {
        match self {
            Use::Folder(k, f) => format!("{}/{f}/", k.dir()),
            Use::Item(k, n) => format!("{}/{n}", k.dir()),
        }
    }
}

/// A folder path inside the library: one or more safe segments, `a/b`.
fn folder_ok(f: &str) -> bool {
    !f.is_empty() && f.split('/').all(segment_ok)
}

/// A skill or agent in the library. `folder` is `""` at the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibItem {
    pub kind: Kind,
    pub name: String,
    pub folder: String,
    pub description: Option<String>,
    /// The skill's folder or the agent's file.
    pub source: PathBuf,
}

impl LibItem {
    /// `net/nqe/find-nqe-query`.
    pub fn path(&self) -> String {
        if self.folder.is_empty() {
            self.name.clone()
        } else {
            format!("{}/{}", self.folder, self.name)
        }
    }

    pub fn in_folder(&self, folder: &str) -> bool {
        self.folder == folder || self.folder.starts_with(&format!("{folder}/"))
    }
}

/// What a kit's `uses` comes to against the library.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KitContents {
    /// In library order, each once.
    pub items: Vec<LibItem>,
    /// `uses` lines that match nothing in the library.
    pub missing: Vec<String>,
}

pub fn resolve_kit(uses: &[Use], library: &[LibItem]) -> KitContents {
    let picked = |i: &LibItem| {
        uses.iter().any(|u| match u {
            Use::Folder(k, f) => *k == i.kind && i.in_folder(f),
            Use::Item(k, n) => *k == i.kind && *n == i.name,
        })
    };
    let missing = uses
        .iter()
        .filter(|u| {
            !library.iter().any(|i| match u {
                Use::Folder(k, f) => *k == i.kind && i.in_folder(f),
                Use::Item(k, n) => *k == i.kind && *n == i.name,
            })
        })
        .map(Use::line)
        .collect();
    KitContents { items: library.iter().filter(|i| picked(i)).cloned().collect(), missing }
}

/// Words that read as a command after `ccs kit`, or name ccs's own dirs.
const KIT_RESERVED: &[&str] = &["kit", "kits", "library", "lib"];

/// A kit name: it becomes a plugin name and a skill prefix (`net:find-nqe-query`),
/// so lowercase letters, digits and `-`, starting with a letter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KitName(String);

impl KitName {
    pub fn parse(s: &str) -> Result<Self, CcsError> {
        let ok = s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !KIT_RESERVED.contains(&s);
        if ok {
            Ok(KitName(s.into()))
        } else {
            Err(CcsError::Usage(format!("bad kit name '{s}' (lowercase letters, digits and -, starting with a letter)")))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The kit a migrated profile's own items go into: the profile name, made a kit name.
pub fn kit_name_for_profile(profile: &str) -> KitName {
    let k: String = profile.to_ascii_lowercase().chars().map(|c| if c == '_' || c == '.' { '-' } else { c }).collect();
    KitName::parse(&k).unwrap_or_else(|_| KitName(format!("p-{k}")))
}

/// One row of a library tree: a folder (`item` is `None`) or an item, at its depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub path: String,
    pub depth: usize,
    pub item: Option<LibItem>,
}

impl TreeRow {
    pub fn leaf(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// The entry this row names: `skills/net/` or `skills/humanize`.
    pub fn entry(&self, kind: Kind) -> Use {
        match &self.item {
            Some(i) => Use::Item(kind, i.name.clone()),
            None => Use::Folder(kind, self.path.clone()),
        }
    }

    /// The folder something added at this row goes into.
    pub fn folder(&self) -> String {
        match &self.item {
            Some(i) => i.folder.clone(),
            None => self.path.clone(),
        }
    }
}

/// Folders and items in path order: a folder comes just before what is in it.
pub fn library_tree(items: &[LibItem], folders: &[String]) -> Vec<TreeRow> {
    let mut rows: Vec<TreeRow> = folders.iter().map(|f| TreeRow { path: f.clone(), depth: f.matches('/').count(), item: None }).collect();
    rows.extend(items.iter().map(|i| TreeRow { path: i.path(), depth: i.path().matches('/').count(), item: Some(i.clone()) }));
    rows.sort_by(|a, b| a.path.split('/').cmp(b.path.split('/')));
    rows
}

/// How a kit holds a tree row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// Its own line in `uses`.
    Direct,
    /// A folder above it is picked; holds that folder's entry line.
    Through(String),
    No,
}

pub fn pick(uses: &[Use], kind: Kind, row: &TreeRow) -> Pick {
    if uses.contains(&row.entry(kind)) {
        return Pick::Direct;
    }
    let inside = |f: &str| row.path.starts_with(&format!("{f}/"));
    uses.iter()
        .find_map(|u| match u {
            Use::Folder(k, f) if *k == kind && inside(f) => Some(Pick::Through(u.line())),
            _ => None,
        })
        .unwrap_or(Pick::No)
}

/// Kit lines after folder `from` became `to`: `from` itself and every folder under it.
/// Items are picked by name and stay as they are.
pub fn rewrite_folder(uses: &[Use], kind: Kind, from: &str, to: &str) -> Vec<Use> {
    uses.iter()
        .map(|u| match u {
            Use::Folder(k, f) if *k == kind && (f == from || f.starts_with(&format!("{from}/"))) => {
                Use::Folder(*k, format!("{to}{}", &f[from.len()..]))
            }
            _ => u.clone(),
        })
        .collect()
}

/// What removing a library entry did: where it went, and the kits that had picked from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRemoved {
    pub trashed: PathBuf,
    pub kits: Vec<String>,
}

// ── The layout earlier versions left (ADR-2610261643 §5, §8) ────────────────

/// A skill or agent found in an old `shared/`, a profile's config dir, or `~/.claude`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyItem {
    pub kind: Kind,
    pub name: String,
    /// Agent subfolders it sat in (`hex` for `agents/hex/rev.md`); `""` for skills.
    pub folder: String,
    pub source: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyProfile {
    pub name: ProfileName,
    pub dir: PathBuf,
    /// Real items in its config dir (not links).
    pub own: Vec<LegacyItem>,
    /// A real `CLAUDE.md` in its config dir.
    pub claude_md: Option<PathBuf>,
    /// ccs-made links into `shared/`, and the `unshared` file.
    pub links: Vec<PathBuf>,
    pub unshared: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LegacyLayout {
    /// `None` when there is no `shared/` dir.
    pub shared: Option<(PathBuf, Vec<LegacyItem>, Option<PathBuf>)>,
    pub profiles: Vec<LegacyProfile>,
    /// A `native` registration: its dir.
    pub native: Option<PathBuf>,
}

impl LegacyLayout {
    /// Nothing marks an old layout. Own items alone do not: earlier versions always
    /// made `shared/`, and the clashes `migrate` leaves in place must not keep it asking.
    pub fn is_empty(&self) -> bool {
        self.shared.is_none() && self.native.is_none() && self.profiles.iter().all(|p| p.links.is_empty() && p.unshared.is_empty())
    }
}

/// The `claude` arguments that load built kits and their joined instructions.
pub fn kit_args(plugin_dirs: &[PathBuf], prompt: Option<&std::path::Path>) -> Vec<String> {
    let mut out = Vec::new();
    for d in plugin_dirs {
        out.push("--plugin-dir".into());
        out.push(d.to_string_lossy().into_owned());
    }
    if let Some(p) = prompt {
        out.push("--append-system-prompt-file".into());
        out.push(p.to_string_lossy().into_owned());
    }
    out
}

/// Add the kits to a launch plan: their arguments go before the user's, and `CCS_KITS` names them.
pub fn with_kits(mut plan: LaunchPlan, kits: &[KitName], kit_args: Vec<String>) -> LaunchPlan {
    if !kits.is_empty() {
        let names: Vec<&str> = kits.iter().map(KitName::as_str).collect();
        plan.set.push(("CCS_KITS".into(), names.join(",")));
    }
    plan.args = kit_args.into_iter().chain(plan.args).collect();
    plan
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryCmd {
    /// `None` lists both kinds.
    List(Option<Kind>),
    /// Copy in a skill folder or an agent file, into `folder` (`""` = the top).
    Add { source: String, folder: String },
    Mkdir(Kind, String),
    /// Move an item or folder into `to` (`""` = the top), made if needed.
    Move { what: Use, to: String },
    /// Rename a folder in place: `skills/net/nqe/` to `queries`.
    Rename { folder: Use, to: String },
    Remove(Use),
    Show(Use),
    Edit(Use),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KitCmd {
    List,
    /// Open the kit's CLAUDE.md in the editor.
    Edit(String),
    New(String),
    Remove(String),
    Show(String),
    Use(String, Vec<String>),
    Drop(String, Vec<String>),
    Rename(String, String),
    Attach { kit: String, profile: String },
    Detach { kit: String, profile: String },
}

const LIBRARY_USAGE: &str = "ccs library list [skills|agents] | add <path> [--in <folder>] | mkdir skills/<folder> | mv <entry> <folder|/> | rename skills/<folder>/ <name> | rm <entry> | show <entry> | edit <entry>
  an entry is skills/<name>, agents/<name>, or a folder like skills/net/";
const KIT_USAGE: &str =
    "ccs kit list | new <kit> | rm <kit> | show <kit> | edit <kit> | rename <kit> <new> | use <kit> <entry>… | drop <kit> <entry>… | attach <kit> <profile> | detach <kit> <profile>";

/// `/` and `""` are the top of the library; a trailing `/` is optional.
fn dest_folder(s: &str) -> Result<String, CcsError> {
    let f = s.trim_end_matches('/');
    if f.is_empty() || folder_ok(f) {
        Ok(f.into())
    } else {
        Err(CcsError::Usage(format!("'{s}' is not a folder name")))
    }
}

fn parse_library(rest: &[String]) -> Result<Command, CcsError> {
    let usage = || CcsError::Usage(LIBRARY_USAGE.into());
    let words: Vec<&str> = rest.iter().map(String::as_str).collect();
    let entry = |s: &str| Use::parse(s);
    let cmd = match words.as_slice() {
        ["list" | "ls"] => LibraryCmd::List(None),
        ["list" | "ls", "skills"] => LibraryCmd::List(Some(Kind::Skill)),
        ["list" | "ls", "agents"] => LibraryCmd::List(Some(Kind::Agent)),
        ["add", src] => LibraryCmd::Add { source: src.to_string(), folder: String::new() },
        ["add", src, "--in", f] => LibraryCmd::Add { source: src.to_string(), folder: dest_folder(f)? },
        ["mkdir", f] => match entry(&format!("{}/", f.trim_end_matches('/')))? {
            Use::Folder(k, f) => LibraryCmd::Mkdir(k, f),
            Use::Item(..) => return Err(usage()),
        },
        ["mv" | "move", what, to] => LibraryCmd::Move { what: entry(what)?, to: dest_folder(to)? },
        ["rename", what, to] => match entry(what)? {
            u @ Use::Folder(..) if segment_ok(to) => LibraryCmd::Rename { folder: u, to: to.to_string() },
            Use::Folder(..) => return Err(CcsError::Usage(format!("'{to}' is not a folder name (letters, digits, . _ -)"))),
            Use::Item(..) => return Err(CcsError::Usage("only a folder is renamed; an item keeps its name, which kits pick it by".into())),
        },
        ["rm" | "remove" | "delete", what] => LibraryCmd::Remove(entry(what)?),
        ["show" | "cat", what] => match entry(what)? {
            u @ Use::Item(..) => LibraryCmd::Show(u),
            Use::Folder(..) => return Err(usage()),
        },
        ["edit", what] => match entry(what)? {
            u @ Use::Item(..) => LibraryCmd::Edit(u),
            Use::Folder(..) => return Err(usage()),
        },
        _ => return Err(usage()),
    };
    Ok(Command::Library(cmd))
}

fn parse_kit(rest: &[String]) -> Result<Command, CcsError> {
    let usage = || CcsError::Usage(KIT_USAGE.into());
    let words: Vec<&str> = rest.iter().map(String::as_str).collect();
    let many = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let cmd = match words.as_slice() {
        ["list" | "ls"] => KitCmd::List,
        ["new", k] => KitCmd::New(k.to_string()),
        ["rm" | "remove" | "delete", k] => KitCmd::Remove(k.to_string()),
        ["show", k] => KitCmd::Show(k.to_string()),
        ["edit", k] => KitCmd::Edit(k.to_string()),
        ["rename", old, new] => KitCmd::Rename(old.to_string(), new.to_string()),
        ["use" | "add", k, entries @ ..] if !entries.is_empty() => KitCmd::Use(k.to_string(), many(entries)),
        ["drop", k, entries @ ..] if !entries.is_empty() => KitCmd::Drop(k.to_string(), many(entries)),
        ["attach", k, p] => KitCmd::Attach { kit: k.to_string(), profile: p.to_string() },
        ["detach", k, p] => KitCmd::Detach { kit: k.to_string(), profile: p.to_string() },
        _ => return Err(usage()),
    };
    Ok(Command::Kit(cmd))
}

/// Which backend a set of variables selects, and its account: `bedrock sso@us-east-1`,
/// `vertex`, `foundry`, `gateway`, or `anthropic`.
pub fn provider_label(vars: &[(String, String)]) -> String {
    let get = |k: &str| vars.iter().find(|(key, _)| key == k).map(|(_, v)| v.trim()).filter(|v| !v.is_empty());
    if get("CLAUDE_CODE_USE_BEDROCK").is_some_and(truthy) {
        let mut s = "bedrock".to_string();
        match (get("AWS_PROFILE"), get("AWS_REGION")) {
            (Some(p), Some(r)) => s += &format!(" {p}@{r}"),
            (Some(p), None) => s += &format!(" {p}"),
            (None, Some(r)) => s += &format!(" @{r}"),
            (None, None) => {}
        }
        s
    } else if get("CLAUDE_CODE_USE_VERTEX").is_some_and(truthy) {
        "vertex".into()
    } else if get("CLAUDE_CODE_USE_FOUNDRY").is_some_and(truthy) {
        "foundry".into()
    } else if get("ANTHROPIC_BASE_URL").is_some() {
        "gateway".into()
    } else {
        "anthropic".into()
    }
}

/// The status line segment (ADR-2610261237). `None` when `claude` was not started by `ccs`.
pub fn status_segment(vars: &[(String, String)]) -> Option<String> {
    let get = |k: &str| vars.iter().find(|(key, _)| key == k).map(|(_, v)| v.trim()).filter(|v| !v.is_empty());
    let profile = get("CCS_PROFILE")?;
    let provider = provider_label(vars);
    let kits: String = get("CCS_KITS").map(|k| k.split(',').filter(|k| !k.is_empty()).map(|k| format!(" +{k}")).collect()).unwrap_or_default();
    Some(format!("⬡ {profile}{kits} · {provider}"))
}

/// What a wrapped status line command produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wrapped {
    Nothing,
    Output(String),
    /// The command could not start. Holds the command as typed.
    Failed(String),
}

/// Put the segment in front of the first line of the wrapped output.
pub fn compose_status(segment: Option<String>, wrapped: Wrapped) -> String {
    let theirs = match wrapped {
        Wrapped::Nothing => String::new(),
        Wrapped::Output(s) => s.trim_end().to_string(),
        Wrapped::Failed(cmd) => format!("⚠ statusline failed: {cmd}"),
    };
    match (segment, theirs.is_empty()) {
        (Some(seg), true) => seg,
        (Some(seg), false) => format!("{seg} │ {theirs}"),
        (None, _) => theirs,
    }
}

/// `settings.json` for a fresh profile: the `ccs` status line, nothing else.
pub fn default_settings(ccs_bin: &str) -> String {
    let command = format!("{} statusline", shell_quote(ccs_bin));
    format!(
        "{{\n  \"statusLine\": {{\n    \"type\": \"command\",\n    \"command\": \"{}\"\n  }}\n}}\n",
        json_escape(&command)
    )
}

fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out += "\\\"",
            '\\' => out += "\\\\",
            c if u32::from(c) < 0x20 => out += &format!("\\u{:04x}", u32::from(c)),
            c => out.push(c),
        }
    }
    out
}

/// The `ccs` profile name for an AWS profile (ADR-2610261238, rule 3).
pub fn ccs_name_for_aws(aws: &str) -> Result<ProfileName, CcsError> {
    ProfileName::parse(if aws == "default" { "aws-default" } else { aws })
}

/// `profile.env` for an imported AWS profile. No region leaves the line commented,
/// so `ccs doctor` fails the profile until one is set.
pub fn aws_profile_env(aws: &str, region: Option<&str>) -> String {
    let region_line = match region {
        Some(r) => format!("AWS_REGION={r}"),
        None => "# AWS_REGION=   <- no region in the AWS config; set one".into(),
    };
    format!(
        "# Imported from the AWS CLI profile '{aws}' by `ccs import aws`.\n\
         CLAUDE_CODE_USE_BEDROCK=1\nAWS_PROFILE={aws}\n{region_line}\n\
         # ANTHROPIC_MODEL=us.anthropic.claude-opus-5-5-v1:0\n"
    )
}

/// One AWS CLI profile and the `ccs` profile that uses it, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwsRow {
    pub aws: String,
    pub imported_as: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportOutcome {
    Created { name: String, region: Option<String> },
    Skipped { aws: String, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    Pass(String),
    Warn(String),
    Fail(String),
}

/// Doctor rules that need only the profile's variables.
pub fn check_env(env: &[(String, String)]) -> Vec<Finding> {
    let has = |k: &str| env.iter().any(|(key, v)| key == k && !v.is_empty());
    let keys: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
    let mut out = vec![Finding::Pass(format!(
        "profile.env parses ({})",
        if keys.is_empty() { "no variables".into() } else { keys.join(" ") }
    ))];
    if env.iter().any(|(k, v)| k == "CLAUDE_CODE_USE_BEDROCK" && truthy(v)) {
        if !has("AWS_REGION") {
            out.push(Finding::Fail("Bedrock profile without AWS_REGION".into()));
        }
        if !has("AWS_PROFILE") {
            out.push(Finding::Warn("Bedrock profile without AWS_PROFILE (uses the default AWS chain)".into()));
        }
    }
    out
}

pub const HELP: &str = "\
ccs — Claude Code profile switcher

A profile is a login. A kit is a set of skills, agents and instructions.
A launch is a profile plus kits. Nothing starts until you choose a profile.

  ccs                            open the picker (when no profile is chosen)
  ccs <profile> [+kit…] [args…]  start claude with <profile>; +kit replaces its kits for this run
  ccs list                       list profiles with their kits (* = resolved)
  ccs new <name> [--from <p>]    make a profile (copies variables, kits, settings; never login)
  ccs default [<name>]           show or set the default profile
  ccs env [<name>]               print exports: eval \"$(ccs env work)\"
  ccs doctor [<name>]            check a profile and its kits
  ccs which                      print the resolved profile
  ccs ui                         browse profiles, kits and the library; launch from the list
  ccs statusline [-- <cmd…>]     status line segment; wraps an existing status line command
  ccs import                     copy ~/.claude skills, agents, CLAUDE.md into the library (kit 'imported')
  ccs import aws [<p>…] [--all] [--region <r>]
                                 list AWS CLI profiles, or import them as Bedrock profiles
  ccs migrate                    convert the shared/ and native layout of earlier versions

  ccs library list [skills|agents] | add <path> [--in <folder>] | mkdir skills/<folder>
              mv <entry> <folder|/> | rename skills/<folder>/ <name> | rm <entry> | show <entry> | edit <entry>
  ccs kit     list | new <k> | rm <k> | show <k> | edit <k> | rename <k> <new> | use <k> <entry>… | drop <k> <entry>…
              attach <k> <profile> | detach <k> <profile>
                                 an entry is skills/<name>, agents/<name>, or a folder: skills/net/
                                 edit opens $VISUAL, else $EDITOR, else vi; rm moves to $CCS_HOME/.trash

Resolution order: argument, .claude-profile (walking up), $CCS_PROFILE, default.
Everything lives in $CCS_HOME (default ~/.claude-profiles).
";

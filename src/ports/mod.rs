//! Ports — the contracts between layers.
//!
//! Rule 2: `ports/` imports `domain/` only, for value types. A port names
//! *what* is needed, never *how*: `ProfileStore` says nothing about symlinks
//! or directories, which is why a test can swap it for a temp dir.

use crate::domain::{
    AwsRow, CcsError, ImportOutcome, Kind, KitName, LaunchPlan, LegacyItem, LegacyLayout, LibItem, LibraryRemoved, ListEntry, Overview, Profile,
    ProfileName, Use,
};

/// Re-exported so an adapter needs to import nothing but this module (rule 4).
pub use crate::domain::{
    front_matter_description, front_matter_name, manifest_field, pick_in, rank, resolve_kit, Found, LineGives, LoadPlan, PlannedKit, PluginContents, PluginPart, AwsRow as AwsProfileRow, ImportOutcome as AwsImport, library_tree, parse_env as parse_profile_env, pick, CcsError as Error, Finding,
    Kind as ItemKind, KitName as Kit, LaunchPlan as Plan, LegacyItem as OldItem, LegacyLayout as OldLayout,
    LegacyProfile as OldProfile, LibItem as LibraryItem, LibraryRemoved as Removed, ListEntry as ProfileRow,
    Overview as ProfileOverview, Pick, Profile as LoadedProfile, ProfileName as Name, TreeRow, Use as Entry,
};

/// Where profiles are kept. A profile is a login: its variables, its default
/// kits, and a config dir that holds login state only (ADR-2610261643).
pub trait ProfileStore {
    /// Every profile, sorted.
    fn names(&self) -> Vec<ProfileName>;
    fn exists(&self, name: &ProfileName) -> bool;
    fn load(&self, name: &ProfileName) -> Result<Profile, CcsError>;
    /// Make an empty profile, or one with `from`'s variables, kits and settings — never its login.
    fn create(&self, name: &ProfileName, from: Option<&ProfileName>) -> Result<(), CcsError>;
    /// Undo a `create(name, None)` that could not be finished: remove `profile.env`,
    /// so `name` is no longer a profile, and the folders if that empties them.
    /// Anything else in the folder, such as a login, stays.
    fn discard(&self, name: &ProfileName) -> Result<(), CcsError>;
    fn default_name(&self) -> Option<String>;
    fn set_default(&self, name: &ProfileName) -> Result<(), CcsError>;
    fn clear_default(&self) -> Result<(), CcsError>;
    /// The profile's folder: `profile.env`, `kits`, `config/`.
    fn folder(&self, name: &ProfileName) -> std::path::PathBuf;
    /// The profile's variables file, `profile.env`.
    fn env_file(&self, name: &ProfileName) -> std::path::PathBuf;
    /// Replace the profile's variables file.
    fn write_env(&self, name: &ProfileName, text: &str) -> Result<(), CcsError>;
    /// Write the profile's `settings.json` unless one exists. Returns whether it wrote.
    fn write_settings_if_absent(&self, name: &ProfileName, text: &str) -> Result<bool, CcsError>;
    /// The kits a profile launches with by default, in order. No file is no
    /// kits; a file that cannot be read is an error.
    fn read_kits(&self, name: &ProfileName) -> Result<Vec<String>, CcsError>;
    /// `read_kits`, with an unreadable file shown as no kits. Only for display:
    /// never write back what this returns.
    fn kits(&self, name: &ProfileName) -> Vec<String> {
        self.read_kits(name).unwrap_or_default()
    }
    fn set_kits(&self, name: &ProfileName, kits: &[String]) -> Result<(), CcsError>;
}

/// Every skill and agent, kept in folders, and every plugin, which has none (ADR-2610261643).
pub trait Library {
    /// Sorted by folder, then name.
    fn items(&self, kind: Kind) -> Vec<LibItem>;
    /// Every folder path, empty ones included. Sorted.
    fn folders(&self, kind: Kind) -> Vec<String>;
    /// What `source` would be added as: a skill (its folder; `SKILL.md` names its folder
    /// too) or an agent (`.md` file). An error says what it is not.
    fn classify(&self, source: &std::path::Path) -> Result<(Kind, std::path::PathBuf), CcsError>;
    /// Copy a skill folder or agent file in as `folder/name`, making the folder.
    fn add(&self, kind: Kind, folder: &str, name: &str, source: &std::path::Path) -> Result<(), CcsError>;
    fn make_folder(&self, kind: Kind, folder: &str) -> Result<(), CcsError>;
    /// A plugin folder's manifest name, from `.claude-plugin/plugin.json`.
    fn plugin_name(&self, dir: &std::path::Path) -> Result<String, CcsError>;
    /// The item's main file: a skill's `SKILL.md`, an agent's `.md`, a plugin's `plugin.json`.
    fn main_file(&self, item: &LibItem) -> std::path::PathBuf;
    fn read(&self, item: &LibItem) -> Result<String, CcsError>;
    /// Move an item (`folder/name`) or a folder into `to` (`""` is the top), making `to`.
    fn move_to(&self, kind: Kind, from: &str, to: &str) -> Result<(), CcsError>;
    /// Rename a folder where it is. Refuses a name already taken there.
    fn rename_folder(&self, kind: Kind, folder: &str, name: &str) -> Result<(), CcsError>;
    /// Move an item or folder (`folder/name` or a folder path) to the trash, whole.
    fn trash(&self, kind: Kind, path: &str) -> Result<std::path::PathBuf, CcsError>;
    /// The skills and agents inside a plugin (ADR-2610281305 R5). Empty for
    /// anything that is not a plugin.
    fn plugin_contents(&self, plugin: &LibItem) -> crate::domain::PluginContents;
}

/// Kits: named picks from the library, built into plugins at launch (ADR-2610261643).
pub trait KitStore {
    fn names(&self) -> Vec<KitName>;
    fn exists(&self, kit: &KitName) -> bool;
    fn create(&self, kit: &KitName) -> Result<(), CcsError>;
    /// To the trash, whole.
    fn remove(&self, kit: &KitName) -> Result<std::path::PathBuf, CcsError>;
    /// Refuses a name already taken.
    fn rename(&self, kit: &KitName, to: &KitName) -> Result<(), CcsError>;
    fn uses(&self, kit: &KitName) -> Result<Vec<Use>, CcsError>;
    fn set_uses(&self, kit: &KitName, uses: &[Use]) -> Result<(), CcsError>;
    /// The kit's instructions, when it has any.
    fn instructions(&self, kit: &KitName) -> Option<String>;
    fn set_instructions(&self, kit: &KitName, text: &str) -> Result<(), CcsError>;
    /// The file `ccs kit edit` opens; it may not exist yet.
    fn instructions_file(&self, kit: &KitName) -> std::path::PathBuf;
    /// Rebuild the kit as a plugin holding exactly `items`. Returns `build_dir(kit)`.
    fn build(&self, kit: &KitName, items: &[LibItem]) -> Result<std::path::PathBuf, CcsError>;
    /// Write one launch's joined instructions. Returns `prompt_path(kits)`.
    fn write_prompt(&self, kits: &[KitName], text: &str) -> Result<std::path::PathBuf, CcsError>;
    /// Where `build` puts the kit. Touches nothing, so a launch can be planned
    /// without building (ADR-2610281305 R2).
    fn build_dir(&self, kit: &KitName) -> std::path::PathBuf;
    /// Where `write_prompt` puts these kits' joined instructions. Touches nothing.
    fn prompt_path(&self, kits: &[KitName]) -> std::path::PathBuf;
}

/// Gets plugins from git (ADR-2610261643 §15).
pub trait Fetcher {
    /// Clone `url` into a new temporary folder. Returns the folder.
    fn clone_temp(&self, url: &str) -> Result<std::path::PathBuf, CcsError>;
    /// Remove a folder `clone_temp` made.
    fn discard(&self, dir: &std::path::Path);
    /// `git pull --ff-only` in `dir`: what changed, or `None` when `dir` is not a clone.
    fn pull(&self, dir: &std::path::Path) -> Result<Option<String>, CcsError>;
}

/// What earlier versions left, and the stock `~/.claude` (ADR-2610261643 §5).
/// Read-only except `retire`, which moves an old path to the trash.
pub trait Legacy {
    fn layout(&self) -> LegacyLayout;
    /// Skills and agents in `~/.claude`, and its `CLAUDE.md`. Never written.
    fn stock(&self) -> (Vec<LegacyItem>, Option<std::path::PathBuf>);
    fn read(&self, path: &std::path::Path) -> Result<String, CcsError>;
    /// Move a path ccs made to the trash, whole; a link is removed. Returns where it went.
    fn retire(&self, path: &std::path::Path) -> Result<Option<std::path::PathBuf>, CcsError>;
}

/// Paths the user types into the TUI (ADR-2610261933).
pub trait PathInput {
    /// A leading `~` is the home dir; a relative path starts where `ccs` started.
    fn expand(&self, typed: &str) -> std::path::PathBuf;
    /// What `typed` can complete to, spelled as typed (a `~` stays). Folders end in `/`.
    /// Dotfiles only when the last part starts with `.`. Sorted.
    fn complete(&self, typed: &str) -> Vec<String>;
}

/// Opens a file in the user's editor and waits for it (ADR-2610261643).
pub trait Editor {
    /// Returns once the editor exits; an editor that fails or cannot start is an error.
    fn edit(&self, path: &std::path::Path) -> Result<(), CcsError>;
}

/// Published releases of ccs, and putting one in place (ADR-2610271000).
pub trait Releases {
    /// The newest release's tag, like `v0.2.0`.
    fn latest(&self) -> Result<String, CcsError>;
    /// Download `assets` of release `tag` into a new folder. Returns the folder.
    fn download(&self, tag: &str, assets: &[String]) -> Result<std::path::PathBuf, CcsError>;
    fn read(&self, file: &std::path::Path) -> Result<String, CcsError>;
    /// The file's SHA-256, lower-case hex.
    fn sha256(&self, file: &std::path::Path) -> Result<String, CcsError>;
    /// Unpack the `ccs` binary from an archive. Returns it.
    fn unpack(&self, archive: &std::path::Path) -> Result<std::path::PathBuf, CcsError>;
    /// Run `binary version`. Returns what it printed; an error when it did not run.
    fn probe(&self, binary: &std::path::Path) -> Result<String, CcsError>;
    /// Put `binary` in place of the running ccs, whole or not at all. Returns where.
    fn install(&self, binary: &std::path::Path) -> Result<std::path::PathBuf, CcsError>;
    /// Remove a folder `download` made. Called after every attempt, so nothing is left.
    fn discard(&self, dir: &std::path::Path);
}

/// Reads text files that may not exist: the settings a status line looks through.
pub trait TextFiles {
    fn read(&self, path: &std::path::Path) -> Option<String>;
}

/// Runs another program, for wrapping an existing status line command.
pub trait CommandRunner {
    /// Run `argv` with `stdin`; return its stdout, or `None` when it could not start.
    fn run(&self, argv: &[String], stdin: &[u8]) -> Option<String>;
}

/// The AWS CLI profiles on this machine (ADR-2610261238).
pub trait AwsProfiles {
    fn names(&self) -> Result<Vec<String>, CcsError>;
    fn region(&self, profile: &str) -> Option<String>;
}

/// Finds a profile pinned to the working directory.
pub trait PinLocator {
    fn pinned(&self) -> Option<String>;
}

/// The shell's command history (ADR-2610281500).
pub trait History {
    /// Append `line` as if it had been typed. False when the shell is not one
    /// ccs knows, or the file cannot be written; the launch goes on either way.
    fn record(&self, line: &str) -> bool;
}

/// Puts a line in the terminal's input, so the shell reads it as typed (ADR-2610281500 §2).
pub trait Typer {
    /// False when there is no terminal, the terminal refuses, or `line` holds a
    /// control character. Nothing is typed then.
    fn type_line(&self, line: &str) -> bool;
}

/// Starts Claude Code.
pub trait Launcher {
    fn available(&self) -> bool;
    /// Replace this process. Returns only when that failed.
    fn launch(&self, plan: &LaunchPlan) -> CcsError;
}

/// Driving port: everything the TUI can ask of the application (ADR-2610261255,
/// rows and keys in ADR-2610261643 §8). A primary adapter sees only this.
pub trait Browser {
    fn profiles(&self) -> Vec<ListEntry>;
    fn overview(&self, name: &ProfileName) -> Result<Overview, CcsError>;
    fn set_default(&self, name: &ProfileName) -> Result<(), CcsError>;
    fn kits(&self) -> Vec<String>;
    fn attach(&self, kit: &str, profile: &ProfileName, on: bool) -> Result<(), CcsError>;
    fn new_kit(&self, kit: &str) -> Result<(), CcsError>;
    fn remove_kit(&self, kit: &str) -> Result<std::path::PathBuf, CcsError>;
    fn rename_kit(&self, kit: &str, to: &str) -> Result<(), CcsError>;
    fn kit_uses(&self, kit: &str) -> Result<Vec<Use>, CcsError>;
    /// Each line of a kit, with what it gives (ADR-2610281305).
    fn kit_lines(&self, kit: &str) -> Result<Vec<crate::domain::LineGives>, CcsError>;
    /// What a profile loads: the plan its launch uses. Builds nothing.
    fn load_plan(&self, profile: &ProfileName) -> Result<crate::domain::LoadPlan, CcsError>;
    /// The skills and agents inside a library plugin.
    fn plugin_contents(&self, plugin: &str) -> Result<crate::domain::PluginContents, CcsError>;
    /// Add or remove one line of a kit's `uses`.
    fn set_use(&self, kit: &str, entry: &Use, on: bool) -> Result<(), CcsError>;
    fn kit_instructions(&self, kit: &str) -> Option<String>;
    /// Open a kit's CLAUDE.md in the editor. The caller hands over the terminal first.
    fn edit_kit(&self, kit: &str) -> Result<(), CcsError>;
    /// Items and folders of one kind.
    fn library(&self, kind: Kind) -> (Vec<LibItem>, Vec<String>);
    fn read(&self, entry: &Use) -> Result<String, CcsError>;
    /// Open an item's main file in the editor. The caller hands over the terminal first.
    fn edit(&self, entry: &Use) -> Result<(), CcsError>;
    /// Copy in the file or folder at a typed path, into `folder`; or clone a plugin
    /// from a typed git source (`owner/repo`, a URL).
    fn add(&self, path: &str, folder: &str) -> Result<(Kind, String), CcsError>;
    /// `git pull` one plugin by name, or every plugin. One line each.
    fn update_plugins(&self, which: Option<&str>) -> Result<Vec<String>, CcsError>;
    fn mkdir(&self, kind: Kind, folder: &str) -> Result<(), CcsError>;
    fn move_entry(&self, entry: &Use, to: &str) -> Result<(), CcsError>;
    fn rename_folder(&self, kind: Kind, folder: &str, name: &str) -> Result<(), CcsError>;
    fn remove(&self, entry: &Use) -> Result<LibraryRemoved, CcsError>;
    /// Set a profile's default kits, in order.
    fn set_kits(&self, profile: &ProfileName, kits: &[String]) -> Result<(), CcsError>;
    fn complete_path(&self, typed: &str) -> Vec<String>;
    /// `ccs new`: an empty profile, or a copy of `from`'s variables, kits and settings.
    fn new_profile(&self, name: &str, from: Option<&ProfileName>) -> Result<(), CcsError>;
    /// Open a profile's `profile.env` in the editor. The caller hands over the terminal first.
    fn edit_profile(&self, name: &ProfileName) -> Result<(), CcsError>;
    /// Whether an earlier ccs left a layout for `migrate`.
    fn needs_migrate(&self) -> bool;
    /// `ccs migrate`. Returns its notes.
    fn migrate(&self) -> Result<Vec<String>, CcsError>;
    /// `ccs import`: `~/.claude` into the library and kit `imported`. Returns its notes.
    fn import_claude(&self) -> Result<Vec<String>, CcsError>;
    /// `ccs import aws`, listing: every AWS CLI profile and the ccs profile using it.
    fn aws_profiles(&self) -> Result<Vec<AwsRow>, CcsError>;
    /// `ccs import aws <p>…` or, with `all`, `--all`.
    fn import_aws(&self, names: &[String], all: bool) -> Result<Vec<ImportOutcome>, CcsError>;
}

//! Secondary adapters — driven by the application.
//!
//! Rule 4: an adapter imports `ports/` only, never another adapter. Rule 5 is
//! the other half: nothing imports these except the composition root.

pub mod aws_cli;
pub mod exec_launcher;
pub mod curl_releases;
pub mod fs_kits;
pub mod fs_legacy;
pub mod fs_library;
pub mod fs_paths;
pub mod fs_history;
pub mod fs_pins;
pub mod fs_store;
pub mod fs_text;
pub mod git_fetcher;
pub mod process_editor;
pub mod process_runner;
pub mod tty_typer;

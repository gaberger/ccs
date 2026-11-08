//! Gate for ADR-2610281500: a launch chosen in the menu is recorded as the
//! command line that would have launched it. Written before the code.

use ccs::domain::command_line;
use ccs::ports::History as _;
use ccs::history;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn the_menu_choice_becomes_the_line_you_would_have_typed() {
    assert_eq!(command_line("work", None, &[]), "ccs work");
    assert_eq!(command_line("work", Some(&s(&["net", "base"])), &s(&["--allow-all"])), "ccs work +net +base --allow-all");
}

#[test]
fn arguments_are_quoted_so_the_line_runs_the_same() {
    assert_eq!(command_line("work", None, &s(&["-p", "hi there", "it's"])), "ccs work -p 'hi there' 'it'\\''s'");
    assert_eq!(command_line("work", None, &s(&[""])), "ccs work ''");
}

#[test]
fn the_recorded_line_parses_back_to_the_same_launch() {
    // Independent oracle: the real parser, not the formatter's own rules.
    let kits = s(&["net"]);
    let args = s(&["-p", "hi there"]);
    let line = command_line("work", Some(&kits), &args);
    let rest = line.strip_prefix("ccs ").unwrap();
    let out = std::process::Command::new("sh")
        .args(["-c", &format!("for a in {rest}; do printf '%s\\n' \"$a\"; done")])
        .output()
        .unwrap();
    let words: Vec<String> = String::from_utf8(out.stdout).unwrap().lines().map(String::from).collect();
    assert_eq!(words, s(&["work", "+net", "-p", "hi there"]));
}

fn tmp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ccs-hist-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn zsh_history_gets_the_extended_format() {
    let d = tmp("zsh");
    let h = history("/bin/zsh", &d, None);
    assert!(h.record("ccs work +net"));
    let text = std::fs::read_to_string(d.join(".zsh_history")).unwrap();
    let line = text.lines().last().unwrap();
    assert!(line.starts_with(": ") && line.ends_with(":0;ccs work +net"), "{line}");
}

#[test]
fn bash_history_gets_the_plain_line_and_histfile_wins() {
    let d = tmp("bash");
    let h = history("/bin/bash", &d, None);
    assert!(h.record("ccs work"));
    assert_eq!(std::fs::read_to_string(d.join(".bash_history")).unwrap(), "ccs work\n");
    let custom = d.join("elsewhere");
    let h = history("/bin/bash", &d, Some(custom.clone()));
    assert!(h.record("ccs a"));
    assert_eq!(std::fs::read_to_string(custom).unwrap(), "ccs a\n");
}

#[test]
fn an_unknown_shell_records_nothing_and_says_so() {
    let d = tmp("fish");
    let h = history("/usr/bin/fish", &d, None);
    assert!(!h.record("ccs work"));
    assert!(std::fs::read_dir(&d).unwrap().next().is_none());
}

#[test]
fn a_multiline_value_cannot_forge_a_second_history_entry() {
    let d = tmp("nl");
    let h = history("/bin/bash", &d, None);
    assert!(h.record("ccs work 'a\nrm -rf x'"));
    assert_eq!(std::fs::read_to_string(d.join(".bash_history")).unwrap().lines().count(), 1);
}

// ── Typing the line into the terminal (ADR-2610281500 §2) ───────────────────

use ccs::ports::Typer as _;

#[test]
fn nothing_is_typed_when_there_is_no_terminal() {
    // `cargo test` has no controlling tty on stdin: the typer must say no, not pretend.
    assert!(!ccs::typer().type_line("ccs work"));
}

#[test]
fn a_line_with_a_control_character_is_never_typed() {
    // A newline would run the line early; ESC could drive the shell's editor.
    assert!(!ccs::typer().type_line("ccs work 'a\nb'"));
    assert!(!ccs::typer().type_line("ccs work \x1b[A"));
}

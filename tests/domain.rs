//! Gate for the pure rules of ADR-2610261212. No disk, no process.
//!
//! Written before the code: if a rule changes, change it here first.

use ccs::domain::{
    check_env, parse_command, provider_label, parse_env, plan_launch, render_exports, resolve, CcsError, Command,
    Finding, Profile, ProfileName, PROVIDER_VARS,
};
use std::path::PathBuf;

fn name(s: &str) -> ProfileName {
    ProfileName::parse(s).unwrap()
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter().map(|(k, x)| (k.to_string(), x.to_string())).collect()
}

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// ── Profile names ────────────────────────────────────────────────────────────

#[test]
fn ordinary_names_are_accepted() {
    for ok in ["personal", "bedrock-work", "aws.prod_2", "A1"] {
        assert_eq!(name(ok).as_str(), ok);
    }
}

#[test]
fn names_that_could_escape_the_profile_root_are_refused() {
    for bad in ["", "../x", "a/b", ".hidden", "-flag", "has space"] {
        assert!(ProfileName::parse(bad).is_err(), "{bad:?} must be refused");
    }
}

#[test]
fn command_words_and_shared_cannot_be_profile_names() {
    // `ccs list` must always mean the command, never a profile called list.
    for reserved in ["list", "new", "import", "default", "env", "link", "doctor", "which", "help", "shared"] {
        assert!(ProfileName::parse(reserved).is_err(), "{reserved} must be reserved");
    }
}

// ── profile.env ──────────────────────────────────────────────────────────────

#[test]
fn profile_env_parses_comments_export_and_quotes() {
    let text = "# Bedrock\n\nexport AWS_PROFILE=work\nAWS_REGION=\"us-east-1\"\n  ANTHROPIC_MODEL='m:0'\nEMPTY=\n";
    assert_eq!(
        parse_env(text).unwrap(),
        pairs(&[("AWS_PROFILE", "work"), ("AWS_REGION", "us-east-1"), ("ANTHROPIC_MODEL", "m:0"), ("EMPTY", "")])
    );
}

#[test]
fn profile_env_is_data_not_shell() {
    // A command substitution is kept as literal text. Nothing evaluates it.
    let env = parse_env("X=$(rm -rf ~)\n").unwrap();
    assert_eq!(env, pairs(&[("X", "$(rm -rf ~)")]));
}

#[test]
fn profile_env_reports_the_bad_line_number() {
    let err = parse_env("A=1\n# ok\nnot an assignment\n").unwrap_err();
    assert_eq!(err, CcsError::EnvSyntax { line: 3 });
}

// ── Resolution order ─────────────────────────────────────────────────────────

#[test]
fn resolution_order_is_argument_pin_env_default() {
    let s = |x: &str| Some(x.to_string());
    assert_eq!(resolve(s("arg"), s("pin"), s("env"), s("def")), s("arg"));
    assert_eq!(resolve(None, s("pin"), s("env"), s("def")), s("pin"));
    assert_eq!(resolve(None, None, s("env"), s("def")), s("env"));
    assert_eq!(resolve(None, None, None, s("def")), s("def"));
    assert_eq!(resolve(None, None, None, None), None);
}

#[test]
fn an_empty_value_does_not_win_resolution() {
    // `CCS_PROFILE=` in a shell must fall through, not select a profile named "".
    let r = resolve(None, Some(String::new()), Some("  ".into()), Some("def".into()));
    assert_eq!(r, Some("def".into()));
}

// ── Launch plan ──────────────────────────────────────────────────────────────

#[test]
fn an_isolated_profile_sets_its_config_dir_and_its_env() {
    let p = Profile {
        name: name("work"),
        env: pairs(&[("CLAUDE_CODE_USE_BEDROCK", "1"), ("AWS_PROFILE", "w")]),
        config_dir: PathBuf::from("/p/work/config"),
    };
    let plan = plan_launch(&p, args(&["--resume"]));

    for v in PROVIDER_VARS {
        assert!(plan.unset.contains(&v.to_string()), "{v} must be cleared first");
    }
    assert_eq!(
        plan.set,
        pairs(&[
            ("CLAUDE_CODE_USE_BEDROCK", "1"),
            ("AWS_PROFILE", "w"),
            ("CLAUDE_CONFIG_DIR", "/p/work/config"),
            ("CCS_PROFILE", "work"),
        ])
    );
    assert_eq!(plan.args, args(&["--resume"]));
}

#[test]
fn every_profile_sets_its_config_dir_so_none_runs_on_stock_dotclaude() {
    // ADR-2610261643 §5: nothing falls back to ~/.claude.
    let p = Profile { name: name("bare"), env: vec![], config_dir: PathBuf::from("/p/bare/config") };
    let plan = plan_launch(&p, vec![]);
    assert!(plan.unset.contains(&"CLAUDE_CONFIG_DIR".to_string()));
    assert_eq!(plan.set, pairs(&[("CLAUDE_CONFIG_DIR", "/p/bare/config"), ("CCS_PROFILE", "bare")]));
}

#[test]
fn provider_vars_cover_bedrock_and_the_config_dir() {
    for v in ["CLAUDE_CODE_USE_BEDROCK", "AWS_PROFILE", "AWS_REGION", "ANTHROPIC_MODEL", "CLAUDE_CONFIG_DIR"] {
        assert!(PROVIDER_VARS.contains(&v), "{v}");
    }
    // Secrets are the user's to manage; clearing them would break their shell setup.
    for v in ["ANTHROPIC_API_KEY", "AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN"] {
        assert!(!PROVIDER_VARS.contains(&v), "{v}");
    }
}

#[test]
fn exports_are_shell_quoted() {
    let p = Profile {
        name: name("work"),
        env: pairs(&[("A", "it's $HOME")]),
        config_dir: PathBuf::from("/my dir/config"),
    };
    let out = render_exports(&plan_launch(&p, vec![]));
    assert!(out.contains("unset AWS_PROFILE\n"));
    assert!(out.contains("export A='it'\\''s $HOME'\n"), "{out}");
    assert!(out.contains("export CLAUDE_CONFIG_DIR='/my dir/config'\n"), "{out}");
    assert!(out.ends_with("export CCS_PROFILE='work'\n"), "{out}");
}

// ── Command line ─────────────────────────────────────────────────────────────

#[test]
fn commands_parse() {
    let none = |_: &str| false;
    assert_eq!(parse_command(&args(&["list"]), none).unwrap(), Command::List);
    assert_eq!(parse_command(&args(&["ls"]), none).unwrap(), Command::List);
    assert_eq!(parse_command(&args(&["import"]), none).unwrap(), Command::Import);
    assert_eq!(parse_command(&args(&["migrate"]), none).unwrap(), Command::Migrate);
    assert!(matches!(parse_command(&args(&["link"]), none), Err(CcsError::Gone(_))));
    assert_eq!(parse_command(&args(&["which"]), none).unwrap(), Command::Which);
    assert_eq!(parse_command(&args(&["--help"]), none).unwrap(), Command::Help);
    assert_eq!(
        parse_command(&args(&["new", "w", "--from", "p"]), none).unwrap(),
        Command::New { name: "w".into(), from: Some("p".into()) }
    );
    assert_eq!(parse_command(&args(&["default"]), none).unwrap(), Command::Default(None));
    assert_eq!(parse_command(&args(&["env", "w"]), none).unwrap(), Command::Env(Some("w".into())));
    assert_eq!(parse_command(&args(&["doctor"]), none).unwrap(), Command::Doctor(None));
}

#[test]
fn a_known_profile_name_launches_with_the_rest_passed_through() {
    let known = |s: &str| s == "work";
    assert_eq!(
        parse_command(&args(&["work", "-p", "hi"]), known).unwrap(),
        Command::Launch { profile: Some("work".into()), kits: None, args: args(&["-p", "hi"]) }
    );
}

#[test]
fn anything_else_launches_the_resolved_profile_with_every_arg() {
    let known = |s: &str| s == "work";
    assert_eq!(
        parse_command(&args(&["-p", "hi"]), known).unwrap(),
        Command::Launch { profile: None, kits: None, args: args(&["-p", "hi"]) }
    );
    assert_eq!(parse_command(&[], known).unwrap(), Command::Launch { profile: None, kits: None, args: vec![] });
}

#[test]
fn new_without_a_name_is_a_usage_error() {
    assert!(matches!(parse_command(&args(&["new"]), |_| false), Err(CcsError::Usage(_))));
    assert!(matches!(parse_command(&args(&["new", "w", "--from"]), |_| false), Err(CcsError::Usage(_))));
}

// ── Doctor rules ─────────────────────────────────────────────────────────────

#[test]
fn bedrock_without_a_region_fails_the_doctor() {
    let f = check_env(&pairs(&[("CLAUDE_CODE_USE_BEDROCK", "1")]));
    assert!(f.iter().any(|x| matches!(x, Finding::Fail(m) if m.contains("AWS_REGION"))), "{f:?}");
    assert!(f.iter().any(|x| matches!(x, Finding::Warn(m) if m.contains("AWS_PROFILE"))), "{f:?}");
}

#[test]
fn a_complete_bedrock_env_has_no_failures() {
    let f = check_env(&pairs(&[("CLAUDE_CODE_USE_BEDROCK", "1"), ("AWS_REGION", "us-east-1"), ("AWS_PROFILE", "w")]));
    assert!(!f.iter().any(|x| matches!(x, Finding::Fail(_) | Finding::Warn(_))), "{f:?}");
}

// ── Status line (ADR-2610261237) ─────────────────────────────────────────────

mod statusline {
    use super::pairs;
    use ccs::domain::{compose_status, default_settings, status_segment, Wrapped};

    #[test]
    fn a_bedrock_profile_shows_name_aws_profile_and_region() {
        let v = pairs(&[("CCS_PROFILE", "work"), ("CLAUDE_CODE_USE_BEDROCK", "1"), ("AWS_PROFILE", "sso"), ("AWS_REGION", "us-east-1")]);
        assert_eq!(status_segment(&v).as_deref(), Some("⬡ work · bedrock sso@us-east-1"));
    }

    #[test]
    fn missing_bedrock_parts_are_left_out() {
        let v = pairs(&[("CCS_PROFILE", "w"), ("CLAUDE_CODE_USE_BEDROCK", "true")]);
        assert_eq!(status_segment(&v).as_deref(), Some("⬡ w · bedrock"));
        let v = pairs(&[("CCS_PROFILE", "w"), ("CLAUDE_CODE_USE_BEDROCK", "1"), ("AWS_REGION", "eu-west-1")]);
        assert_eq!(status_segment(&v).as_deref(), Some("⬡ w · bedrock @eu-west-1"));
    }

    #[test]
    fn other_providers_are_named() {
        let seg = |extra: (&str, &str)| status_segment(&pairs(&[("CCS_PROFILE", "p"), extra])).unwrap();
        assert_eq!(seg(("CLAUDE_CODE_USE_VERTEX", "1")), "⬡ p · vertex");
        assert_eq!(seg(("CLAUDE_CODE_USE_FOUNDRY", "1")), "⬡ p · foundry");
        assert_eq!(seg(("ANTHROPIC_BASE_URL", "https://gw")), "⬡ p · gateway");
        assert_eq!(seg(("UNRELATED", "x")), "⬡ p · anthropic");
    }

    #[test]
    fn a_falsy_bedrock_flag_is_not_bedrock() {
        let v = pairs(&[("CCS_PROFILE", "p"), ("CLAUDE_CODE_USE_BEDROCK", "0")]);
        assert_eq!(status_segment(&v).as_deref(), Some("⬡ p · anthropic"));
    }

    #[test]
    fn without_ccs_there_is_no_segment() {
        assert_eq!(status_segment(&pairs(&[("CLAUDE_CODE_USE_BEDROCK", "1")])), None);
        assert_eq!(status_segment(&pairs(&[("CCS_PROFILE", "")])), None);
    }

    #[test]
    fn wrapping_prefixes_only_the_first_line() {
        let out = compose_status(Some("⬡ w".into()), Wrapped::Output("model · dir\nline two\n".into()));
        assert_eq!(out, "⬡ w │ model · dir\nline two");
    }

    #[test]
    fn composing_handles_every_empty_side() {
        assert_eq!(compose_status(Some("⬡ w".into()), Wrapped::Nothing), "⬡ w");
        assert_eq!(compose_status(Some("⬡ w".into()), Wrapped::Output("\n".into())), "⬡ w");
        assert_eq!(compose_status(None, Wrapped::Output("theirs\n".into())), "theirs");
        assert_eq!(compose_status(None, Wrapped::Nothing), "");
    }

    #[test]
    fn a_failed_wrapped_command_is_named_not_hidden() {
        let out = compose_status(Some("⬡ w".into()), Wrapped::Failed("/x/sl.sh".into()));
        assert_eq!(out, "⬡ w │ ⚠ statusline failed: /x/sl.sh");
    }

    #[test]
    fn default_settings_is_json_with_an_escaped_command() {
        let s = default_settings("/odd \"dir\"\\ccs");
        assert_eq!(
            s,
            "{\n  \"statusLine\": {\n    \"type\": \"command\",\n    \"command\": \"'/odd \\\"dir\\\"\\\\ccs' statusline\"\n  }\n}\n"
        );
    }
}

// ── AWS import (ADR-2610261238) ──────────────────────────────────────────────

mod aws {
    use super::{args, pairs};
    use ccs::domain::{aws_profile_env, ccs_name_for_aws, parse_command, parse_env, Command};

    #[test]
    fn an_aws_profile_keeps_its_name() {
        assert_eq!(ccs_name_for_aws("claude-bedrock").unwrap().as_str(), "claude-bedrock");
    }

    #[test]
    fn the_aws_default_profile_becomes_aws_default() {
        assert_eq!(ccs_name_for_aws("default").unwrap().as_str(), "aws-default");
    }

    #[test]
    fn an_aws_name_ccs_cannot_use_is_an_error() {
        assert!(ccs_name_for_aws("team/admin").is_err());
        assert!(ccs_name_for_aws("list").is_err());
    }

    #[test]
    fn the_written_env_selects_bedrock_profile_and_region() {
        let text = aws_profile_env("sso", Some("us-east-1"));
        assert_eq!(
            parse_env(&text).unwrap(),
            pairs(&[("CLAUDE_CODE_USE_BEDROCK", "1"), ("AWS_PROFILE", "sso"), ("AWS_REGION", "us-east-1")])
        );
    }

    #[test]
    fn without_a_region_the_line_is_left_commented_for_doctor_to_catch() {
        let text = aws_profile_env("sso", None);
        assert_eq!(parse_env(&text).unwrap(), pairs(&[("CLAUDE_CODE_USE_BEDROCK", "1"), ("AWS_PROFILE", "sso")]));
        assert!(text.contains("# AWS_REGION="), "{text}");
    }

    #[test]
    fn import_aws_commands_parse() {
        let none = |_: &str| false;
        assert_eq!(
            parse_command(&args(&["import", "aws"]), none).unwrap(),
            Command::ImportAws { profiles: vec![], all: false, region: None }
        );
        assert_eq!(
            parse_command(&args(&["import", "aws", "a", "--region", "eu-west-1", "b"]), none).unwrap(),
            Command::ImportAws { profiles: args(&["a", "b"]), all: false, region: Some("eu-west-1".into()) }
        );
        assert_eq!(
            parse_command(&args(&["import", "aws", "--all"]), none).unwrap(),
            Command::ImportAws { profiles: vec![], all: true, region: None }
        );
        assert!(parse_command(&args(&["import", "aws", "--region"]), none).is_err());
        assert!(parse_command(&args(&["import", "gcp"]), none).is_err());
    }

    #[test]
    fn statusline_commands_parse() {
        let none = |_: &str| false;
        assert_eq!(parse_command(&args(&["statusline"]), none).unwrap(), Command::Statusline { wrap: vec![] });
        assert_eq!(
            parse_command(&args(&["statusline", "--", "sh", "-c", "x"]), none).unwrap(),
            Command::Statusline { wrap: args(&["sh", "-c", "x"]) }
        );
        assert!(parse_command(&args(&["statusline", "sh"]), none).is_err(), "a wrapped command needs --");
    }
}

#[test]
fn the_provider_label_names_the_backend_and_its_account() {
    let v = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, x)| (k.to_string(), x.to_string())).collect::<Vec<_>>();
    assert_eq!(provider_label(&v(&[("CLAUDE_CODE_USE_BEDROCK", "1"), ("AWS_PROFILE", "p"), ("AWS_REGION", "r")])), "bedrock p@r");
    assert_eq!(provider_label(&v(&[("ANTHROPIC_BASE_URL", "http://x")])), "gateway");
    assert_eq!(provider_label(&[]), "anthropic");
}

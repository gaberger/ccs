# ADR-2610261238: Import AWS CLI profiles as Bedrock profiles

**Status:** Accepted
**Date:** 2026-10-26
**Drivers:** The user asked how to import AWS profiles. Making one `ccs` profile per AWS account by hand (`ccs new`, edit `profile.env`) does not scale past two accounts.

## Context

The machine has AWS CLI profiles (`claude-bedrock`, `claude-bedrock-gary`,
`horizon`, `default`). Not all of them are Bedrock accounts.

AWS profiles can live in `~/.aws/config`, in `~/.aws/credentials`, or in files
named by `AWS_CONFIG_FILE` and `AWS_SHARED_CREDENTIALS_FILE`. SSO profiles add
more forms. The AWS CLI already resolves every one of these.

## Decision

Add `ccs import aws [<aws-profile>…] [--all] [--region <r>]`.

1. **Source of truth is the AWS CLI.** Names come from
   `aws configure list-profiles`, regions from
   `aws configure get region --profile <p>`. `ccs` does not parse AWS files and
   never reads a credential.
2. **No names lists, and changes nothing.** It prints each AWS profile and
   whether a `ccs` profile already uses it. Importing needs names or `--all`,
   because not every AWS profile is a Bedrock account.
3. **One `ccs` profile per AWS profile, same name.** The AWS profile `default`
   becomes `aws-default`, because `default` is a `ccs` command word. A name that
   `ccs` cannot use is reported and skipped.
4. **The profile.env written** is `CLAUDE_CODE_USE_BEDROCK=1`,
   `AWS_PROFILE=<p>`, `AWS_REGION=<r>`. The region is the AWS profile's own, else
   `--region`. With neither, the `AWS_REGION` line is written commented out, and
   `ccs doctor` fails the profile until it is set.
5. **Existing profiles are skipped, never overwritten.**
6. The new profile gets the `ccs` status line (ADR-2610261237).

## Consequences

- Three Bedrock accounts are one command:
  `ccs import aws claude-bedrock claude-bedrock-gary`.
- `ccs import aws` needs the AWS CLI on PATH. Without it, the command fails and
  says so. Bedrock users have it already.
- `ANTHROPIC_MODEL` is not set. Claude Code picks its own Bedrock default. A user
  who wants a pinned model edits `profile.env`.

## Implementation

- `domain/`: `ccs_name_for_aws`, `aws_profile_env`, `Command::ImportAws`.
- `ports/`: `AwsProfiles` (list names, get a region).
- `adapters/secondary/aws_cli.rs`: `AwsCli`, runs the `aws` binary.
- `ProfileStore::write_env`.
- Gate: `cargo test && hexa analyze . --grade A`. Tests use a fake `AwsProfiles`.

### Checked against the real machine (2026-10-26)

In a throwaway `$CCS_HOME`, `ccs import aws` listed the four real AWS profiles
and created nothing. `ccs import aws claude-bedrock claude-bedrock-gary` made
two profiles, both with region `us-east-1` read from the AWS config, and
`ccs doctor claude-bedrock` passed. Running `ccs statusline --
~/.claude/statusline-bedrock.sh` under that profile printed
`⬡ claude-bedrock · bedrock claude-bedrock@us-east-1 │ [AWS Bedrock (us-east-1)] Opus 5.5 | switcher`.

Injected bugs, each caught by the named test and reverted:

| Injected bug | Test that failed |
|---|---|
| Wrapped status line gets no stdin | `statusline_wraps_a_real_script_and_passes_stdin_through` |
| `--region` beats the profile's own region | `the_region_flag_fills_only_a_missing_region` |
| Import overwrites an existing profile | `existing_unknown_and_unusable_profiles_are_skipped_not_overwritten` |
| A failed wrapped command is hidden | `statusline_survives_a_missing_wrapped_script` |

## References

- ADR-2610261212 — one config directory per profile
- ADR-2610261237 — status line segment
- `aws configure list-profiles`: https://docs.aws.amazon.com/cli/latest/reference/configure/list-profiles.html

## Evidence

`cargo test 2>&1 | grep "test result" && hexa analyze . --grade A 2>&1 | grep "Architecture grade"` at a2f6ff9 with uncommitted changes on 2026-10-26 16:43 UTC:

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  ⬡ Architecture grade: A+ — score 100/100
```

# ADR-2610281600: `ccs upgrade` without `gh`

Status: accepted (2026-11-07). Supersedes the `gh` clause of ADR-2610271000 §2.

## Context

ADR-2610271000 downloaded releases with `gh release download` because
`gaberger/ccs` was private and `gh` held the login. The repository is public
now (anonymous `curl` of its releases succeeds), so the login, and with it `gh`,
is no longer needed. The user: "we need the ccs upgrade to work without gh".

## Decision

- `ccs upgrade` uses `curl` only. The newest tag is read from the redirect of
  `https://github.com/<repo>/releases/latest` (it lands on `…/releases/tag/vX.Y.Z`;
  no API call, so no anonymous rate limit). Assets come from
  `https://github.com/<repo>/releases/download/<tag>/<asset>`.
- Everything after the download is unchanged: the archive is checked against
  `SHA256SUMS.txt`, the new binary is run once with `version`, and it is renamed
  over the running one.
- The `Releases` port is unchanged; `CurlReleases` (`curl_releases.rs`)
  replaces `GhReleases`. `$CCS_CURL_BIN` replaces `$CCS_GH_BIN`; `$CCS_REPO` stays.
- The README install line uses `curl` too.

## Consequences

- No token is sent. If the repository becomes private again, or a fork is
  private, `ccs upgrade` fails with curl's 404; that case needs a new decision.
- Machines running 0.8.0 or older still carry the `gh` code, so their first
  upgrade to this release still needs `gh` (or the README `curl` install line).
  From this release on, none do.
- Needs `curl` on the machine (present on macOS and mainstream Linux).

## Gate

`tests/upgrade.rs` runs the real binary against a fake `curl`, including
`upgrade_never_runs_gh` (a `gh` on `PATH` that records being called) and
`upgrade_needs_curl_and_nothing_else`. Real run: with `gh` off `PATH`,
`ccs upgrade --check` and `ccs upgrade 0.7.1` against the live releases.

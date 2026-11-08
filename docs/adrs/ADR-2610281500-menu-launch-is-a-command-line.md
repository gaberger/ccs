# ADR-2610281500: A launch from the menu is a command line

## Decision

When the menu starts a launch (`Launch`, `LaunchWith`), ccs forms the line that
would have started the same thing (`ccs work +net --allow-all`), prints it as
`$ <line>` on stderr, and appends it to the shell's history file through the
`History` port. `domain::command_line` is the pure formatter; `FsHistory` is the
adapter (zsh extended format, bash plain, any other shell: nothing).

## 2. The shell runs it (built in)

The `Typer` port (`TtyTyper`, `TIOCSTI`) puts the line into the terminal's input
queue and ccs exits. The shell that started ccs reads the line, runs it, and files
it in history itself, immediately, with no wrapper or rc-file change. The line
re-enters ccs as `ccs <profile> …`, so the menu and the typed command are the
same path. If the terminal refuses (no tty, Linux 6.2+ `dev.tty.legacy_tiocsti=0`)
or the line holds a control character, ccs falls back to §1: print the line,
append it to the history file, launch directly. Gate: `tests/typed_launch.py`
drives the real menu in a pty.

## Limits of the fallback (§1)

- A child process cannot write into its parent shell's memory. A running shell
  sees the entry when it rereads the file (zsh with `SHARE_HISTORY` or
  `INC_APPEND_HISTORY`, bash after `history -n`), otherwise at next start.
- `$HISTFILE` is used when exported; shells rarely export it, so the default
  `~/.zsh_history` / `~/.bash_history` applies.
- Newlines in an argument become spaces in the history entry only.
- The `Shell` exit (a subshell in the profile folder) is not a `ccs` command and is not recorded.

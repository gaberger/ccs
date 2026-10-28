# TUI design 1: the dual-pane builder

Status: proposal. Replaces the Home / profile page / kit page / library picker
flow of ADR-2610261643 §18–§20. Keeps the `Browser` port, the `State`/`draw`
split, `Exit`, the terminal-restore rules and ADR-2610261255 Decisions 3, 5, 6.

## 1. Summary

1. One working screen: the whole library on the left, the kit being edited on the right, a load bar along the bottom.
2. The kit pane lists every item the kit will load, grouped under the `uses` line that brings it, so folder picks are shown expanded, not as a bare path (job 1).
3. The library pane shows every kind in one tree with a mark on each row against the open kit: picked, comes with a folder or plugin, partly picked, or not picked. Unpicked means available (jobs 2 and 3).
4. Space toggles the row under the cursor in either pane, and the footer always says in words what space will do on that row, so picking needs no guessing (job 4). `n` makes a kit in place and leaves the cursor in the library, ready to pick (job 5).
5. The bottom bar always reads "<profile> will load N skills · M agents · P plugins", and `l` opens the full list of what that profile loads, item by item, with the kit and line that brings each one.

## 2. Screen inventory

There are two full screens and six overlays. Everything else from today's
pages (profile page, kit page, library page) folds into these.

| # | Name | What it is | Reached by | Left by |
|---|---|---|---|---|
| S1 | **Builder** | Title bar (profile, the kits it starts with), kit bar, Library pane, Kit pane, load bar, hint line. At 160+ columns a third pane, the Inspector, shows the row under the cursor. | `ccs`, `ccs ui` | `q`, `r` (run Claude), Ctrl-C |
| S2 | **Load view** | Everything one profile loads, item by item: kind, name as Claude sees it, kit, `uses` line; then problems, then the exact `claude --plugin-dir …` command. | `l`, or a click on the load bar | Esc, right click; Enter on a row returns to S1 with that kit open and the cursor on that item |
| O1 | **Profile picker** | Profiles, with the default marked. Also holds the profile actions that the old profile page had: `e` edit `profile.env`, `d` make default, `c` copy to a new profile, `t` shell in its folder, `N` new profile, `i` import (AWS, `~/.claude`). | `p`, click on the profile name | Enter (choose), Esc |
| O2 | **Kit picker** | All kits with item counts and which profiles use each. Kit actions: `n` new, `N` copy this kit, `R` rename, `D` delete (to trash, asks `y`). Needed at 80 columns, where the kit bar does not fit. | `k`, click on `kit … ▾` | Enter, Esc |
| O3 | **New kit prompt** | One line that replaces the kit bar: name field, a live check of the name, Enter to make it. | `n`, click on `+ new kit` | Enter (makes and opens the kit), Esc |
| O4 | **Reader** | Read-only text of an item's main file (SKILL.md, agent .md, plugin README), drawn over the pane that does not hold the cursor so the row stays visible. | Enter or `v` on an item | Esc, `q`, right click |
| O5 | **Confirm line** | One line above the hint line for anything that rewrites more than the one line the user pointed at: splitting a folder pick, deleting a kit, trashing a library item. | the action | `y` / Esc |
| O6 | **Find line** | `/` turns the hint line into a filter field. Both panes filter as you type. | `/` | Enter (keep filter, go to the list), Esc (clear) |

Library housekeeping that the old library page did (add, move, new folder,
rename folder, trash, update plugins) stays as keys in the Library pane; it does
not get its own screen.

```
                 p ──► O1 profile picker ──Enter──┐
                 k ──► O2 kit picker ─────Enter───┤
                 n ──► O3 new kit prompt ─Enter───┤
   ccs / ccs ui ─────► S1 BUILDER ◄───────────────┘
                       │  ▲   │  ▲    │
            Enter / v  │  │Esc│  │Esc │ r ──► Exit::Launch (terminal restored, claude starts)
                       ▼  │   ▼  │    │ q ──► Exit::Quit
                  O4 reader   S2 load view (l)
                              Enter on a row ──► S1, that kit open, cursor on that item
```

## 3. Mockups

Marks used in every mockup:

| Mark | Meaning |
|---|---|
| `[x]` | picked: the kit has a `uses` line for exactly this row |
| `[^]` | comes with a picked folder or plugin above it (no line of its own) |
| `[-]` | a folder with some, not all, of its items picked |
| `[ ]` | not picked: available |
| `▶` | the cursor |
| `▷` | in the Kit pane: the same item the Library cursor is on |
| `┏━┓` | the pane that has the keyboard (heavy border); `┌─┐` the other |

In colour: `[x]` green, `[^]` cyan, `[-]` yellow, missing lines red. The marks
are also distinct as text, so the design works with `NO_COLOR` and in tests.

### 3a. Builder, 120x36

Profile claude-bedrock-gary, editing kit `test`
(`skills/forward-dev/`, `skills/horizon/`, `skills/network/inspect/`,
`skills/network/plan/`). The cursor is in the Library, on an item that comes
with the folder pick `skills/network/inspect/`.

```
 ccs │ profile claude-bedrock-gary ▾ │ starts with  base · writing · slides · test                         r run claude 
 kits │ ●base  ●writing  ●slides  [●test]  claude-flow  dev  forward-dev  network  │ + new kit        ● on this profile 
┏━ Library ━ [all] skills agents plugins ━━━━━━━━━━ 41·96·2 ━┓┌─ Kit test ─ 4 lines · 31 skills · 0 agents ────────────┐
┃ ▾ SKILLS · 31 of 41 in this kit                            ┃│ Used by   ● claude-bedrock-gary                        │
┃ [x] ▸ forward-dev/                                     2/2 ┃│ CLAUDE.md none · c writes one                          │
┃ [x] ▸ horizon/                                         2/2 ┃│ ─ what it loads, line by line ──────────────────────── │
┃ [ ] ▸ methods/                                       empty ┃│ ▾ skills/forward-dev/                    folder      2 │
┃ [-] ▾ network/                                       25/35 ┃│     analyze-apidiff-report                             │
┃ [ ]   ▸ change/                                        0/2 ┃│     fn-parity                                          │
┃ [x]   ▾ inspect/                                     10/10 ┃│ ▾ skills/horizon/                        folder      2 │
┃ [^]       compare-device-config                            ┃│     design-system-review                               │
┃▶[^]       inspect-bgp-neighbors                            ┃│     horizon-worktree-dev                               │
┃ [^]       inspect-device-files                             ┃│ ▾ skills/network/inspect/                folder     10 │
┃ [^]       inspect-edge                                     ┃│     compare-device-config                              │
┃ [^]       inspect-history                                  ┃│   ▷ inspect-bgp-neighbors                              │
┃ [^]       inspect-inventory                                ┃│     inspect-device-files                               │
┃ [^]       inspect-networks                                 ┃│     inspect-edge                                       │
┃ [^]       inspect-snapshots                                ┃│     inspect-history                                    │
┃ [^]       inspect-topology                                 ┃│     inspect-inventory                                  │
┃ [^]       inspect-vulnerabilities                          ┃│     inspect-networks                                   │
┃ [ ]   ▸ investigate/                                   0/3 ┃│     inspect-snapshots                                  │
┃ [ ]   ▸ nqe/                                           0/4 ┃│     inspect-topology                                   │
┃ [x]   ▸ plan/                                        15/15 ┃│     inspect-vulnerabilities                            │
┃ [ ]   ▸ review/                                        0/1 ┃│ ▸ skills/network/plan/                   folder     15 │
┃ [ ] ▸ writing/                                         0/2 ┃│     15 skills · → shows them                           │
┃ ▾ AGENTS · 0 of 96 in this kit                             ┃│                                                        │
┃ [ ] ▸ claude-flow/                                    0/76 ┃│ ─ totals ───────────────────────────────────────────── │
┃ [ ] ▸ dev/                                            0/18 ┃│ 4 lines → 31 skills · 0 agents · 0 plugins             │
┃ [ ] ▸ network/                                         0/2 ┃│ 4 folder lines take new items as they arrive           │
┃ ▾ PLUGINS · 0 of 2 in this kit                             ┃│ missing lines: none                                    │
┃ [ ] ▸ egm                              7 skills · 9 agents ┃│                                                        │
┃ [ ] ▸ forward-decks                    2 skills            ┃│                                                        │
┗━ [x] picked  [^] via folder  [-] partly  [ ] not ━━━━━━━━━━┛└─ space drops a line · ⏎ opens it ──────────────────────┘
 ▣ claude-bedrock-gary will load 31 skills · 0 agents · 1 plugin (forward-decks, 2 skills inside) · from 4 kits · l all 
 inspect-bgp-neighbors comes from skills/network/inspect/ · space: split that folder so you can drop this one · ⏎ read  
 tab kit pane · / find · u available only · n new kit · k kits · p profile · a attach · l loads · r run · q quit        
```

Notes on 3a:

- The kit bar shows every kit. `●` marks the kits this profile starts with;
  `[ ]` brackets the kit being edited. A click on a name opens it; `[` and `]`
  step through them.
- Folders in the Library show `picked/total`. Folders with a pick in them open
  by default; others stay folded. That is why `inspect/` is open and
  `investigate/` is not.
- The Kit pane folds a line with more than 12 items (`plan/`, 15) and says so
  in the row under it. `→` on that line unfolds it.
- Line 35 is the row hint: it describes the row under the cursor in words and
  says what space would do there. After any change it shows the CLI command
  that made it for three seconds (for example `done: ccs kit use test
  skills/network/plan/`), then goes back to the hint.
- Line 36 is the key line for the pane. It is the only help the screen needs;
  `?` grows it to three lines in place.

### 3b. The same, 80x24

At 80 columns the kit bar collapses into the title (`kit test ▾`, `k` or a
click opens the kit picker). The panes split 40/40. Item names longer than the
pane are cut with `…`; the detail line (line 23) always prints the full path
and the first words of the description of the row under the cursor.

```
 ccs │ claude-bedrock-gary ▾ │ kit test ▾  ● on this profile              r run 
┏━ Library ━ all ━━━━━━━━━━━━━━━━━━━━━━┓┌─ Kit test · 31 skills ───────────────┐
┃ ▾ SKILLS                       31/41 ┃│ ● claude-bedrock-gary · no CLAUDE.md │
┃ [x] ▸ forward-dev/               2/2 ┃│ ▸ skills/forward-dev/              2 │
┃ [x] ▸ horizon/                   2/2 ┃│ ▸ skills/horizon/                  2 │
┃ [ ] ▸ methods/                     0 ┃│ ▾ skills/network/inspect/         10 │
┃ [-] ▾ network/                 25/35 ┃│     compare-device-config            │
┃ [ ]   ▸ change/                  0/2 ┃│   ▷ inspect-bgp-neighbors            │
┃ [x]   ▾ inspect/               10/10 ┃│     inspect-device-files             │
┃ [^]     compare-device-config        ┃│     inspect-edge                     │
┃▶[^]     inspect-bgp-neighbors        ┃│     inspect-history                  │
┃ [^]     inspect-device-files         ┃│     inspect-inventory                │
┃ [^]     inspect-edge                 ┃│     inspect-networks                 │
┃ [^]     inspect-history              ┃│     inspect-snapshots                │
┃ [^]     inspect-inventory            ┃│     inspect-topology                 │
┃ [^]     inspect-networks             ┃│     inspect-vulnerabilities          │
┃ [^]     inspect-snapshots            ┃│ ▸ skills/network/plan/            15 │
┃ [^]     inspect-topology             ┃│                                      │
┃ [^]     inspect-vulnerabilities      ┃│ 4 lines → 31 skills                  │
┃ [ ]   ▸ investigate/             0/3 ┃│ folders take new items               │
┗━ ↓ 11 more ━━━━━━━━━━━━━━━━━━━━━━━━━━┛└──────────────────────────────────────┘
 ▣ loads 31 skills · 0 agents · 1 plugin (+2 skills) · 4 kits             l all 
 skills/network/inspect/inspect-bgp-neighbors · Lists BGP neighbors per device… 
 space split folder · ⏎ read · tab kit · / find · n new · a attach · ? more     
```

At 80 the Kit pane opens only the line that holds the item under the Library
cursor and folds the rest, so the two cursors stay level. Below 70 columns
(today's `NARROW`) the panes stack: one pane shows at a time under a
`Library | Kit test` tab row, Tab swaps them, and the load bar stays.

### 3c. Making a new kit

Profile sticksandstrings (no kits yet). `n` turns the kit bar into a prompt.
The name is checked as it is typed, against kits and plugins.

```
 ccs │ profile sticksandstrings ▾ │ starts with  no kits                                                   r run claude 
 new kit │ name: research█   ✓ no kit or plugin is called research   ⏎ make it · esc cancel                             
```

If the name is taken the check reads `✗ kit research exists`, and Enter does
nothing. If a plugin has the name, it reads `! plugins/research exists; both
would load as "research"` and Enter still works (see §8).

After Enter: `ccs kit new research` runs through the port, the kit opens, the
Library pane gets the keyboard with the cursor on the first folder, and the
empty Kit pane tells the user the next three steps. Rows 17 to 33 are empty
and cut here.

```
 ccs │ profile sticksandstrings ▾ │ starts with  no kits                                                   r run claude 
 kits │ base  writing  slides  test  [research]  claude-flow  dev  forward-dev  network  │ + new kit  ● on this profile 
┏━ Library ━ [all] skills agents plugins ━━━━━━━━━━ 41·96·2 ━┓┌─ Kit research ─ empty ─────────────────────────────────┐
┃ ▾ SKILLS · 0 of 41 in this kit                             ┃│ Kit research is empty.                                 │
┃▶[ ] ▸ forward-dev/                                     0/2 ┃│                                                        │
┃ [ ] ▸ horizon/                                         0/2 ┃│ Pick from the library on the left:                     │
┃ [ ] ▸ methods/                                       empty ┃│   space  pick the row under the cursor                 │
┃ [ ] ▸ network/                                        0/35 ┃│   /      find by name, e.g. /humanize                  │
┃ [ ] ▸ writing/                                         0/2 ┃│   3      show plugins only (0 shows all)               │
┃ ▾ AGENTS · 0 of 96 in this kit                             ┃│                                                        │
┃ [ ] ▸ claude-flow/                                    0/76 ┃│ Then:                                                  │
┃ [ ] ▸ dev/                                            0/18 ┃│   a      attach research to sticksandstrings           │
┃ [ ] ▸ network/                                         0/2 ┃│   l      see what sticksandstrings loads               │
┃ ▾ PLUGINS · 0 of 2 in this kit                             ┃│                                                        │
┃ [ ] ▸ egm                              7 skills · 9 agents ┃│ made with: ccs kit new research                        │
┃ [ ] ▸ forward-decks                    2 skills            ┃│                                                        │
```

`N` (in the kit picker, or on the kit bar) makes a kit that starts as a copy of
the open one, for "like test, but with two more skills".

### 3d. Opening a plugin to see inside, 160x50

At 160 columns the Inspector pane appears on the right. Here the kind filter
is on plugins (`3`), egm is picked into `research`, and `→` has unfolded it.
The plugin's skills and agents are rows in the tree, marked `[^]` because they
come with the plugin. They cannot be picked one at a time: a plugin is loaded
whole by `--plugin-dir`. Rows 25 to 46 are empty and cut here.

```
 ccs │ profile sticksandstrings ▾ │ starts with  no kits                                                                                           r run claude 
 kits │ base  writing  slides  test  [research]  claude-flow  dev  forward-dev  network  │ + new kit                                          ● on this profile 
┏━ Library ━ skills agents [plugins] ━━━━━━━━━━━━━━━━━━┓┌─ Kit research ─ 1 line · 7 skills · 9 agents ────┐┌─ plugins/egm ────────────────────────────────────┐
┃ ▾ PLUGINS · 1 of 2 in this kit                       ┃│ Used by   (none) · a attaches to sticksandstrings││ plugin egm                                       │
┃▶[x] ▾ egm                        7 skills · 9 agents ┃│ CLAUDE.md none · c writes one                    ││ Draft specs, review designs, and check           │
┃       skills                                         ┃│ ─ what it loads ──────────────────────────────── ││ implementation drift.                            │
┃ [^]     egm_bootstrap                                ┃│ ▾ plugins/egm          plugin · 7 skills 9 agents││                                                  │
┃ [^]     egm_brainstorm                               ┃│     skill  egm:egm_bootstrap                     ││ holds     7 skills · 9 agents                    │
┃ [^]     egm_draft                                    ┃│     skill  egm:egm_brainstorm                    ││ source    git clone (u pulls updates)            │
┃ [^]     egm_drift_report                             ┃│     skill  egm:egm_draft                         ││ in kits   dev · research                         │
┃ [^]     egm_resolve                                  ┃│     skill  egm:egm_drift_report                  ││ loaded by (no profile yet)                       │
┃ [^]     egm_review                                   ┃│     skill  egm:egm_resolve                       ││                                                  │
┃ [^]     egm_review_loop                              ┃│     skill  egm:egm_review                        ││ A plugin is picked whole. Its skills and         │
┃       agents                                         ┃│     skill  egm:egm_review_loop                   ││ agents come with it and cannot be picked         │
┃ [^]     bootstrap_orchestrator                       ┃│     agent  egm:bootstrap_orchestrator            ││ one at a time; [^] marks them.                   │
┃ [^]     codebase_details                             ┃│     agent  egm:codebase_details                  ││                                                  │
┃ [^]     comprehension                                ┃│     agent  egm:comprehension                     ││ ─ cursor: plugins/egm ────────────────────────── │
┃ [^]     design_critic                                ┃│     agent  egm:design_critic                     ││ space  drop it from research                     │
┃ [^]     implementation_readiness                     ┃│     agent  egm:implementation_readiness          ││ v      read its README                           │
┃ [^]     structural                                   ┃│     agent  egm:structural                        ││ u      git pull it                               │
┃ [^]     style                                        ┃│     agent  egm:style                             ││ ←      fold it                                   │
┃ [^]     synthesizer                                  ┃│     agent  egm:synthesizer                       ││                                                  │
┃ [^]     zoom_out                                     ┃│     agent  egm:zoom_out                          ││                                                  │
┃ [ ] ▸ forward-decks                         2 skills ┃│                                                  ││                                                  │
```

(The brief counts 8 agents in egm; the clone on this machine has 9 agent
files, listed above. The design shows what the library holds.)

When egm is not picked, its inside rows show `·` instead of a box: they can be
read with Enter but not toggled, and the row hint says "inside plugin egm:
pick the plugin to get it".

The Inspector at 160 columns always shows, for the row under the cursor: the
description, what it holds, where it came from, which kits use it, which
profiles load it, and the keys that apply to it. At 120 and 80 the same facts
appear one line at a time in the row hint.

### 3e. What a profile loads (Load view), 120x36

After the success test in §5: sticksandstrings with kit research
(`plugins/egm`, `skills/writing/forward-generate-report`,
`skills/writing/humanize-writing`). Rows 26 to 33 are empty and cut here.

```
 ccs │ what sticksandstrings loads                                       tab by kind / by kit · esc back · r run claude 
┏━ sticksandstrings + research ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃ 9 skills · 9 agents · 1 plugin, from 1 kit · CLAUDE.md: none · problems: none                                        ┃
┃                                                                                                                      ┃
┃  KIND    NAME AS CLAUDE SEES IT            FROM KIT    THROUGH THE LINE                                              ┃
┃  skill   research:forward-generate-report  research    skills/writing/forward-generate-report                        ┃
┃  skill   research:humanize-writing         research    skills/writing/humanize-writing                               ┃
┃  skill   egm:egm_bootstrap                 research    plugins/egm                                                   ┃
┃  skill   egm:egm_brainstorm                research    plugins/egm                                                   ┃
┃  skill   egm:egm_draft                     research    plugins/egm                                                   ┃
┃  skill   egm:egm_drift_report              research    plugins/egm                                                   ┃
┃  skill   egm:egm_resolve                   research    plugins/egm                                                   ┃
┃  skill   egm:egm_review                    research    plugins/egm                                                   ┃
┃  skill   egm:egm_review_loop               research    plugins/egm                                                   ┃
┃  agent   egm:bootstrap_orchestrator        research    plugins/egm                                                   ┃
┃  agent   egm:codebase_details              research    plugins/egm                                                   ┃
┃  agent   egm:comprehension                 research    plugins/egm                                                   ┃
┃  agent   egm:design_critic                 research    plugins/egm                                                   ┃
┃  agent   egm:implementation_readiness      research    plugins/egm                                                   ┃
┃  agent   egm:structural                    research    plugins/egm                                                   ┃
┃  agent   egm:style                         research    plugins/egm                                                   ┃
┃  agent   egm:synthesizer                   research    plugins/egm                                                   ┃
┃  agent   egm:zoom_out                      research    plugins/egm                                                   ┃
┃                                                                                                                      ┃
┃  plugin  egm                               research    plugins/egm                                                   ┃
┃                                                                                                                      ┃
┃ ─ how ccs starts it ──────────────────────────────────────────────────────────────────────────────────────────────── ┃
┃ claude --plugin-dir <built kit research> --plugin-dir ~/.claude-profiles/library/plugins/egm                         ┃
┃ same as  ccs loads sticksandstrings  (prints this list)                                                              ┃
```

The "by kit" tab (Tab) groups the same rows under each kit, then each line.
For a profile with overlapping kits it also shows what overlaps. For
claude-bedrock-gary with `network` added for one launch, the by-kind list
gives each overlapping item once, with both sources:

```
  skill   network:inspect-edge   network, test   skills/network/ ; skills/network/inspect/   ! loaded twice, once per kit
```

The `problems` count at the top covers: lines that resolve to nothing, the
same item loaded by two kits, two items with the same name inside one kit, and
a kit with the same name as a plugin. Each problem is a row with its own hint
and a key that fixes it where there is one.

## 4. Key map and mouse map

Keys are single letters with a mnemonic. Lower case is for things done every
session; upper case or an overlay holds rare and destructive ones. Space and
Enter mean the same in both panes. No vim letters (`k` is kits), so letters
are never ambiguous.

| Key | Mouse | Where | Does | Same as (CLI) |
|---|---|---|---|---|
| ↑ ↓ PgUp PgDn Home End | wheel over a pane | any list | move the cursor | |
| → | click `▸` | folder, plugin, kit line | unfold | |
| ← | click `▾` | any row | fold, or go to the parent row | |
| Tab | click in a pane | Builder | the other pane gets the keyboard | |
| space | click the box `[ ]` | Library row | pick or unpick it in the open kit | `ccs kit use/drop <kit> <entry>` |
| space | click the box | Kit pane, a line | drop the line | `ccs kit drop <kit> <entry>` |
| space | click the box | an item that comes with a folder | offer to split the folder (O5) | `ccs kit drop <kit> <folder>` + `ccs kit use <kit> <items>…` |
| f | | a folder whose items are all picked one by one | replace them with one folder line | `ccs kit use <kit> <folder>` + `ccs kit drop …` |
| Enter / v | click the selected row | item | read it (O4) | `ccs library show <entry>` |
| e | | item | edit its file in `$VISUAL`/`$EDITOR` | `ccs library edit <entry>` |
| / | click `/` in a pane title | Builder | find: filter both panes by name and path; `//` also matches descriptions | `ccs library list` + grep |
| 0 1 2 3 | click `all skills agents plugins` | Library | show all kinds / skills / agents / plugins | `ccs library list skills` … |
| u | | Library | available only: hide rows already in the kit | `ccs library list --not-in <kit>` (new) |
| h / . | | Library, folder or plugin | hide this source / show hidden ones | view setting, no CLI effect |
| n | click `+ new kit` | Builder | new kit (O3) | `ccs kit new <kit>` |
| k, [ ] | click a kit name | Builder | kit picker / previous, next kit | `ccs kit list` |
| N R D | | kit picker | copy, rename, delete a kit | `ccs kit new --from` (new), `rename`, `rm` |
| c | click `CLAUDE.md` | Builder | edit the kit's CLAUDE.md | `ccs kit edit <kit>` |
| a | click `●`/`○` on the kit bar, or `Used by` | Builder | attach or detach the open kit to the profile | `ccs kit attach/detach <kit> <profile>` |
| p | click the profile name | Builder | profile picker (O1) | `ccs list`, `ccs default`, `ccs new` |
| l | click the load bar | Builder | Load view (S2) | `ccs loads <profile>` (new) |
| r | click `r run claude` | Builder, Load view | start Claude with the profile and its kits | `ccs <profile>` |
| A M m x U | | Library | add, new folder, move, trash, update plugins | `ccs library add/mkdir/mv/rm/update` |
| ? | click `?` | Builder | grow the key line to three lines, in place | |
| Esc | right click | anywhere | close the overlay, clear the find, or go back from S2; never quits | |
| q, Ctrl-C | | anywhere but a text field | quit | |

A double click is never needed: a first click selects, a second click on the
selected row does what Enter does, and a click on a box toggles at once. A
drag from the Library to the Kit pane picks the row; this is a convenience,
never the only way.

**Help without a help screen.** Four things carry it:

1. The row hint (line 35 at 120 columns) describes the row under the cursor in
   words and names what space and Enter do there, using real names: "space:
   attach research to sticksandstrings", not "space: toggle".
2. The key line lists the keys for the pane that has the keyboard, most used
   first. `?` grows it in place to three lines; the panes shrink by two rows.
3. Empty states say what to do next (3c).
4. After every change the row hint prints the CLI command that made it, so the
   TUI teaches the CLI.

## 5. Walkthrough of the success test

Task: make kit `research`, add plugin egm and two skills from `writing/`,
attach it to sticksandstrings, then view everything sticksandstrings loads.
Start: `ccs` in a shell. The Builder opens on the default profile,
claude-bedrock-gary, with its first kit (`base`) open.

| Step | Keys | Count | What the screen shows |
|---|---|---|---|
| 1. Choose the profile | `p` `↓` `Enter` | 3 | The profile picker lists claude-bedrock-gary (default) and sticksandstrings. After Enter the title reads `profile sticksandstrings ▾ │ starts with no kits`; no kit on the bar has a `●`. |
| 2. Make the kit | `n` `r e s e a r c h` `Enter` | 10 | The prompt checks the name as it is typed (`✓ no kit or plugin is called research`). Enter makes it; screen 3c. Row hint: `done: ccs kit new research`. |
| 3. Pick egm | `3` `space` `0` | 3 | `3` shows plugins only; the cursor lands on egm, the first row. Space marks it `[x]`; the Kit pane shows `plugins/egm` with its 7 skills and 9 agents; the load bar does not change yet (the kit is not attached) and says so. `0` shows all kinds again. |
| 4. Find writing | `/` `w r i t i n g` `Enter` | 9 | Both panes filter as letters arrive. The Library shows `▾ SKILLS`, `[ ] ▾ writing/ 0/2`, and its two skills. Enter leaves the find line and puts the cursor on `writing/`. |
| 5. Pick two skills | `↓` `space` `↓` `space` | 4 | forward-generate-report then humanize-writing turn `[x]`; `writing/` becomes `[-] 1/2`, then `2/2`. The row hint adds: "all of writing/ is picked one by one · f makes it one folder line". The user leaves them as two lines, as asked. |
| 6. Clear the find | `Esc` | 1 | Full tree back, cursor still on humanize-writing. |
| 7. Attach | `a` | 1 | The hint line said "a attach research to sticksandstrings". The kit bar shows `[●research]`, the title `starts with research`, the load bar `sticksandstrings will load 9 skills · 9 agents · 1 plugin · from 1 kit`. Row hint: `done: ccs kit attach research sticksandstrings`. |
| 8. View the load | `l` | 1 | Screen 3e. |

Total: **32 keystrokes**, 15 of them the letters of `research` and `writing`.
At an ordinary pace that is 20 to 30 seconds. No step needs the help screen:
step 1 uses `p profile` from the key line; steps 2, 7 and 8 use keys the key
line and the empty kit pane name (`n new kit`, `a attach`, `l loads`); steps 3
and 4 use `3` and `/`, which the empty Kit pane lists in 3c.

The same by mouse only: click the profile name, click sticksandstrings, click
`+ new kit`, type `research`, Enter, click `plugins` in the Library title,
click egm's box, click `all`, click `▸` on writing/, click two boxes, click the
`a attach research to sticksandstrings` text in the hint line, click the load
bar. 13 clicks and 9 keys.

## 6. How each job is solved

**(1) What a kit holds.** The Kit pane never shows a bare `uses` line. Each
line is a heading with its kind (`folder`, `item`, `plugin`) and count, and
the items it gives sit under it, plugins included. The totals line gives
skills, agents and plugins, and says how many lines are folders that take new
items later. A line that gives nothing is red and says why.

**(2) What the library holds and what is inside.** The Library pane is one
tree over all three kinds with a count on every folder and every plugin, and
plugins unfold like folders to show their skills and agents. The pane title
gives the totals (41 · 96 · 2). Enter reads any item, and at 160 columns the
Inspector shows its description, kits and profiles without a key.

**(3) What is available but not picked.** Every Library row has a mark against
the open kit, so `[ ]` is "available" and `0/76` on a folder says how much is
left. `u` hides everything already in the kit. Picks made through a folder show
as `[^]`, so the user never re-picks something they already have.

**(4) Pick and unpick with no guessing.** Space does one thing, and the row
hint says what it will be before the key is pressed. The results show in
three places at once: the mark, the Kit pane (the `▷` row), and the load bar.
The one case that is not a plain toggle, dropping an item that comes with a
folder, asks first (O5) and says what will change.

**(5) Make a new kit.** `n` from anywhere in the Builder, a name checked as it
is typed, Enter. The kit opens with the Library focused, and the empty Kit
pane lists the next steps, including attaching it to the current profile by
name.

## 7. Scale and noise

The library on this machine is 41 skills, 96 agents (76 of them under
`claude-flow/`), and 2 plugins. The tree has to stay readable when it is
mostly noise.

- **Folded by default.** Folders open only when they hold a pick of the open
  kit, or when a find matches inside them. With kit `test` open the agents
  section is three rows: `claude-flow/ 0/76`, `dev/ 0/18`, `network/ 0/2`.
- **Sub-folders carry the grouping.** `claude-flow/` unfolds to its own
  folders (consensus, custom, flow-nexus, github, goal, optimization, …), each
  with a count, before any agent row. The 76 agents are never one flat list.
- **Kind filter.** `1 2 3 0` show skills, agents, plugins, or all. The filter
  is in the pane title, so it is visible and clickable.
- **Find.** `/` filters both panes by name and folder path, case-insensitive,
  matching words in any order (`/coord gossip`). `//` also searches
  descriptions. Parent folders of each match stay as context rows with their
  counts. The pane title shows `12 of 139 match`.
- **Hide a source.** `h` on a folder or plugin hides it from the Library pane
  (`.` shows hidden sources again). One row stays at the bottom of its kind:
  `3 hidden · . shows them`. Hiding changes nothing a kit loads; an item that
  a kit picks still shows in the Kit pane with `(hidden in library)`.
  The setting is stored under `$CCS_HOME`, never `~/.claude`.
- **Near-duplicates.** Agents whose descriptions are identical, or the same
  after the first sentence, get a `≈` after the name, and the row hint names
  the others: `goal/agent ≈ goal/goal-planner (same description)`. At 120
  columns and up, the Library pane shows the first words of each description
  on the row, which tells hierarchical-coordinator from mesh-coordinator from
  gossip-coordinator without opening them.
- **Other kits.** An item another kit already uses shows that kit's name in
  dim text after the row (at 120 and up). This helps the user reuse a kit
  instead of copying its picks.

## 8. Edge cases

**An item picked both directly and through a folder.** The Library shows `[x]`
(the direct line wins the mark). In the Kit pane the item appears under both
lines, the second time dimmed with `also via skills/writing/`. Counts and the
Load view count it once. The direct line gets a hint: "redundant:
skills/writing/ already gives this · space drops this line". Unpicking it
drops only the direct line; the mark turns to `[^]` and the row hint says "still
in the kit through skills/writing/", so space never looks like it did nothing.

**A `uses` line that no longer resolves.** It stays in the kit; ccs does not
remove it. The Kit pane lists it first, in red: `✗ skills/writing/old-skill
not in the library`. The kit bar shows `research!`, the load bar says `1 line
missing`, and the Load view lists it under problems. Space drops it; Enter
runs a find for its last word, to locate a renamed or moved item. A folder line
whose folder exists but is empty (today, `skills/methods/`) is not an error: it
reads `folder · empty now, takes new items`.

**A plugin and a kit with the same name.** A kit is built into a plugin named
after the kit (`fs_kits.rs` writes the kit name into `plugin.json`), so kit
`egm` and plugin `egm` loaded together give Claude two plugins called `egm`.
The design shows names with their kind everywhere it could be unclear (`kit
egm`, `plugins/egm`), warns at naming time (3c), and lists the clash as a
problem in the Load view when both load at once. The same check covers two
skills with one name in different folders picked into one kit, since the kit
build links each skill by its name.

**Empty library on a new host.** The Library pane shows an empty state in place
of the tree: "The library is empty. `i` copies skills and agents from
~/.claude (ccs import) · `A` adds a folder, a file, or a git repo (ccs
library add)". With no kits the kit bar holds only `+ new kit`, and the Kit
pane says "No kits yet · n makes one". With no profiles the title reads
`no profile · p makes one` and `r` is disabled with that reason.

**Very long names.** Names are cut at the end with `…` to fit the pane; the
row hint (and the Inspector at 160) always shows the full path. Tree indent is
two columns per level and stops growing after the third level; deeper rows
show the extra path as a dim prefix (`…/backend/dev-backend-api`). Counts and
marks have fixed columns, so a long name never moves a box and clicks always
land.

**Narrow terminal.** 80 columns: 3b. Below 70 columns: one pane at a time
under a `Library | Kit` tab row; Tab swaps; the `▷` sync becomes "jump to it"
when the user switches pane. Below 40x12 the screen shows only "ccs needs at
least 40x12 · q quits" and keeps taking `q` and Ctrl-C.

**Kits switched for one launch.** Today `ccs <profile> +kit` and Home's ticks
can change kits for one run. In the Builder, `r` runs the profile with its
attached kits. To run with other kits once, the Load view takes `+`/`-` on a
kit (its `by kit` tab), shows the result, and `r` from there returns
`Exit::LaunchWith`; nothing is saved.

## 9. Changes behind the screen

### New or changed `Browser` methods

The TUI must still import only `crate::ports`. New value types are defined in
the domain and re-exported by the port, as today.

| Method | Returns | Why |
|---|---|---|
| `catalog(&self) -> Catalog` | Every item of every kind with its folder path and description; the folder tree with counts; each plugin's own skills and agents. | One call builds the Library tree. Today `library(kind)` is called per kind, and a plugin is one item with no contents. |
| `plugin_contents(&self, plugin: &str) -> Result<Vec<LibItem>, CcsError>` | The skills and agents inside one plugin, with descriptions. | 3d and the Kit pane. (Can be part of `Catalog`; listed separately because it reads a git clone and may be slow.) |
| `kit_view(&self, kit: &str) -> Result<KitView, CcsError>` | Per `uses` line: the line, its kind (item, folder, plugin), the items it gives, and a status (ok, empty folder, missing). Plus overlaps (one item from two lines) and name clashes. | The Kit pane. Today `kit_uses` gives the lines and the TUI calls `resolve_kit`, which returns the items and the missing lines but not which line gave which item. |
| `load_plan(&self, profile: &ProfileName, kits: Option<&[String]>) -> Result<LoadPlan, CcsError>` | Each item with its kit and line, the name Claude will see, the plugin dirs, the `claude` argv, the instructions in use, and the problems. | The load bar and S2. It must be built by the same use case the launcher uses, so the view cannot disagree with what starts. |
| `usage(&self) -> Usage` | For each library entry, the kits that use it; for each kit, the profiles that start with it. | Inspector, "Used by", the dim kit names in §7. |
| `replace_use(&self, kit: &str, old: &Use, new: &[Use]) -> Result<(), CcsError>` | | Split a folder into items, or fold items into a folder, in one write, so a failure cannot leave a half-changed kit. |
| `copy_kit(&self, kit: &str, from: &str) -> Result<(), CcsError>` | | `N`. |
| `name_check(&self, name: &str) -> NameCheck` | Free, taken by a kit, or shared with a plugin. | The live check in O3. |
| `hidden(&self) -> Vec<Use>` and `set_hidden(&self, entry: &Use, on: bool) -> Result<(), CcsError>` | | `h` and `.`; stored under `$CCS_HOME`. |

`overview`, `kits`, `attach`, `new_kit`, `remove_kit`, `rename_kit`,
`set_use`, `kit_instructions`, `edit_kit`, `read`, `edit`, `add`,
`update_plugins`, `mkdir`, `move_entry`, `rename_folder`, `remove`,
`new_profile`, `edit_profile`, the import and migrate methods stay as they
are. `kit_uses` and `library(kind)` can stay for the CLI and tests, but the TUI
stops calling them.

### CLI for parity

| TUI | CLI | Status |
|---|---|---|
| pick / unpick | `ccs kit use <k> <entry>…`, `ccs kit drop <k> <entry>…` | exists |
| new kit | `ccs kit new <k>` | exists |
| copy a kit | `ccs kit new <k> --from <k2>` | new |
| rename, delete, edit CLAUDE.md | `ccs kit rename`, `ccs kit rm`, `ccs kit edit` | exists |
| attach / detach | `ccs kit attach/detach <k> <profile>` | exists |
| what a kit holds, by line | `ccs kit show <k>` | exists; must print each line with its items under it, and missing lines, overlaps and clashes |
| split a folder / fold items into a folder | `ccs kit replace <k> <old> <new>…` | new (same as `replace_use`); `use` + `drop` already get there in two steps |
| what is inside a plugin | `ccs library show plugins/<p>` | must list its skills and agents |
| available only | `ccs library list [kind] --not-in <k>` | new |
| what a profile loads | `ccs loads <profile> [+kit…]` | new; prints the 3e table and the `claude` command |
| hide a source | none | a view setting; it changes nothing ccs loads, so it needs no command |
| run | `ccs <profile> [+kit…]` | exists |

### Tests and gate

The gate for building this is a set of `tests/tui.rs` cases written first, one
per job, each drawing into `TestBackend` at 80x24 and 120x36 and reading the
text: the Kit pane lists the 10 inspect skills under `skills/network/inspect/`;
a `[^]` row exists for a folder-given item; `u` hides picked rows; the
success-test keys in §5, sent as `Key`s against a fake `Browser`, end with the
Load view showing `9 skills · 9 agents · 1 plugin`; and a real-`App` test
that `load_plan` and the launcher produce the same argv.

## 10. Weaknesses

- **It is a builder first and a launcher second.** Most sessions are "start
  Claude with my usual profile". Today's Home is built for that; here it is
  `r` on a screen whose space goes to editing. The strongest counter-argument
  to this design is exactly this: the user edits kits rarely and launches
  daily, so the screen they see most is tuned for the rarer job. The answer
  here is that `ccs <profile>` already launches with no screen, and the
  Builder opens on the profile's first kit with the load bar showing what `r`
  will start. That is still one more thing on screen than a launcher needs.
- **Two panes halve the width.** At 80 columns each pane has 38 columns, and
  names like `investigate-collection-failure` with indent and mark get cut.
  The detail line covers it, but a single full-width list (a picker with
  marks, no cart) reads better on a small terminal.
- **The cart picture hides that a folder pick is a rule.** Showing a folder's
  items expanded makes it look like a list of items. A user may not notice
  that `skills/network/inspect/` will also take the next skill added there.
  The `folder` tag and "folder lines take new items" help, but the expanded
  view leans toward the wrong reading.
- **The relation between profiles and kits is less visible.** Only one
  profile shows at a time; seeing which profiles use which kits takes the kit
  picker or the Inspector. A profile-centred design shows that relation
  directly.
- **The same item can appear three times on screen** (Library row, Kit pane,
  Inspector). That is the point of the sync cursor, but at 160 columns it is a
  lot of repeated text.
- **More marks to learn.** Four marks (`[x] [^] [-] [ ]`) plus `·` inside
  unpicked plugins. The legend sits in the Library pane's bottom border at 120,
  but at 80 it does not fit, and `[^]` is not a glyph anyone has seen before.
- **Bigger port.** The design needs `catalog`, `kit_view`, `load_plan` and
  `usage`, which push resolution work that the TUI does today into use cases.
  That is the right place for it, but it is more change behind the screen
  than a redesign that only redraws.

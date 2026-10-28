# Design 3: Profile-first loadout board

## 1. Summary

1. The main screen is one profile's **loadout**: every kit it starts with, and under each kit what that kit actually loads, by kind, with counts. A summary bar above it gives the totals.
2. A row shows how it got there: picked alone (`●`), picked as a folder (`◉`, "new ones load too"), brought in by a folder (`○`), inside a plugin (`◇`), or missing (`✗`). Jobs 1 and 4 are about these marks.
3. You add things where you are. `a` opens a quick-add search over the whole library. It marks what is already picked and asks which kit to put the result in. "+ new kit" is offered in the same list (jobs 4 and 5).
4. The Library is a second view. Each row in it says which kits and profiles use it, and folders and plugins open in place to show what they hold (jobs 2 and 3).
5. Every change prints the `ccs` command it ran on the bottom line. The TUI teaches the CLI, and nothing exists only in the TUI.

## 2. Screen inventory

| # | Screen / pane / overlay | What it is for | How you get there | How you leave |
|---|---|---|---|---|
| S1 | **Board** (main) | The loadout of one profile. Kits are collapsible groups, then kinds, then entries. | `ccs ui`, `ccs` with no profile on a terminal, `1`, or click `Board` | `q` quits; `s` starts Claude |
| S1a | Profile bar (top line of S1) | Lists the profiles. One of them is the board's subject. | always visible | `[` `]` switch, click a name, `p` opens the list |
| S1b | Summary bar (second line) | "What will I get": totals for the profile, plus the exact start command | always visible | click `⏎ start` or press `s` to launch |
| S1c | Detail pane (right; at 80 cols, a 3-line strip at the bottom) | Facts about the highlighted row: what it is, why it is here, who else uses it, what it holds, its CLI command | follows the cursor | `Tab` moves focus into it for scrolling |
| S1d | "Not on this profile" drawer (last group on the board) | Kits that exist but this profile does not start with. Space attaches one. | scroll to the bottom, or `End` jumps to it | Esc / move away |
| S1e | Inline name field | Names a new kit, or renames one, on the board line itself | `n`, `r` | Enter, Esc |
| S2 | **Quick-add** overlay | Fuzzy search over skills, agents, plugins and folders. Shows marks for the target kit. Adds one or many. | `a`; Enter on a `+ add to <kit>…` row; click that row | Esc |
| S2a | "Into which kit?" step | Chooses the target kit. This profile's kits come first, then other kits, then `+ new kit "<typed>"`. | Enter in S2 when no target kit is set yet | Enter, Esc |
| S3 | **Library** view | The whole library as a tree: skills, agents and plugins. Each row has a "used by" column. | `2`, or click `Library` | `1` or Esc returns to the board |
| S4 | **Command palette** | Every action, by name, with its key and its CLI form. Use it to find a key you don't know. | `:` | Enter runs; Esc |
| S5 | Text view | Reads a SKILL.md, an agent file, a plugin README or a kit's CLAUDE.md | `v` | Esc, `q`, right click |
| S6 | Confirm line (on the footer) | Confirms a change to a kit that another profile also uses, and deletes | triggered | `y` / `c` / Esc |
| S7 | Context menu | Right click on a row lists what you can do to it, with each key | right click | click, Esc |

There is one rule for moving around. `1` and `2` switch the two views. Every overlay closes with Esc. Esc on the board does nothing, so a stray Esc cannot quit. `q` quits from any view, but inside an overlay it is typed as text.

## 3. Mockups

Legend used in all mockups:

| Mark | Meaning |
|---|---|
| `●` | picked by its own `uses` line |
| `◉` | a folder picked by one `uses` line: everything under it loads, including items added later |
| `○` | loads because a folder above it is picked; it has no `uses` line of its own |
| `◆` | a plugin, picked by its own `uses` line |
| `◇` | a skill or agent inside a plugin: it loads with the plugin and cannot be picked alone |
| `✗` | a `uses` line that no longer matches anything in the library |
| `▶` `▼` | collapsed / expanded |
| `[x]` `[~]` `[ ]` | in quick-add and the library, relative to the target kit: picked / brought in by a picked folder / not picked |
| `⇄` | this kit is also on another profile |

### (a) Board at 120x36: claude-bedrock-gary

The cursor (reverse video, shown here as `▌`) is on the folder line `skills/network/inspect/` in kit `test`.

```
 ccs  [1 Board] 2 Library   profile ◀ claude-bedrock-gary ★  sticksandstrings ▶    bedrock claude-bedrock-gary@us-east-1
 LOADS 4 kits · 31 skills · 0 agents · 1 plugin (+2 skills inside) · 0 CLAUDE.md [⏎ s start claude-bedrock-gary +4 kits]
┌ Board: what claude-bedrock-gary starts with ────────────────────────┐┌ skills/network/inspect/ ──────────────────────┐
│ ▼ base                                            empty      ⇄ none ││ folder · 10 skills                            │
│     (nothing picked)                  + add to base…                ││                                               │
│ ▼ writing                                         2 skills          ││ WHY IT LOADS                                  │
│   Skills 2                                                          ││ kit test has the line                         │
│     ◉ skills/writing/                         folder · 2            ││   skills/network/inspect/                     │
│        ○ forward-generate-report                                    ││ so every skill in this folder loads, and any  │
│        ○ humanize-writing                                           ││ skill added to it later.                      │
│     + add to writing…                                               ││                                               │
│ ▼ slides                                          1 plugin          ││ ALSO USED BY                                  │
│   Plugins 1                                                         ││ kits      test (only)                         │
│     ◆ plugins/forward-decks                   2 skills · 0 agents ▶ ││ profiles  claude-bedrock-gary                 │
│     + add to slides…                                                ││                                               │
│ ▼ test                                            29 skills         ││ INSIDE (10)                                   │
│   Skills 29                                                         ││ compare-device-config  inspect-history        │
│     ◉ skills/forward-dev/                     folder · 2          ▶ ││ inspect-bgp-neighbors  inspect-inventory      │
│     ◉ skills/horizon/                         folder · 2          ▶ ││ inspect-device-files   inspect-networks       │
│▌    ◉ skills/network/inspect/                 folder · 10         ▼ ││ inspect-edge           inspect-snapshots      │
│        ○ compare-device-config                                      ││ inspect-topology       inspect-vulnerabilities│
│        ○ inspect-bgp-neighbors                                      ││                                               │
│        ○ inspect-device-files                                       ││ NOT PICKED IN THIS FOLDER'S PARENT            │
│        ○ inspect-edge                                               ││ skills/network/ has 6 more folders (25 skills)│
│        ○ inspect-history                                            ││ that test does not load: change/ investigate/ │
│        ○ … 5 more (→ to show)                                       ││ nqe/ review/ …   a to add                     │
│     ◉ skills/network/plan/                    folder · 15         ▶ ││                                               │
│     + add to test…                                                  ││                                               │
│ + new kit for claude-bedrock-gary                                   ││                                               │
│ ─ not on this profile (4) ──────────────────────────────────────    ││ CLI                                           │
│   [ ] claude-flow   76 agents                                       ││ ccs kit show test                             │
│   [ ] dev           18 agents · 1 plugin                            ││ ccs kit drop test skills/network/inspect/     │
│   [ ] forward-dev   4 skills                                        ││                                               │
│   [ ] network       35 skills · 2 agents                            ││                                               │
└─────────────────────────────────────────────────────────────────────┘└───────────────────────────────────────────────┘
 x drop from test · → expand · v view · a add to test · e edit · g group by kind · [ ] profile · : all actions · q quit
 ✓ ready · 8 kits · library 41 skills, 96 agents, 2 plugins
```

Notes on (a):

- The summary bar counts each item once, after removing duplicates across kits. Plugin contents are counted separately ("+2 skills inside"), because a plugin is one `uses` line but loads many things.
- The kind sub-heading (`Skills 29`) is drawn only when a kit holds more than one kind, or when the terminal is wider than 100 columns. At 80 columns it folds into the count on the kit line.
- A folder line shows its count on the right. The `○` children below it are dim, because they are a consequence of the folder line and you do not edit them one by one.
- `⇄ none` on `base` says no other profile uses this kit. Kit `test` would show `⇄ none` too. If `sticksandstrings` also had `writing`, its header would read `⇄ sticksandstrings`.
- The "not on this profile" drawer means a kit is never hidden just because it is not attached here. `[ ]` plus space attaches it.

### (b) The same board at 80x24

The detail pane becomes a three-line strip. Profile names shrink to tabs. The kind sub-headings fold into the kit lines.

```
 ccs [1 Board] 2 Lib   ◀ claude-bedrock-gary ★ │ sticksandstrings ▶
 LOADS 4 kits · 31 skills · 0 agents · 1 plugin(+2)   [⏎ s start]
┌ claude-bedrock-gary ─────────────────────────────────────────────────────────┐
│ ▼ base                    empty                                              │
│ ▼ writing                 2 skills                                           │
│     ◉ skills/writing/              folder · 2                              ▶ │
│ ▼ slides                  1 plugin                                           │
│     ◆ plugins/forward-decks        plugin · 2 skills                       ▶ │
│ ▼ test                    29 skills                                          │
│     ◉ skills/forward-dev/          folder · 2                              ▶ │
│     ◉ skills/horizon/              folder · 2                              ▶ │
│▌    ◉ skills/network/inspect/      folder · 10                             ▶ │
│     ◉ skills/network/plan/         folder · 15                             ▶ │
│ + new kit                                                                    │
│ ─ not on this profile (4): claude-flow · dev · forward-dev · network ─────── │
└──────────────────────────────────────────────────────────────────────────────┘
 skills/network/inspect/ · folder, 10 skills · picked as a folder by test
 used by kits: test · profiles: claude-bedrock-gary
 ccs kit drop test skills/network/inspect/
 x drop · → open · a add · : all actions · q quit
 ✓ ready
```

At 80x24 the `+ add to <kit>…` rows are hidden, because they cost one line per kit. `a` still adds to the kit the cursor is in, and the hint line says so ("a add to test").

### (c) Making a new kit

Frame 1: on `sticksandstrings`, which starts with no kits. The empty board says what to do next. The user pressed `n`, and the new group's header became a text field.

```
 ccs   [1 Board]  2 Library    profile ◀ claude-bedrock-gary ★  sticksandstrings ▶                             anthropic
 LOADS nothing yet: 0 kits                                                         [⏎ s start sticksandstrings, no kits]
┌ Board: what sticksandstrings starts with ───────────────────────────┐┌ new kit ──────────────────────────────────────┐
│ ▼ kit name: research▏                                               ││ A kit is a named list of picks from the       │
│     will be made and attached to sticksandstrings                   ││ library, plus optional CLAUDE.md text.        │
│                                                                     ││                                               │
│ ─ not on this profile (8) ──────────────────────────────────────    ││ Enter makes it and attaches it to             │
│   [ ] base          empty                                           ││ sticksandstrings. The cursor then lands on    │
│   [ ] claude-flow   76 agents                                       ││ "+ add to research…".                         │
│   [ ] dev           18 agents · 1 plugin                            ││                                               │
│   [ ] forward-dev   4 skills                                        ││ Name rules: letters, digits, - and _.         │
│   [ ] network       35 skills · 2 agents                            ││ "research" is free.                           │
│   [ ] slides        1 plugin             ⇄ claude-bedrock-gary      ││                                               │
│   [ ] test          29 skills            ⇄ claude-bedrock-gary      ││ CLI                                           │
│   [ ] writing       2 skills             ⇄ claude-bedrock-gary      ││ ccs kit new research                          │
│                                                                     ││ ccs kit attach research sticksandstrings      │
└─────────────────────────────────────────────────────────────────────┘└───────────────────────────────────────────────┘
 enter make · esc cancel
```

Frame 2: after Enter, the user presses Enter on `+ add to research…`. Quick-add opens with its target set to research. They type `writing`, mark two rows with space, and the next Enter adds both. The plugin `egm` was added a moment earlier, and the echo line shows its command.

```
 ccs   [1 Board]  2 Library    profile ◀ claude-bedrock-gary ★  sticksandstrings ▶                             anthropic
 LOADS 1 kit · 0 skills · 0 agents · 1 plugin (+7 skills, 8 agents inside)        [⏎ s start sticksandstrings +research]
┌ Board ┌ Add to kit research ─────────────────────────────────────────────────────────────────────────────┐───────────┐
│ ▼ res │ > writing▏                                    4 matches of 137 · tab: kind                       │           │
│     ◆ │                                                                                                  │           │
│     + │   [ ] ◉ skills/writing/                         folder · 2 skills   kits: writing                │           │
│ + new │   [•] ● skills/writing/forward-generate-report  skill             kits: writing (folder)         │           │
│ ─ not │▌  [•] ● skills/writing/humanize-writing         skill             kits: writing (folder)         │           │
│   [ ] │   [ ] ◇ plugins/forward-decks › …-retemplate   in a plugin: add the plugin instead               │           │
│   [ ] │                                                                                                  │           │
│   [ ] │ humanize-writing                                                                                 │           │
│   [ ] │ Aggressively rewrite AI-generated text into natural human prose in the user's voice.             │           │
│   [ ] │                                                                                                  │           │
│   [ ] │ 2 marked · enter add 2 to research · space mark · tab kind · esc close                           │           │
│   [ ] └──────────────────────────────────────────────────────────────────────────────────────────────────┘           │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
 ✓ added plugins/egm to research    ccs kit use research plugins/egm
```

Notes on (c):

- `[•]` means marked in this overlay, waiting for Enter. After Enter, those rows show `[x]`.
- The fourth result, from the `forward-decks` plugin, shows that search also looks inside plugins. That row cannot be picked. Space on it says "a skill inside a plugin loads with the plugin; add plugins/forward-decks instead?"
- Results are ordered: folder first, then items, then plugin contents. Within each group, an exact name match comes first.
- When quick-add opens without a target kit (`a` on a profile line, or from the Library), Enter goes on to step S2a:

```
 ┌ Into which kit? ──────────────────────────────┐
 │ > ▏                                           │
 │ on sticksandstrings                           │
 │▌  research                                    │
 │ other kits                                    │
 │   base · claude-flow · dev · forward-dev ·    │
 │   network · slides · test · writing           │
 │ + new kit "…" (type a name)                   │
 └───────────────────────────────────────────────┘
```

  Typing a name that does not exist turns the last row into `+ new kit "research2" (attach to sticksandstrings)`.

### (d) Opening a plugin to see what is inside

Board for sticksandstrings after the walkthrough. The cursor is on `plugins/egm`, and `→` has expanded it.

```
 ccs   [1 Board]  2 Library    profile ◀ claude-bedrock-gary ★  sticksandstrings ▶                             anthropic
 LOADS 1 kit · 2 skills · 0 agents · 1 plugin (+7 skills, 8 agents inside)        [⏎ s start sticksandstrings +research]
┌ Board: what sticksandstrings starts with ───────────────────────────┐┌ plugins/egm ──────────────────────────────────┐
│ ▼ research                      2 skills · 1 plugin           ⇄ none││ plugin · git clone · updated 3 days ago       │
│   Skills 2                                                          ││ "Draft specs, review designs, and check       │
│     ● skills/writing/forward-generate-report                        ││  implementation drift."                       │
│     ● skills/writing/humanize-writing                               ││                                               │
│   Plugins 1                                                         ││ A plugin loads whole. Its skills and agents   │
│▌    ◆ plugins/egm                      7 skills · 8 agents       ▼  ││ cannot be picked one by one.                  │
│       Skills 7                                                      ││ In Claude Code, its skills are named egm:…    │
│        ◇ egm_bootstrap        ◇ egm_draft      ◇ egm_resolve        ││                                               │
│        ◇ egm_brainstorm       ◇ egm_drift_report  ◇ egm_review      ││ ALSO USED BY                                  │
│        ◇ egm_review_loop                                            ││ kits      dev, research                       │
│       Agents 8                                                      ││ profiles  sticksandstrings (via research)     │
│     ◇ bootstrap_orchestrator  ◇ comprehension   ◇ structural        ││           none via dev                        │
│        ◇ codebase_details        ◇ design_critic   ◇ style          ││                                               │
│        ◇ implementation_readiness   ◇ synthesizer                   ││ ALSO IN THE CLONE                             │
│     + add to research…                                              ││ commands 0 · hooks 0 · MCP servers 0          │
│ + new kit for sticksandstrings                                      ││                                               │
│ ─ not on this profile (8) ──────────────────────────────────────    ││ v README · u git pull · x drop from research  │
│   [ ] base · claude-flow · dev · forward-dev · network · slides ·   ││                                               │
│       test · writing                                                ││ CLI                                           │
│                                                                     ││ ccs library show plugins/egm                  │
│                                                                     ││ ccs kit drop research plugins/egm             │
└─────────────────────────────────────────────────────────────────────┘└───────────────────────────────────────────────┘
 → ← open/close · v view egm_review · x drop egm from research · u update · : all actions · q quit
 ✓ added 2 to research    ccs kit use research skills/writing/forward-generate-report skills/writing/humanize-writing
```

The agent names in (d) come from the plugin's own manifest. The brief says egm holds 8 agents, and the mockup shows 8. The clone on disk holds 9 files under `agents/`. The design shows what the plugin declares, and the detail pane gives the declared count. Section 9 explains how ccs reads that count. If the manifest and the files disagree, the board says so (see edge cases).

`◇` rows can be read (`v`) but not dropped. `x` on a `◇` row says "drop plugins/egm to remove these 15; a plugin cannot be split".

### (e) Library view with "used by"

`2` from the board. The left column chooses a source, and the main list is a tree with usage columns. The filter is `/coord`, which matches names and descriptions.

```
 ccs   1 Board  [2 Library]   adding to: research (sticksandstrings) · t to change     41 skills · 96 agents · 2 plugins
 / coord▏    kind: all   [ ] unused only   [x] group near-duplicates   hidden: none                      12 of 137 shown
┌ Sources ──────────────┐┌ Library ──────────────────────────────────────────────┬─ in kits ───────┬─ on profiles ─────┐
│ ▼ skills          41  ││ ▼ ◉ agents/claude-flow/                  76 agents    │ claude-flow     │ —                 │
│   forward-dev/   2    ││   ▼ consensus/                            7           │ ″               │ ″                 │
│   horizon/       2    ││       byzantine-coordinator                           │ ″               │ ″                 │
│   methods/       0    ││       gossip-coordinator                              │ ″               │ ″                 │
│   network/      35    ││   ▼ swarm/                                3           │ ″               │ ″                 │
│   writing/       2    ││     ┌ *-coordinator, 3 alike                          │                 │                   │
│ ▼ agents          96  ││▌    │ hierarchical-coordinator  Queen-led…            │ ″               │ ″                 │
│   claude-flow/  76    ││     │ mesh-coordinator          Peer-to-pe…           │ ″               │ ″                 │
│   dev/          18    ││     │ adaptive-coordinator      Dynamic to…           │ ″               │ ″                 │
│   network/       2    ││   ▼ templates/                            9           │                 │                   │
│ ▼ plugins          2  ││       coordinator-swarm-init                          │ ″               │ ″                 │
│   egm      7s 8a      ││       memory-coordinator                              │ ″               │ ″                 │
│   forward-decks 2s    ││       sparc-coordinator                               │ ″               │ ″                 │
│                       ││   ▶ v3/  (1 match)                       16           │ ″               │ ″                 │
│ H hide a source       ││ ▼ ◉ agents/network/                       2 agents    │ network         │ —                 │
│ hidden: none          ││     (no match)                                        │                 │                   │
│                       ││ ▼ ◆ plugins/egm                  7 skills · 8 agents  │ dev, research   │ sticksandstrings  │
│ USED-BY KEY           ││     ◇ synthesizer  (matches its description)          │ ″               │ ″                 │
│ ″ same as above       ││                                                       │                 │                   │
│ — on no profile       │├─ hierarchical-coordinator ──────────────────────────────────────────────────────────────────┤
│                       ││ agent · agents/claude-flow/swarm/hierarchical-coordinator.md                                │
│                       ││ Queen-led hierarchical swarm coordination with specialized worker delegation                │
│                       ││ in kit claude-flow through the folder line agents/claude-flow/ · on no profile              │
│                       ││ [ ] not in research · space adds this one agent to research                                 │
└───────────────────────┘└─────────────────────────────────────────────────────────────────────────────────────────────┘
 space add to research · t change kit · / filter · U unused only · H hide source · → open · v view · 1 board · q quit
 ccs library list agents --used-by
```

Notes on (e):

- The library always has a **target kit**, shown at the top ("adding to: research"). It is the kit the cursor was in on the board, and `t` changes it. Without a target, space does nothing and the hint says "t choose a kit to add to". This is the one mode in the design, and it is visible on screen at all times.
- The "in kits" column gives the kits that pick the row. `″` means "same as the folder line above". This keeps 76 claude-flow rows from repeating "claude-flow". "on profiles" goes through kits to the profiles that start with them. `—` means no profile uses it, so it is in the library but loads nowhere.
- "group near-duplicates" boxes rows in the same folder that share a word stem (`*-coordinator`). The box only groups rows visually. It does not merge them.
- At 160x50 this view gains a fourth column, the description, and the bottom detail strip grows to show the first 12 lines of the file.

### Using the extra space at 160x50

- The board and the Library sit side by side. The left 90 columns hold the board, and the right 70 hold the Library, whose target is the kit under the board cursor. Space in the library pane adds to that kit, and the board updates live. This is the "playlist editor" layout, and it appears when there is room for it.
- The detail pane moves under the board as a 12-line strip.
- On the board, `○` children show their descriptions in dim text.
- The profile bar shows each profile's kit count: `claude-bedrock-gary ★ (4)  sticksandstrings (1)`.

## 4. Key map and mouse map

A key means the same thing on every screen. Where a key has no meaning on the current row, the hint line leaves it out instead of graying it.

| Key | Board (S1) | Library (S3) | Quick-add (S2) | CLI equivalent |
|---|---|---|---|---|
| `↑` `↓` / `j` `k` | move | move | move in results | — |
| `→` / `l` | expand a kit, folder or plugin | expand | — | — |
| `←` / `h` | collapse, or go to the parent row | collapse / parent | — | — |
| `Enter` | expand or collapse; on `+ add…` open quick-add; on `+ new kit` name it | expand / collapse | add the marked rows, or the highlighted row | — |
| `space` | on a `[ ]` kit in the drawer: attach it | add to or drop from the target kit | mark / unmark | `ccs kit attach` / `ccs kit use` / `ccs kit drop` |
| `x` | drop the row from the thing that holds it: a pick from its kit, a kit from this profile | drop from the target kit | — | `ccs kit drop <k> <e>` / `ccs kit detach <k> <p>` |
| `a` | quick-add, targeting the kit the cursor is in | quick-add, targeting the current target kit | — | `ccs kit use <k> <e>…` |
| `n` | new kit, attached to this profile | new kit, made the target | (`+ new kit` row) | `ccs kit new <k>` + `ccs kit attach <k> <p>` |
| `r` | rename the kit (inline) | rename a folder | — | `ccs kit rename`, `ccs library rename` |
| `e` | on a kit: edit its CLAUDE.md; on an item: edit its file | edit the item | — | `ccs kit edit`, `ccs library edit` |
| `v` | view the file (S5) | view | view the highlighted result | `ccs library show <e>` |
| `g` | group by kit, or by kind (one merged list with kit tags) | — | — | `ccs loadout <p> --by-kind` |
| `[` `]` | previous / next profile | — | — | — |
| `p` | profile list: switch, make, copy, set default | — | — | `ccs list`, `ccs new`, `ccs default` |
| `t` | — | choose the target kit | choose the target kit | — |
| `/` | filter the board | filter the library | (typing filters already) | `ccs library list … ` |
| `Tab` | move focus between panes | cycle the kind filter: all, skills, agents, plugins | cycle the kind filter | — |
| `u` | undo the last change (one level, shown as `u undo: drop …`) | undo | — | — |
| `U` | — | toggle "unused only" | — | `ccs library list --unused` |
| `H` | — | hide or show the highlighted source (a top folder or a plugin) | — | `ccs library hide <e>` / `unhide` |
| `s` | start Claude with this profile and its kits | same | — | `ccs <profile>` |
| `1` `2` | Board / Library | Board / Library | — | — |
| `:` | command palette (S4) | command palette | — | — |
| `Esc` | close overlay or filter; never quits | back to board | close | — |
| `q`, `Ctrl-C` | quit | quit | (`q` is text) / quit | — |

Mouse:

| Mouse action | Effect |
|---|---|
| Click a row | select it |
| Click the selected row again | same as Enter |
| Click `▶` / `▼` | expand / collapse |
| Click a `[ ]` box | same as space |
| Click a profile name in the top bar | switch the board to that profile |
| Click `1 Board` / `2 Library` | switch view |
| Click `[⏎ s start …]` | start (single click; this button is the only control that launches) |
| Click `+ add to <kit>…` / `+ new kit` | open quick-add for that kit / name a new kit |
| Right click on a row | context menu (S7): every action for that row, each with its key |
| Right click on empty space | close an overlay; on the board, nothing |
| Wheel | scroll the pane under the pointer |
| Click a CLI line in the detail pane | copy it to the clipboard (OSC 52), when the terminal supports it |

**Help without a help screen.** Four places show the keys, and none of them is a separate page:
1. The **hint line** lists only the keys that act on the highlighted row, with the row's verb in them ("x drop from test", "space attach dev"). Since it changes with the cursor, it is short.
2. **Action rows** (`+ add to test…`, `+ new kit for …`, the `[ ]` drawer) are real rows in the list. The next step can be reached with the cursor, not only with a key you have to remember.
3. The **right-click menu** and the **`:` palette** list every action by name with its key. Typing "attach" in `:` finds it.
4. The **CLI echo line** prints the `ccs` command after each change.

## 5. Walkthrough of the success test

Goal: make a kit `research`, add plugin egm and two skills from writing/, attach it to sticksandstrings, then see everything sticksandstrings loads. The user has not seen the help.

Start: `ccs ui` (not counted). The board opens on the default profile, claude-bedrock-gary.

| # | Keys | Count | What is on screen, and what tells the user what to do next |
|---|---|---|---|
| 1 | `]` | 1 | The profile bar shows `◀ … ▶`, and the hint line says `[ ] profile`. The board is now for sticksandstrings: "LOADS nothing yet: 0 kits", a highlighted `+ new kit for sticksandstrings` row, and the drawer of 8 kits. |
| 2 | `n` (or Enter on the highlighted row) | 1 | The header becomes `▼ kit name: ▏`, with "will be made and attached to sticksandstrings". |
| 3 | `r e s e a r c h` | 8 | The detail pane says `"research" is free`. |
| 4 | `Enter` | 1 | Kit made and attached. Echo line: `ccs kit new research && ccs kit attach research sticksandstrings`. The cursor is on `+ add to research…`. |
| 5 | `Enter` | 1 | Quick-add opens, titled "Add to kit research". |
| 6 | `e g m` | 3 | The top result is `◆ plugins/egm  7 skills · 8 agents  kits: dev`. |
| 7 | `Enter` | 1 | Added. Echo: `ccs kit use research plugins/egm`. The overlay stays open with the query selected, so typing replaces it. |
| 8 | `w r i t i n g` | 7 | Results: the folder `skills/writing/`, then its two skills. |
| 9 | `↓` `space` `↓` `space` | 4 | Both skills are marked `[•]`. The overlay footer says "2 marked · enter add 2 to research". |
| 10 | `Enter` | 1 | Both added. Echo: `ccs kit use research skills/writing/forward-generate-report skills/writing/humanize-writing`. |
| 11 | `Esc` | 1 | Back on the board. The summary bar reads `LOADS 1 kit · 2 skills · 0 agents · 1 plugin (+7 skills, 8 agents inside)`. |
| 12 | `g` | 1 | "By kind": one list of 9 skills and 8 agents. Each row is tagged with the kit it comes from, and the plugin rows are marked `◇ egm`. This is everything sticksandstrings loads. |

**Total: 30 keystrokes** (18 of them typed letters). At normal typing speed that is 20 to 30 seconds. Step 12 is optional. After step 11 the summary bar already gives the totals, and the research group shows the two skills and the plugin line. `→` on the plugin line (1 more key, plus moving to it) opens egm in place, as in mockup (d).

Things a first-time user might do instead, and what happens:
- Pressing `a` on the empty board, before making a kit. Quick-add opens with no target, so Enter goes to "Into which kit?". There, typing `research` offers `+ new kit "research" (attach to sticksandstrings)`. This route takes 3 more keystrokes and gives the same result.
- Picking the folder `skills/writing/` instead of the two skills. The result looks different: the board shows `◉ skills/writing/ folder · 2` and not two `●` lines. The detail pane explains what that means: "new skills added here load too".

## 6. How each job is solved

**(1) Show what a kit holds.** On the board, a kit is a group whose lines are its `uses` lines, each with a mark that says how it picks. Under a folder line, the board shows the items that folder brings in, as `○` children. Under a plugin line, it shows the plugin's skills and agents as `◇`. Every count is real: "29 skills" is what test loads after folders are expanded. A kit that is on no profile still has a group in the drawer, and Enter opens it the same way.

**(2) Show what the library holds and what is inside it.** The Library view is one tree across skills, agents and plugins. A plugin opens like a folder, and its rows are marked `◇` to show that they come with the plugin. The Sources column gives counts per top folder and per plugin, so "what is there" can be answered before reading a single row. Quick-add searches inside plugins too, so a skill that only exists in a plugin can be found by name.

**(3) Show what is available but not picked.** There are three places. The detail pane of a folder line lists the sibling folders the kit does not load. The Library's "unused only" toggle (`U`) shows items no kit picks. The `[ ]` marks in quick-add and the Library show what the target kit lacks. On the board, the drawer lists the kits that exist but are not on this profile.

**(4) Pick and unpick with no guessing.** Every place you can pick names its target kit: the quick-add title, the Library's "adding to:" line, and the hint line. One key adds (space or Enter), and one key drops (`x`). After each change, the echo line gives the exact command that ran, and `u` undoes it. `●`, `◉` and `○` are drawn differently on purpose. An item that only comes in through a folder cannot be dropped alone, and the screen says why: "comes from skills/writing/; drop the folder, or replace it with single picks".

**(5) Make a new kit.** There are three ways in: `n` on the board, the `+ new kit` row, and `+ new kit "<typed>"` in the "Into which kit?" step. All three end with the kit attached to the profile you are looking at and the cursor on `+ add to <kit>…`. So making a kit leads directly into filling it.

## 7. Scale and noise

The 96 agents are 76 claude-flow, 18 dev and 2 network. Many of the claude-flow names differ only in a prefix, such as `hierarchical-`, `mesh-` and `adaptive-coordinator`.

- **Folders first, items on demand.** Board and Library both open with folders collapsed at the first level below the source. `claude-flow/` is one line ("76 agents"), not 76. On the board, a folder line's `○` children start collapsed if there are more than 5 of them, with "… N more (→ to show)".
- **Grouping by source.** The Library's left column is a list of sources: the top folders of each kind, plus each plugin. Clicking one scrolls to it, and `H` hides it.
- **Hiding a source.** `H` on `claude-flow/` hides it from the Library and from quick-add. A chip at the top says `hidden: claude-flow (76)`, so the counts stay honest. Hidden sources are still loaded by kits that pick them. A hidden source affects only what you browse and search, never what loads. This setting is kept in `$CCS_HOME/ui.toml` and can be set with `ccs library hide agents/claude-flow/`.
- **Near-duplicate grouping.** Inside one folder, names that share their last hyphen-separated word (`*-coordinator`, `*-manager`) and are three or more in number are boxed under a single label. The box is only visual and can be switched off. A detail line under each name repeats the first 40 characters of its description, which is usually the fastest way to tell the variants apart.
- **Search.** Quick-add and `/` match fuzzily on the name, the folder path and the first line of the description, in that order of weight. `Tab` restricts the search to one kind. A query that ends in `/` matches only folders.
- **Ordering in quick-add.** Within each kind, items the target kit lacks come before items it already has. Items from hidden sources are left out unless the query names the source exactly (`claude-flow/`).

## 8. Edge cases

**An item picked both directly and through a folder.** The board shows the direct `●` line with a dim tag `also via skills/writing/`. The folder's children list it as `○`, and the totals count it once. Dropping the `●` line leaves the item loaded, so the echo line says "still loads through skills/writing/". `:` has an action "remove redundant picks in <kit>", which drops only the `●` lines that a folder line in the same kit already covers. CLI: `ccs kit tidy <k>`.

**A uses line that no longer resolves.** It is drawn in place as `✗ skills/network/old-skill  not in library`, in red. The kit header and the summary bar add "1 missing", so the problem shows even when the kit is collapsed. Enter on the line opens quick-add with the last segment of the path as the query, so a renamed or moved item can be re-picked. `x` drops the line. The launch still works, and the `✗` line is skipped, as `ccs doctor` reports today.

**A plugin and a kit with the same name.** Kits are never results in item search, and every row carries a kind mark and a full path (`plugins/egm`). The only place where both names could appear together is the "Into which kit?" list, and that list holds only kits. In the "used by" columns, kits are listed by bare name under a header that says "in kits", and plugins never appear in that column.

**An empty library on a new host.** The board says `LOADS nothing yet`. Quick-add with no results shows two action rows: `+ add plugin from git: <typed text> as owner/repo` (when the text looks like `owner/repo` or a URL), and `⇣ import ~/.claude (copies, never changes it)`. The Library's Sources column shows each kind with 0 and the same two actions. Each row prints its CLI form (`ccs library add owner/repo`, `ccs import`).

**Very long names.** The leaf name is kept, and the folder path is shortened from the left: `…/development/backend/dev-backend-api`. If the leaf alone does not fit, it is cut in the middle so both ends stay visible (`collective-intel…-coordinator`). That way, near-duplicates that differ at the start or the end stay distinct. The detail pane and the hint line always show the full path. Kit names longer than 24 characters are cut on the board and shown in full in the header of the detail pane.

**A narrow terminal.** At 100 columns and above, the layout is (a). From 80 to 99, it is (b): the detail pane becomes a strip, kind sub-headings fold into the kit lines, and the `+ add` rows are hidden. From 60 to 79, the summary bar wraps onto two lines and the profile bar shows only the current profile with `◀ ▶`. Below 60, the board is a plain list, overlays fill the screen, and the hint line shows three keys plus `: more`. Under 24 rows, the detail strip drops to one line. Each width keeps the summary bar, because it is the main purpose of this design.

**A kit shared by two profiles.** Suppose `writing` is on both profiles. Its header shows `⇄ also on sticksandstrings`. The first change to a shared kit in a session puts a confirm line on the footer: `writing is also on sticksandstrings. y change both · c copy to writing-gary for this profile only · esc cancel`. `y` remembers the choice for that kit until the TUI exits. `c` runs `ccs kit copy writing writing-gary`, swaps the copy in for the original on this profile only, and then applies the change to the copy. The Library's "on profiles" column shows the same fact ahead of time.

**A kit on no profile** (today: claude-flow, dev, forward-dev, network). It appears in every profile's drawer. Enter on it opens it read-only in place, so you can see what it holds. Space attaches it. In the Library, `t` can make it the target without attaching it.

**A plugin whose manifest and files disagree** (egm: 8 declared, 9 agent files). The board uses the count from the manifest. The detail pane adds "9 agent files in the clone; 8 declared in plugin.json". The design never guesses which one Claude Code will load. It reports both.

## 9. Required changes behind the screen

The design needs four groups of data that the `Browser` trait in `src/ports/mod.rs` does not have today. Each method is a call into a use case. The TUI does no resolving itself, because the same answer must come out of the CLI. Per project rule 4, the new value types are defined in `domain/`, and `ports/` re-exports them.

```rust
// One profile's loadout: the answer the board draws.
fn loadout(&self, profile: &ProfileName) -> Result<Loadout, CcsError>;
// What a plugin holds (skills, agents, commands, hooks, MCP servers), from its manifest and files.
fn plugin_contents(&self, plugin: &str) -> Result<PluginContents, CcsError>;
// For every library entry (item, folder, plugin): which kits pick it and how; which profiles start with those kits.
fn usage(&self) -> UsageIndex;
// One kit, resolved like a loadout (used for drawer kits and the Library target).
fn kit_contents(&self, kit: &str) -> Result<KitLoad, CcsError>;
// Hidden sources for browsing and search.
fn hidden(&self) -> Vec<Use>;
fn set_hidden(&self, entry: &Use, hidden: bool) -> Result<(), CcsError>;
// Copy a kit (the shared-kit "c" answer) and tidy redundant picks.
fn copy_kit(&self, kit: &str, to: &str) -> Result<(), CcsError>;
fn tidy_kit(&self, kit: &str) -> Result<Vec<Use>, CcsError>;   // returns lines removed
```

```rust
pub struct Loadout { pub profile: ProfileName, pub kits: Vec<KitLoad>, pub totals: Totals, pub unattached: Vec<KitSummary> }
pub struct KitLoad { pub kit: String, pub lines: Vec<UseLine>, pub instructions: bool, pub shared_with: Vec<ProfileName>, pub totals: Totals }
pub struct UseLine { pub entry: Use, pub status: LineStatus, pub gives: Vec<LibItem>, pub also_via: Option<Use> }
pub enum LineStatus { Ok, Missing, Redundant { covered_by: Use } }
pub struct PluginContents { pub skills: Vec<String>, pub agents: Vec<String>, pub commands: usize, pub hooks: usize, pub mcp: usize, pub agent_files_on_disk: usize }
pub struct Totals { pub skills: usize, pub agents: usize, pub plugins: usize, pub inside_plugins: (usize, usize), pub missing: usize }
pub struct UsageIndex { /* path -> Vec<(kit, via: Use)>, kit -> Vec<ProfileName> */ }
```

Two rules for these:

- `loadout` must count each item once, across kits. It must report `Missing` for lines that do not resolve, not drop them. Per the CLAUDE.md lesson "never discard a deserialize error you did not expect": a missing item has to show as `✗` and must never show as a smaller count.
- `plugin_contents` reads the same manifest Claude Code reads. When it cannot read the manifest, it returns an error that the board shows. It does not fall back to counting files without saying so. Per the lesson "a gate that degrades silently is worse than no gate".

**CLI parity.** Every TUI action must also exist as a command. These are needed in addition to what `ccs --help` lists today:

| TUI action | CLI command | Exists today? |
|---|---|---|
| The board | `ccs loadout <profile> [--by-kind]`: prints the same groups, marks and counts as text | **new** |
| Open a plugin | `ccs library show plugins/egm` lists skills, agents and counts, after the description | extend (today it prints the file) |
| "Used by" columns | `ccs library list [kind] --used-by`, `ccs library who <entry>` | **new** |
| Hide a source | `ccs library hide <entry>` / `ccs library unhide <entry>` | **new** |
| New kit attached to the current profile | `ccs kit new <k> --attach <p>` (shorthand for the two commands) | **new flag** |
| Shared-kit copy | `ccs kit copy <k> <new>` | **new** |
| Remove redundant picks | `ccs kit tidy <k>` | **new** |
| Add several at once | `ccs kit use <k> <e>…` | exists |
| Drop, attach, detach, rename, edit | `ccs kit drop / attach / detach / rename / edit` | exist |
| Unused only | `ccs library list --unused` | **new flag** |
| Undo | none: the echo line shows the inverse command (`ccs kit drop …`) | — |

Tests that become the gate before any code (gate-first): `tests/tui.rs` drives `State` through the 30 keystrokes of §5 and asserts the final `Browser` calls and the summary-bar text. `tests/cli.rs` asserts that `ccs loadout sticksandstrings` prints the same totals. A property test checks that `loadout(p).totals` equals the size of the de-duplicated union of `kit_contents(k)` over `p`'s kits. Its oracle walks the library itself, and does not call `loadout`.

## 10. Weaknesses

- **Kits without a profile are second class.** Today 4 of the 8 kits (claude-flow, dev, forward-dev, network) are on no profile. On a profile-first board they live in a drawer at the bottom, and that drawer is the same on every profile. A user who wants to curate kits as reusable units has to go through the Library's target kit (`t`), or open a drawer kit read-only. A kit-first design (a list of kits on the left, kit contents and the library on the right) does this better.
- **One screen answers two questions.** The board is both "what will this profile load" and "edit this kit". When kits are shared, those questions pull apart: an edit made while looking at one profile changes another. The `⇄` mark and the confirm line reduce the surprise, but the model still invites it. A kit-first editor never has to ask "change both?".
- **Quick-add favours recall over recognition.** Typing "egm" is fast if you know the name. A user who does not know what is in the library has to switch to view 2. The 160-column side-by-side layout removes the switch, but 80-column users switch views often.
- **The board gets long.** A profile with `dev` and `claude-flow` attached would put 94 agents under folder lines. Folding keeps it to one line per folder, but the board is no longer a single glance. The summary bar has to carry that, and a bar is a weaker answer than a list.
- **The extra marks cost learning.** Six marks (`● ◉ ○ ◆ ◇ ✗`) is a lot. A user who misreads `◉` versus `●` will not see that a folder pick follows new items. Some terminal fonts draw `◉` and `●` alike, so the design needs an ASCII fallback (`[F]`, `[*]`, ` . `, `[P]`, ` - `, `[!]`), set with `CCS_ASCII=1`.
- **Strongest counter-argument.** In ccs the unit of reuse is the kit, not the profile. Profiles are logins: there are two today, and they change rarely. Kits change often. A design centred on the thing that rarely changes makes the frequent task (building and tuning kits) reachable only through it. Against that: the user's first complaint is "hard to see what skills, agents and plugins are in a kit", and they launch a profile, not a kit. "What will I get when I run this?" is the question every session starts with, and only a profile-first screen can answer it without extra steps.

# TUI design 2: the catalog matrix

Status: proposal. Replaces Home's kit list, the kit page and the library picker
of ADR-2610261643 §18–§21. Keeps the `Browser` port, `Exit`, the State/draw
split, the profile page and the import screens.

## 1. Summary

1. One screen lists every item in the library — skills, agents, plugins and what is inside each plugin — as rows grouped by kind and folder. Typing filters it.
2. Each kit is a column of marks. A mark tells you how the kit picks that row: `●` by its own line, `○` through a folder or plugin line, `◐` some of what is inside, `!` a line that finds nothing.
3. A cell is a checkbox. Space or a click adds or removes that one `uses` line, and the status line shows the matching `ccs kit use|drop` command.
4. A new kit is a new column (`n`). The current profile is a strip at the top. Its kits carry `◆` in the column headers, and a `loads` column shows what it will load. `l` opens that list in full.
5. All five jobs happen on the same grid. To see what a kit holds, read down its column. To see what the library holds, read the rows. To see what is still free, look for `·` in a column. Picking is a click on a visible cell, and a new kit is a new column.

## 2. Screen inventory

| # | Screen, pane or overlay | What it is for | How you get there | How you leave |
|---|---|---|---|---|
| 1 | **Catalog** (full screen, the start screen) | Rows are items, columns are kits. Everything below is part of it or opens over it. | `ccs` and `ccs ui` open it | `q` or Ctrl-C quits. `g` quits and starts Claude Code. |
| 1a | Profile strip (top line) | The current profile, its kits, how many items it loads, and `g start` | Always shown. `↑` from the column headers reaches it. | `↓` |
| 1b | Filter bar (second line) | Filter text, kind chips (all/skills/agents/plugins), show chips (all / in kit / not in kit) | `/`, or click it | `⏎` keeps the filter and returns to the grid. `Esc` clears it. |
| 1c | Column headers (3 lines) | Kit names, `◆` for the profile's kits, the number of items each kit gives, and `+ kit` | `↑` from the first row, or click | `↓` |
| 1d | Sources rail (left, ≥ 100 columns) | Folder tree with counts, hidden sources, profiles, legend of marks. Click a folder to filter to it. | Always shown when wide enough. Tab moves the focus to it. | Tab |
| 1e | Detail pane (bottom, or right at ≥ 150 columns) | The row under the cursor in words: path, description, every kit that picks it and how, which profiles load it, and the name Claude Code shows | Always shown | — |
| 1f | Footer | The keys that work on this cell, starting with what Space would do here | Always shown. `?` grows it to three lines. | `?` again |
| 2 | **Loads lens** | Everything one profile will load, under the names Claude Code will show, with the kit and line that gives each one, and the checks. Space turns a kit off for this launch only. | `l`, or click `→ loads N items` | `Esc` or a right click goes back to the catalog. `⏎` on a row goes to that row in the catalog. |
| 3 | New-kit prompt (fills the detail pane) | Name the kit. Start it empty or from a copy of another kit. | `n`, or click `+ kit` | `⏎` makes the kit. `Esc` cancels. |
| 4 | Kit menu (small overlay under a column header) | Every action on one kit with its key: rename `r`, delete `D`, CLAUDE.md `i`, the profiles that use it (checklist), start a new kit from a copy | `⏎` on a column header, or click a kit name | `Esc`, or an action |
| 5 | Profile picker (small overlay under the strip) | Choose the current profile. Its last line is `+ New profile`. | `p`, or click the profile name | `⏎` chooses. `Esc` cancels. |
| 6 | Profile page (unchanged from §18) | Variables, default, copy, shell, doctor | `⏎` on the strip, or click `◆ name` | `Esc` |
| 7 | Text view (unchanged) | An item's SKILL.md or agent file, or a plugin's manifest | `⏎` on an item row | `Esc`, `q`, or a right click |
| 8 | Prompts on the footer line | Confirm a delete or trash (`y`), confirm a folder split, add to library (a path with completion, or `owner/repo`), move to a folder | The key that needs it | `Esc` |

```
                         ┌───────────────────────┐
   profile picker ◄─ p ──┤                       ├── ⏎ on strip ─► profile page (§18)
   new-kit prompt ◄─ n ──┤   CATALOG (start)     ├── ⏎ on item ──► text view
   kit menu ◄─ ⏎ header ─┤                       ├── l ──────────► LOADS LENS
                         └───────────┬───────────┘
        every overlay, page and the lens: Esc or right click returns here
                                     │ g (from the catalog or the lens)
                                     ▼
                     Exit::Launch(p) / Exit::LaunchWith{p, kits}
```

## 3. Mockups

The real data comes from the user's machine. Marks:

| Mark | On an item row | On a folder or plugin row |
|---|---|---|
| `●` | the kit has a `uses` line for this item | the kit has a `uses` line for this folder or plugin |
| `○` | it comes in through a folder or plugin line above it | it comes in through a parent folder line |
| `◐` | — | the folder is not picked whole, but something inside it is |
| `◉` | it is picked by its own line **and** through a folder (the own line adds nothing) | same, for a subfolder |
| `!` | the kit has a line for this path, but nothing in the library matches it | same |
| `·` | not picked (shown dim) | nothing inside is picked |
| `◆` | in the `loads` column: the current profile loads it. In the count row: this kit belongs to the current profile. | same |

Only the cursor cell is drawn as `[·]`. In the real UI the active kit's column
header is drawn in reverse video, shown here as `‹test›`. The rail lists the
legend, and the detail pane always spells out the mark under the cursor in
words, so nobody has to learn the glyphs first.

### (a) Main screen, 120×36

Profile claude-bedrock-gary. The cursor is on `humanize-writing` in the column
of kit `test`. Folder `skills/network/` and its subfolders are open. Agents in
`claude-flow/` stay folded: 76 rows behind one line.

```
 ccs   profile ◆ claude-bedrock-gary ▾   kits base · writing · slides · test   → loads 33 items   g start   ? more keys
 / type to filter…            kind [all 139] skills 41 agents 96 plugins 2    show [all] · in test · not in test
┌ sources ───────┐┌ catalog · 139 items · 8 kits ──────────────────────────────────────────────────────────────────────┐
│▾ skills      41││  library                               loads       claude-flow forward-dev slides      writing     │
│  forward-dev  2││                                              base        dev         network    ‹test›       + kit │
│  horizon      2││  ◆ kit of profile · n = items          33    ◆0    76    34    4     37    ◆2    ◆29   ◆2          │
│  methods      0││──────────────────────────────────────┼──────────────────────────────────────────────────────────── │
│  network     35││  ▾ skills                          41  ◐     ·     ·     ·     ◐     ◐     ·     ◐     ◐          █│
│  writing      2││    ▸ forward-dev/                   2  ◆     ·     ·     ·     ●     ·     ·     ●     ·          █│
│▾ agents      96││    ▸ horizon/                       2  ◆     ·     ·     ·     ●     ·     ·     ●     ·          █│
│  claude-flow 76││      methods/                       0  ·     ·     ·     ·     ·     ·     ·     ·     ·          █│
│  dev         18││    ▾ network/                      35  ◐     ·     ·     ·     ·     ●     ·     ◐     ·          █│
│  network      2││      ▸ change/                      2  ·     ·     ·     ·     ·     ○     ·     ·     ·          █│
│▾ plugins      2││      ▸ inspect/                    10  ◆     ·     ·     ·     ·     ○     ·     ●     ·          █│
│  egm         16││      ▸ investigate/                 3  ·     ·     ·     ·     ·     ○     ·     ·     ·          █│
│  forward-decks…││      ▸ nqe/                         4  ·     ·     ·     ·     ·     ○     ·     ·     ·          █│
│                ││      ▸ plan/                       15  ◆     ·     ·     ·     ·     ○     ·     ●     ·          █│
│hidden          ││      ▸ review/                      1  ·     ·     ·     ·     ·     ○     ·     ·     ·          █│
│  none · z hides││    ▾ writing/                       2  ◆     ·     ·     ·     ·     ·     ·     ·     ●          █│
│                ││        forward-generate-report         ◆     ·     ·     ·     ·     ·     ·     ·     ○          █│
│profiles        ││▶       humanize-writing                ◆     ·     ·     ·     ·     ·     ·    [·]    ○          █│
│◆ claude-bedro… ││  ▾ agents                          96  ·     ·     ◐     ◐     ·     ◐     ·     ·     ·          █│
│  sticksandstr… ││    ▸ claude-flow/                  76  ·     ·     ●     ·     ·     ·     ·     ·     ·          █│
│                ││    ▸ dev/                          18  ·     ·     ·     ●     ·     ·     ·     ·     ·          █│
│◆ = loads it    ││    ▾ network/                       2  ·     ·     ·     ·     ·     ●     ·     ·     ·          █│
│● own line      ││        network-advisor                 ·     ·     ·     ·     ·     ○     ·     ·     ·          ░│
│○ via folder    ││        network-troubleshooter          ·     ·     ·     ·     ·     ○     ·     ·     ·          ░│
│  or plugin     ││  ▾ plugins                          2  ◐     ·     ·     ◐     ·     ·     ◐     ·     ·          ░│
│◐ some inside   ││    ▸ egm                           16  ·     ·     ·     ●     ·     ·     ·     ·     ·          ░│
│◉ both          ││    ▸ forward-decks                  2  ◆     ·     ·     ·     ·     ·     ●     ·     ·          ░│
│! missing       │├ skills/writing/humanize-writing · skill ───────────────────────────────────────────────────────────┤
│                ││ Aggressively rewrite AI-generated text into natural human prose in the user's specific voice. This…│
│                ││ in test: no (space adds it) · in writing: ○ through skills/writing/ · claude-bedrock-gary loads it…│
│                ││ 1 of 9 kits pick it · used by 1 of 2 profiles                                                      │
└────────────────┘└────────────────────────────────────────────────────────────────────────────────────────────────────┘
 space add to test  ←→ kit  ↑↓ item  ⏎ view  / filter  f show  n new kit  a attach  p profile  l loads  ? more
```

What it shows, reading only the screen:

- **What a kit holds:** read down a column. `test` picks `forward-dev/`, `horizon/`, `network/inspect/` and `network/plan/` by their own lines (`●`). `network/` shows `◐` in that column because only part of it is picked. The count row says `test` gives 29 items.
- **What the library holds:** the rows, with counts on every folder (41 skills, 96 agents, 2 plugins). `methods/` is empty and shows 0.
- **What is free:** the `·` cells. `f` filters to "not in test".
- **Folder picks look different from own picks:** `network/` agents show `●` in `network` on the folder row and `○` on each agent under it.
- **Profile:** `◆` above `base`, `slides`, `test`, `writing`. The `loads` column shows what claude-bedrock-gary will get.

### (b) The same at 80×24

No rail. Kit columns keep their 6-cell width, because 8 kits still fit next to an
18-column name area. Names longer than the area are cut in the middle, and the
full name stays on the detail line. Scrolled so the cursor row is in view.

```
 ccs ◆ claude-bedrock-gary ▾  base·writing·slides·test → 33             g start
 / filter…          [all] skills agents plugins   [all] in test · not in test
                         loads       claude-flow forward-dev slides      writing
                               base        dev         network    ‹test›
 ◆ = kit of profile      33    ◆0    76    34    4     37    ◆2    ◆29   ◆2
───────────────────────┼────────────────────────────────────────────────────────
      methods/        0  ·     ·     ·     ·     ·     ·     ·     ·     ·     │
    ▾ network/       35  ◐     ·     ·     ·     ·     ●     ·     ◐     ·     │
      ▸ change/       2  ·     ·     ·     ·     ·     ○     ·     ·     ·     │
      ▸ inspect/     10  ◆     ·     ·     ·     ·     ○     ·     ●     ·     █
      ▸ investigate/  3  ·     ·     ·     ·     ·     ○     ·     ·     ·     █
      ▸ nqe/          4  ·     ·     ·     ·     ·     ○     ·     ·     ·     █
      ▸ plan/        15  ◆     ·     ·     ·     ·     ○     ·     ●     ·     █
      ▸ review/       1  ·     ·     ·     ·     ·     ○     ·     ·     ·     █
    ▾ writing/        2  ◆     ·     ·     ·     ·     ·     ·     ·     ●     █
        forwar…eport     ◆     ·     ·     ·     ·     ·     ·     ·     ○     │
▶       humani…iting     ◆     ·     ·     ·     ·     ·     ·    [·]    ○     │
  ▾ agents           96  ·     ·     ◐     ◐     ·     ◐     ·     ·     ·     │
    ▸ claude-flow/   76  ·     ·     ●     ·     ·     ·     ·     ·     ·     │
    ▸ dev/           18  ·     ·     ·     ●     ·     ·     ·     ·     ·     │
── skills/writing/humanize-writing ────────────────────────────────────────────…
 test: not picked · writing ○ via skills/writing/ · loaded by this profile
 Aggressively rewrite AI-generated text into natural human prose in the…
 space add · ←→ kit · ⏎ view · / filter · n new kit · a attach · ? more
```

### (c) Making a new kit

`n` adds an empty column at the right edge and opens the name prompt in the
detail pane. The name is typed into the column header itself, so you see where the
kit will live before it exists. `Tab` in the prompt chooses an existing kit to
copy its lines from. `⏎` runs `new_kit` and leaves the cursor in the new column,
on the same row, ready for Space.

```
 ccs   profile ◆ claude-bedrock-gary ▾   kits base · writing · slides · test   → loads 33 items   g start   ? more keys
 / type to filter…            kind [all 139] skills 41 agents 96 plugins 2    show [all] · in research · not in it
┌ sources ───────┐┌ catalog · 139 items · 9 kits ──────────────────────────────────────────────────────────────────────┐
│▾ skills      41││  library                         loads       claude-flow forward-dev slides      writing     + kit │
│  forward-dev  2││                                        base        dev         network     test        research▏   │
│  horizon      2││  ◆ kit of profile · n = items    33    ◆0    76    34    4     37    ◆2    ◆29   ◆2    0           │
│  methods      0││────────────────────────────────┼────────────────────────────────────────────────────────────────── │
│  network     35││▶ ▾ skills                    41  ◐     ·     ·     ·     ◐     ◐     ·     ◐     ◐    [·]         █│
│  writing      2││    ▸ forward-dev/             2  ◆     ·     ·     ·     ●     ·     ·     ●     ·     ·          █│
│▾ agents      96││    ▸ horizon/                 2  ◆     ·     ·     ·     ●     ·     ·     ●     ·     ·          █│
│  claude-flow 76││      methods/                 0  ·     ·     ·     ·     ·     ·     ·     ·     ·     ·          █│
│  dev         18││    ▾ network/                35  ◐     ·     ·     ·     ·     ●     ·     ◐     ·     ·          █│
│  network      2││      ▸ change/                2  ·     ·     ·     ·     ·     ○     ·     ·     ·     ·          █│
│▾ plugins      2││      ▸ inspect/              10  ◆     ·     ·     ·     ·     ○     ·     ●     ·     ·          █│
│  egm         16││      ▸ investigate/           3  ·     ·     ·     ·     ·     ○     ·     ·     ·     ·          █│
│  forward-decks…││      ▸ nqe/                   4  ·     ·     ·     ·     ·     ○     ·     ·     ·     ·          ░│
│                ││      ▸ plan/                 15  ◆     ·     ·     ·     ·     ○     ·     ●     ·     ·          ░│
│                ││      ▸ review/                1  ·     ·     ·     ·     ·     ○     ·     ·     ·     ·          ░│
│profiles        ││    ▾ writing/                 2  ◆     ·     ·     ·     ·     ·     ·     ·     ●     ·          ░│
│◆ claude-bedro… │├ new kit ───────────────────────────────────────────────────────────────────────────────────────────┤
│  sticksandstr… ││ name  research▏                                                                                    │
│                ││ A kit starts empty. Then: space picks rows into it · a attaches it to a profile · i writes its CLA…│
│                ││ start from: [empty] base claude-flow dev forward-dev network slides test writing  (tab picks)      │
└────────────────┘└────────────────────────────────────────────────────────────────────────────────────────────────────┘
 ⏎ make kit research  (= ccs kit new research)   esc cancel   a-z 0-9 - _ only   ←→ edit the name
```

After `⏎`, the status line reads `✓ ccs kit new research`. The column header reads
`‹research›`, and its count is 0.

### (d) Opening a plugin to see inside

`⏎` on `egm` opens it like a folder. Its skills and agents are listed under it.
Every cell inside shows `○`, because a kit takes a plugin whole. The detail pane
says this, and it gives the name Claude Code will use (`/egm:egm_draft`). Space on
a row inside a plugin works on the plugin's own line, and the footer says so
before you press it. (This is after the walkthrough: the profile is
sticksandstrings, and kit `research` holds egm.)

```
 ccs   profile ◆ sticksandstrings ▾   kits research   → loads 18 items                            g start   ? more keys
 / type to filter…            kind all 139 skills 41 agents 96 [plugins 2]    show [all] · in research · not in it
┌ sources ───────┐┌ catalog · plugins · 9 kits ────────────────────────────────────────────────────────────────────────┐
│▾ skills      41││  library                         loads       claude-flow forward-dev slides      writing     + kit │
│▾ agents      96││                                        base        dev         network     test       ‹research›   │
│▾ plugins      2││  ◆ kit of profile · n = items    18    0     76    34    4     37    2     29    2     ◆18         │
│  egm  7s 9a    ││────────────────────────────────┼────────────────────────────────────────────────────────────────── │
│  forward-decks…││  ▾ plugins                    2  ◐     ·     ·     ◐     ·     ·     ◐     ·     ·     ◐          █│
│                ││    ▾ egm                     16  ◆     ·     ·     ●     ·     ·     ·     ·     ·     ●          █│
│profiles        ││        skills inside egm      7                                                                   █│
│  claude-bedro… ││          egm_bootstrap           ◆     ·     ·     ○     ·     ·     ·     ·     ·     ○          █│
│◆ sticksandstr… ││          egm_brainstorm          ◆     ·     ·     ○     ·     ·     ·     ·     ·     ○          █│
│                ││▶         egm_draft               ◆     ·     ·     ○     ·     ·     ·     ·     ·    [○]         █│
│                ││          egm_drift_report        ◆     ·     ·     ○     ·     ·     ·     ·     ·     ○          █│
│                ││          egm_resolve             ◆     ·     ·     ○     ·     ·     ·     ·     ·     ○          █│
│                ││          egm_review              ◆     ·     ·     ○     ·     ·     ·     ·     ·     ○          █│
│                ││          egm_review_loop         ◆     ·     ·     ○     ·     ·     ·     ·     ·     ○          █│
│                ││        agents inside egm      9                                                                   █│
│                ││          bootstrap…hestrator     ◆     ·     ·     ○     ·     ·     ·     ·     ·     ○          █│
│                ││          codebase_details        ◆     ·     ·     ○     ·     ·     ·     ·     ·     ○          ░│
│                ││          comprehension           ◆     ·     ·     ○     ·     ·     ·     ·     ·     ○          ░│
│                ││          design_critic           ◆     ·     ·     ○     ·     ·     ·     ·     ·     ○          ░│
│                ││          implement…readiness     ◆     ·     ·     ○     ·     ·     ·     ·     ·     ○          ░│
│                ││        ▸ 4 more                                                                                   ░│
│                ││    ▸ forward-decks            2  ·     ·     ·     ·     ·     ·     ●     ·     ·     ·          ░│
│                │├ plugins/egm › skills/egm_draft · skill inside a plugin ────────────────────────────────────────────┤
│                ││ Claude Code calls it /egm:egm_draft · plugin egm: Draft specs, review designs, and check implement…│
│                ││ in research ○ and dev ○, through plugins/egm · a plugin is picked whole: space here drops all of e…│
│                ││ from git google/elephant-goldfish-model · U pulls it · sticksandstrings loads it (via research)    │
└────────────────┘└────────────────────────────────────────────────────────────────────────────────────────────────────┘
 space drop egm from research (whole plugin)   ⏎ view SKILL.md   ← back to egm   e edit   U update plugin   / filter
```

Note on the count: the brief says egm has 8 agents. `plugins/egm/agents/` on this
machine holds 9 files (bootstrap_orchestrator … zoom_out, synthesizer included).
The mockups use what is on disk. The count comes from the library, not from this
document.

### (e) What this profile will load (loads lens), 80×24

`l` from the catalog. The lens lists everything the profile will load, grouped by
kind, under the names Claude Code will show. A kit's skills get the kit's prefix
(`research:`), and a plugin keeps its own (`egm:`). Each row says which kit gives
it and whether by its own line or through a plugin or folder. The checks at the
bottom run the same code as the launch, so the lens cannot disagree with what
`g` starts.

```
 ccs › loads ◆ sticksandstrings                           g start  esc catalog
 kits [x] research    gives 18: 9 skills · 9 agents · CLAUDE.md none
  name in Claude Code                    kit        how
────────────────────────────────────────────────────────────────────────────────
 ▾ skills                               9
     research:forward-generate-report      research   ● own line
▶    research:humanize-writing             research   ● own line
     egm:egm_bootstrap                     research   ○ plugins/egm
     egm:egm_brainstorm                    research   ○ plugins/egm
     egm:egm_draft                         research   ○ plugins/egm
     egm:egm_drift_report                  research   ○ plugins/egm
     egm:egm_resolve                       research   ○ plugins/egm
     egm:egm_review                        research   ○ plugins/egm
     egm:egm_review_loop                   research   ○ plugins/egm
 ▾ agents                               9
     egm:egm-bootstrap-orchestrator        research   ○ plugins/egm
     egm:egm-codebase-details              research   ○ plugins/egm
     ▸ 7 more from plugins/egm
 ▾ instructions (CLAUDE.md)             0
     none (i on a kit writes one)
────────────────────────────────────────────────────────────────────────────────
 ✓ every uses line found  ✓ no prefix claimed twice  ✓ nothing loaded twice
 = ccs sticksandstrings  (claude --plugin-dir …/research --plugin-dir …/egm)
 g start  space kit on/off for this launch only  ⏎ find in catalog  esc back
```

## 4. Key map and mouse map

Each key means one thing on every row, and the footer shows the keys that do
something on the current cell. Letters do not filter, because `/` does. So
single-letter keys are safe at all times except while typing in a prompt.

| Key | Mouse | Does | Same as (CLI) |
|---|---|---|---|
| `↑` `↓` / `j` `k` | click a row; wheel scrolls | move between rows; `↑` from the first row reaches the column headers, then the profile strip | — |
| `←` `→` | click a cell or a kit name | change the active kit column (no `h`/`l` aliases: `l` is the loads lens) | — |
| PgUp PgDn Home End | scrollbar drag | page / first / last row | — |
| **Space** | **click a cell** | add or remove this row's line in the active kit (see the Space rules below) | `ccs kit use\|drop <kit> <entry>` |
| Shift-`↑`/`↓` | Shift-click | select several rows. Space then acts on all of them in the active kit. | `ccs kit use <kit> <e1> <e2>…` |
| `u` | — | undo the last pick or drop (sends the opposite `set_use`) | `ccs kit drop\|use …` |
| `t` | — | on a folder row: remove the own lines that the folder line already covers (`◉` → `○`) | `ccs kit drop <kit> <entry>…` |
| `⏎` | click the selected row again; click `▸`/`▾` | folder or plugin: open or close. Item: view its file. Column header: kit menu. Strip: profile page. | `ccs library show <entry>` |
| `/` | click the filter bar | type to filter. `⏎` returns to the grid with the filter kept. `Esc` clears it. | `ccs kit matrix --filter <text>` (new) |
| `0` `1` `2` `3` | click a kind chip | all / skills / agents / plugins | `ccs library list skills` … |
| `f` | click a show chip | show all → only in the active kit → only not in it | `ccs kit show <kit>` |
| `c` | — | kit columns: all kits ↔ only this profile's kits | — |
| `G` | — | rows grouped by folder ↔ one flat list sorted by name, with the folder as a column | — |
| `z` | click `×` beside a folder in the rail | hide this folder or plugin from the catalog; `z` on the rail's hidden list shows it again | `ccs library hide\|unhide <entry>` (new) |
| `n` | click `+ kit` | new kit (a new column) | `ccs kit new <k> [--from <k2>]` |
| `r` | kit menu | rename the active kit | `ccs kit rename <k> <new>` |
| `D` | kit menu | delete the active kit (asks `y`) | `ccs kit rm <k>` |
| `i` | kit menu | edit the active kit's CLAUDE.md in `$EDITOR` | `ccs kit edit <k>` |
| `a` | click the `◆`/count cell of a column | attach or detach the active kit to the current profile | `ccs kit attach\|detach <k> <p>` |
| `p` | click the profile name | choose the current profile | `ccs default` is **not** changed; this is a view choice |
| `l` | click `→ loads N items` | loads lens for the current profile | `ccs loads <p>` (new) |
| `g` | click `g start` | start Claude Code with the current profile and its kits | `ccs <p>` |
| `e` | — | edit the item's file in `$EDITOR` | `ccs library edit <entry>` |
| `+` | click `+ add` in the rail | add to the library: a path (completed) or `owner/repo` | `ccs library add …` |
| `m` | — | move the item or folder to another folder (typing a new name makes it) | `ccs library mv …` / `mkdir` |
| `x` | — | move the item to the trash (asks `y`) | `ccs library rm <entry>` |
| `U` | — | `git pull` this plugin (on a plugin row) or every plugin | `ccs library update [plugins/<n>]` |
| Tab | — | focus: grid → rail → filter | — |
| `?` | click `? more` | grow the footer to three lines: every key for this kind of row | — |
| `Esc` | right click | close the overlay, else clear the filter, else leave the lens. **Never quits.** | — |
| `q`, Ctrl-C | — | quit | — |

**Space rules**, so that nothing is a guess. The footer shows the outcome before you press:

| Cell under the cursor | Space does | Footer says |
|---|---|---|
| `·` on an item, folder or plugin | adds that line | `space add to test` |
| `●` | drops that line | `space drop from test` |
| `◉` | drops the own line. The item stays, through the folder (`○`). | `space drop own line (stays via skills/network/)` |
| `○` under a folder | asks: split the folder line into item lines, minus this one (`y`), or leave it | `space: comes through skills/network/inspect/ · split?` |
| `○` inside a plugin | drops the whole plugin, and the footer says so first | `space drop egm from research (whole plugin)` |
| `◐` on a folder | adds the folder line. The items inside that had their own lines become `◉`, and the footer offers `t` to remove those lines. | `space add whole folder (3 own lines become extra)` |
| `!` | drops the dead line | `space drop missing line` |

**How help is shown without a help screen:**

1. The footer always starts with what Space does **on this cell, in this kit**. For example: `space add to test`.
2. The detail pane states the mark in words: `in writing: ○ through skills/writing/`.
3. After each change the status line shows the CLI command it ran: `✓ ccs kit use research plugins/egm`. You learn the CLI and get a record of every change.
4. The places you can click carry labels on screen: `+ kit`, `g start`, `→ loads 33 items`, the kind and show chips.
5. `?` grows the footer in place to three lines. It does not open a separate screen.
6. Empty states say what to press. For an empty library, the rows say `+ add a skill (path)` and `+ add a plugin (owner/repo, e.g. google/elephant-goldfish-model)`.

## 5. Walkthrough of the success test

Start: `ccs` in a terminal 80 columns or wider. The catalog opens with
claude-bedrock-gary, the cursor on the first row and kit `test` active.

| # | Keys | Count | Screen afterwards |
|---|---|---|---|
| 1 | `n` | 1 | An empty column appears at the right. The detail pane reads `new kit · name ▏`. |
| 2 | `r e s e a r c h` | 8 | The header shows `research▏`. |
| 3 | `⏎` | 1 | `✓ ccs kit new research`. The column is now `‹research›` with count 0, and the cursor is in it. |
| 4 | `/` | 1 | The cursor moves to the filter bar. |
| 5 | `e g m` | 3 | Rows: `▾ plugins › ▸ egm 16`. The cursor sits on the best match, and a plugin or folder wins over the rows inside it. |
| 6 | `⏎` | 1 | Back to the grid, filter kept. The footer reads `space add to research`. |
| 7 | Space | 1 | `✓ ccs kit use research plugins/egm`. The cell shows `●`, and the count reads 16. |
| 8 | `/` | 1 | The old filter text is selected, so typing replaces it. |
| 9 | `w r i t i n g /` | 8 | Rows: `▾ writing/ 2`, `forward-generate-report`, `humanize-writing`. The cursor is on the folder. |
| 10 | `⏎` | 1 | Back to the grid. |
| 11 | `↓` Space | 2 | `✓ ccs kit use research skills/writing/forward-generate-report` (`●`) |
| 12 | `↓` Space | 2 | `✓ ccs kit use research skills/writing/humanize-writing` (`●`). Count 18. |
| 13 | `p` | 1 | The profile picker shows claude-bedrock-gary and sticksandstrings. |
| 14 | `↓` `⏎` | 2 | The strip reads `◆ sticksandstrings · kits (none) → loads 0 items`. The `◆` marks above the kits clear, and the `loads` column empties. |
| 15 | `a` | 1 | `✓ ccs kit attach research sticksandstrings`. `◆18` appears over `research`, and the strip reads `kits research → loads 18 items`. |
| 16 | `l` | 1 | The loads lens (mockup e) shows 9 skills, 9 agents, no CLAUDE.md, and the three checks passed. |

**Total: 35 keystrokes**, of which 19 are the letters of `research`, `egm` and
`writing/`. Everything else is navigation or a single key named on screen.
Every key the walkthrough needs is visible when it is needed. `+ kit` and `n
new kit` show in the header and footer. `/ filter` is on the second line. The
footer says `space add to research`. `p profile`, `a attach` and `l loads` are
in the footer. No help screen is opened. At an ordinary typing pace, with a
moment to read each screen, the run takes 25–40 seconds.

The same run with a mouse: click `+ kit`, type `research`, `⏎`. Click the filter
bar, type `egm`, then click egm's `research` cell. Type `/writing/` and click two cells. Click the
profile name, then `sticksandstrings`. Click the `research` count cell to attach.
Click `→ loads 18 items`.

## 6. How each of the five jobs is solved

**(1) What a kit holds.** Read down its column. Own lines are `●` and inherited rows are `○`.
A folder that is partly picked shows `◐`, so you can see a picked row under a
closed folder without opening it. The count row gives the total, and `f` cuts the rows to
"in this kit". This is what `ccs kit show` prints, with the `uses`/`gives` split
drawn as `●`/`○`. The loads lens answers the related question for a profile.

**(2) What the library holds, and what is inside it.** The rows are the library:
every kind, every folder (empty ones included) with a count, and plugins open
like folders to list their skills and agents. The detail pane gives the
description, the source (a folder, or a git source for plugins) and the name
Claude Code will use. The rail gives the same tree as a short index with counts.

**(3) What is available but not picked.** A `·` in the active column is free. `f`
switches to "not in this kit", which lists only those rows. The `loads` column
does the same for the profile: rows without `◆` are in the library but not
loaded.

**(4) Pick and unpick with no guessing.** One cell is one `uses` line in one kit,
and the active kit is named in the footer before every Space. The mark tells you
in advance what Space will do (table in §4). The only case where Space cannot
act directly is an item that comes in through a folder, and there it asks rather
than guessing. Each change shows its CLI command, and `u` undoes it.

**(5) Make a new kit.** `n`, a name, `⏎`. The kit shows up as a column you then fill
with Space, and the cursor is already in it. Starting from a copy is one Tab in
the same prompt. Attaching it to a profile is `a` (or the kit menu's profile
checklist, to attach it to a profile other than the current one).

## 7. Scale and noise

**96 agents, mostly near duplicates.**

- Folders with more than 12 rows start closed. `claude-flow/` is one line with
  76 and a `●` in the `claude-flow` column until you open it. The catalog
  remembers which folders are open between runs, as a view setting in
  `$CCS_HOME`.
- The filter matches the name, the folder path and the description. A name match
  ranks above a path match, which ranks above a description match. Typing
  `coordinator` gives 11 rows. Each row shows its folder dimmed to the right of
  the name (`consensus/`, `swarm/`, `v3/`…), so `gossip-coordinator` and
  `mesh-coordinator` can be told apart without opening anything. The detail pane
  gives the first line of the description.
- `G` swaps folder grouping for one flat list sorted by name, with the folder as
  a column. Near duplicates then sit next to each other, which helps when you
  compare them.
- **Hiding a source.** `z` on `agents/claude-flow/` removes its 76 rows from the
  catalog. The rail lists it under *hidden* with its count, and clicking it or
  pressing `z` there brings it back. Hiding is a view setting and never changes a kit.
  When "show: in <kit>" is on, a kit's hidden rows still appear, so a kit's
  contents are never hidden from you. A hidden source still loads if a kit picks
  it, and the loads lens lists it.
- **Kind chips** (`1` `2` `3`) cut the grid to one kind, and the rail does
  the same for one folder.

**8 or more kits at 80 columns.** Columns take the space they need, in this order:

1. While the name area can keep at least 18 columns, every kit column stays
   6 wide with full names staggered over two header lines. Each name gets up to
   11 characters, enough for `claude-flow` and `forward-dev`. That is 8 kits at
   80 columns (mockup b).
2. Beyond that, cells shrink to 3 columns, and each kit gets a two-letter tag
   (the first letters of its hyphen parts, made unique). The legend replaces
   the description line in the detail pane, and the active kit's full name
   is always in the header. That fits 16 kits at 80 columns:

```
 ◆ kit of claude-bedrock-gary   column: ‹test› (ts) 29 items
                                        lo ba cf dv fd nw sl ts wr rs op rv ui
                                           ◆              ◆  ◆  ◆
────────────────────────────────────────────────────────────────────────────────
   ▾ writing/                          2  ◆  ·  ·  ·  ·  ·  ·  ·  ●  ●  ·  ·  ·
       forward-generate-report            ◆  ·  ·  ·  ·  ·  ·  ·  ○  ○  ·  ·  ·
▶      humanize-writing                   ◆  ·  ·  ·  ·  ·  · [·] ○  ○  ·  ●  ·
 lo loads  ba base  cf claude-flow  dv dev  fd forward-dev  nw network
 sl slides  ts test  wr writing  rs research  op ops-oncall  rv review  ui ui-d…
```

3. Beyond 16, the grid scrolls sideways. The `loads` column and the active
   column stay pinned. `‹ 4 more` / `3 more ›` arrows in the header can be
   clicked. The profile's kits are always ordered first.
4. `c` shows only the current profile's kits. That is usually 1–4 columns,
   which is the common case when you are building a profile.

At **160×50** the extra space goes to reading, not to more chrome. The rail
widens to 24 columns. The name area grows to 40 and gains a 30-column
description column. The detail pane moves to the right (36 columns) and shows
the full description, every `uses` line that gives the row, and the profiles that
load it. About 40 rows are visible, enough for the whole skills tree with
`network/` open.

## 8. Edge cases

- **An item picked both directly and through a folder.** The cell shows `◉`. The
  detail pane names both lines: "`skills/network/inspect/inspect-bgp-neighbors`
  and `skills/network/inspect/` both give this. The own line adds nothing." Space
  drops the own line, and the cell becomes `○`. The loads lens lists the item once,
  and its check `nothing loaded twice` stays green. If two *kits* of one profile
  give the same item, the lens lists it twice under both prefixes
  (`test:inspect-edge`, `network:inspect-edge`) and the check turns to
  `⚠ inspect-edge comes from 2 kits`.
- **A `uses` line that finds nothing.** The line becomes a row in a
  *missing* group at the top of its kind, `! skills/writing/old-draft`, with `!`
  in the kit's column and a red `!1` next to the kit's count. Space on `!` drops the
  line. The loads lens shows `✗ 1 uses line finds nothing (kit research)`. It
  does not block `g`, matching `ccs doctor`, which warns and does not refuse.
  A missing row is never hidden by a filter or by `z`.
- **A plugin and a kit with the same name** (for example a kit `egm` and the
  plugin `egm`). They cannot collide on the grid, because one is a column and the
  other a row. But both claim the prefix `egm:` in Claude Code. The new-kit
  prompt warns while you type: `a plugin is called egm; both would show as
  /egm:…`. The column header carries `⚠`. If one launch loads both, the lens shows
  `✗ prefix egm: claimed by kit egm and plugin egm`, the warning the launch
  prints today.
- **Empty library on a new host.** The three kind rows show `0`, followed by
  rows you can act on: `+ add a skill or agent (path)`, `+ add a plugin
  (owner/repo, e.g. google/elephant-goldfish-model)` and `⇣ import ~/.claude`.
  The kit columns still appear (a new host has `base`). `⏎` on an action row
  runs it. This keeps §21's lesson: the empty state names the key on this screen.
- **Very long names.** Item names are cut in the middle (`forwar…eport`), so the
  start and the end both stay readable. The full path is always on the detail
  pane's title line. Kit names longer than 11 characters are cut with `…` in the
  header and shown in full while their column is active.
- **Narrow terminal.** Under 100 columns the rail goes. Under 80 the cells use
  tags (rule 2 above). Under 70 (today's `NARROW`) only the `loads` column and the
  active column are drawn, with a `‹ test 7/9 ›` header that `←→` pages
  through. Under 40×12 the screen says `make the terminal at least 40×12` and
  keys still work (`q` quits).
- **Terminals that draw `●○◐◉◆` double-width or not at all.**
  `CCS_ASCII=1` (also set when `TERM=linux`) uses `*` own, `+` through, `~`
  some, `#` both, `!` missing, `.` none, `@` profile.
- **A folder line plus a subfolder line in one kit.** The subfolder row shows `◉`. Space on
  it drops the subfolder line.
- **A plugin with no skills or agents inside** (a hooks-only plugin). It opens to a single
  dim row, `no skills or agents (hooks, commands only)`, and its count is 0.

## 9. Required changes behind the screen

Per CLAUDE.md, an ADR must come before any code here, because it adds `Browser`
methods. The gate must be written first. A suggested gate is
`cargo test --test tui && cargo test --test boundaries && hexa analyze . --grade A+ --exit-code`,
with one new `tests/tui.rs` test that types the 35 keys of §5 into `State`. It
then asserts through a fake `Browser` that kit `research` has exactly
`plugins/egm`, `skills/writing/forward-generate-report` and
`skills/writing/humanize-writing`, that sticksandstrings has `[research]`, and that
the lens shows 18 rows. A count of 0 rows must fail the test.

**New `Browser` methods.** The domain types are re-exported through `ports`, per rule 4.

| Method | Returns | Why the TUI cannot do it alone |
|---|---|---|
| `catalog(&self) -> Catalog` | every skill, agent and plugin, all folders (empty ones too), each plugin's skills and agents (`inside: Some("egm")`), descriptions, and the name Claude Code shows (`egm:egm_draft`) | Today `library(kind)` returns no plugin contents and no Claude names. Plugin contents need the manifest reader in the secondary adapter. |
| `matrix(&self) -> Vec<KitColumn>` | per kit: its `uses` lines, what each line gives (`Given { item, via: Vec<Use> }`), its missing lines, the profiles that use it | Folder resolution must stay in the domain. If the TUI re-implemented "what a folder line gives", the grid could disagree with `ccs kit show` and with the launch. |
| `loads(&self, p: &ProfileName, kits: Option<&[String]>) -> Result<LoadPlan, CcsError>` | rows (Claude name, kind, kit, line), instructions, checks (missing, prefix clash, loaded twice), the `--plugin-dir` list | This must be **the same function the launch uses**. A second copy for display is a second source of truth (CLAUDE.md lesson). |
| `hidden(&self) -> Vec<Use>`, `set_hidden(&self, e: &Use, on: bool)` | the hidden sources | A view setting stored in `$CCS_HOME/catalog.toml`. Never in `~/.claude`. |
| `view_state` / `set_view_state` (open folders, grouping, column filter) | — | So the catalog reopens as you left it. This is optional, and the TUI can live without it. |
| `copy_kit(&self, from: &str, to: &str)` | — | "start from" in the new-kit prompt. The same can be done with `new_kit` plus one `set_use` per line, but that is not atomic. |
| `split_folder(&self, kit, folder: &Use, except: &[Use])` | — | Replaces a folder line with item lines minus some, in one write. A half-done split would leave a kit holding fewer items than it showed. |

Reused as they are: `profiles`, `kits`, `attach`, `new_kit`, `remove_kit`,
`rename_kit`, `kit_uses`, `set_use`, `kit_instructions`, `edit_kit`, `read`,
`edit`, `add`, `update_plugins`, `mkdir`, `move_entry`, `remove`, `set_kits`,
`complete_path`, `new_profile`, `edit_profile`, the import and migrate methods.
Rendering needs no new dependency, and the filter's matcher is a small scorer
written in the adapter. Adding a fuzzy-match crate would need its own ADR,
since ADR-2610261255 decision 6 keeps ratatui as the only dependency.

**CLI parity.** Every action in the TUI must also exist as a CLI command.

| TUI action | CLI | Exists? |
|---|---|---|
| Space on a cell | `ccs kit use\|drop <k> <entry>…` | yes |
| `n` new kit | `ccs kit new <k>` | yes |
| new kit from a copy | `ccs kit new <k> --from <k2>` | **new** |
| `r`, `D`, `i` | `ccs kit rename`, `ccs kit rm`, `ccs kit edit` | yes |
| `a` attach | `ccs kit attach\|detach <k> <p>` | yes |
| the grid itself, filter, `f` | `ccs kit matrix [skills\|agents\|plugins] [--filter <text>] [--kit <k> [--not]]`: the same rows and marks as text | **new**. The TUI and the CLI must use one function, so their output can be compared in a test. |
| open a plugin | `ccs library list plugins/egm`: its skills and agents (today `show` prints the raw manifest) | **new** |
| loads lens | `ccs loads <p> [+kit…]` | **new** |
| `z` hide | `ccs library hide\|unhide <entry>`. `ccs library list` follows it, and `--all` shows hidden rows. | **new** |
| folder split | `ccs kit split <k> <folder> --except <entry>…` (today: `drop` then `use`) | **new** (optional) |
| view, edit, add, move, trash, update | `ccs library show\|edit\|add\|mv\|rm\|update` | yes |
| `g` start, start with kits for this launch only | `ccs <p>`, `ccs <p> +kit…` | yes |

Docs to update in the same change: `src/adapters/primary/README.md` (the
Interface and Invariants sections), ADR-2610261643 (§18–§21 marked replaced), and
`ccs --help`.

## 10. Weaknesses

- **The grid is mostly empty.** With 9 kits and 139 rows, most of the
  cells are `·`. The marks that matter are spread thin, and a wide terminal makes
  that worse, not better. Folded folders, `f` and `c` reduce it but do not remove it.
- **A two-dimensional cursor allows picks into the wrong kit.** Space works on the
  active column, and the cursor can sit on a row far from that column's header. The
  footer naming the kit, the reverse-video header, the CLI echo and `u` all
  reduce the risk. A design that edits one kit at a time cannot have the error at all.
- **Columns cost width that names need.** At 80 columns the name area is 18
  characters, and long skill names are cut (`humani…iting`). Past 8 kits the
  headers become two-letter tags that must be looked up. A list-and-detail design
  gives every name the full width.
- **Plugins look pickable inside but are not.** Rows inside a plugin show cells,
  and Space on them acts on the whole plugin. The footer says so first, but it still
  surprises the first time.
- **Profiles are second-class.** The grid is about kits × items. A profile is a
  strip, a `◆` row, a column and a lens. Attaching a kit to a profile other than
  the current one goes through the kit menu's checklist, one level deeper than
  attaching to the current profile.
- **Folder lines mean "now and later", but the grid shows "now".** `○` on today's
  items can suggest a choice made item by item. The detail pane says "through
  skills/network/inspect/: also takes anything added there later", but the grid
  itself cannot show that.

**The strongest counter-argument.** None of the five jobs asks to compare kits,
and comparing kits is the one thing a matrix is good at. The jobs are all one kit
at a time (what is in it, pick into it, make it) or one profile at a time (what
will it load). A kit-first design does every one of those with a single column
of checkboxes next to the library tree, at full name width, with no
two-dimensional cursor. That design is: a list of kits on the left, the chosen
kit's library tree with `[x]` on the right, and the same loads lens. This matrix
only beats it when you ask "which other kit already has this?" or when you
spread one item across several kits. That happens, for example, when the 29
items of `test` overlap with `forward-dev` and `network`. The answer to the
argument is that `f` ("in this kit") with `c` (this profile's kits) turns the
matrix into that kit-first view on demand, and the matrix keeps the overlaps
visible when you need them. If user testing shows that people live in `f` mode,
the kit-first design should replace this one.

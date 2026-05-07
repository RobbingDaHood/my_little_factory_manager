# Zellij + nushell UI

A terminal UI for *My Little Factory Manager*. The game server is unchanged —
this directory is a thin client that hits the existing HTTP API and lays the
results out in a Zellij session driven by nushell scripts.

![Screenshot of the Zellij + nushell UI showing tokens, hand, library, possible actions, an active contract, and the bottom command pane after `mlfm> play 2`](docs/screenshot.png)

## Layout

```
┌───────────┬──────────────┬─────────────┐
│ tokens    │ hand         │             │
├───────────┼──────────────┤   library   │
│ actions   │ contract     │             │
├───────────┴──────────────┴─────────────┤
│ play  (thin one-line command pane)     │
└────────────────────────────────────────┘
```

| Pane     | Source              | Refreshes                                  |
| -------- | ------------------- | ------------------------------------------ |
| tokens   | `panes/tokens.nu`   | `GET /player/tokens`                       |
| hand     | `panes/hand.nu`     | `GET /state` (filtered to `counts.hand>0`) |
| actions  | `panes/actions.nu`  | `GET /actions/possible`                    |
| contract | `panes/contract.nu` | `GET /state` — active contract or offers   |
| library  | `panes/library.nu`  | `GET /library/cards`                       |
| play     | `bin/play.nu`       | interactive nu with command module loaded  |

The contract pane auto-switches: when a contract is active it shows the
requirements + reward + adaptive adjustments; otherwise it lists offered
contracts grouped by tier.

## Prerequisites

- [zellij](https://zellij.dev/) (terminal multiplexer)
- [nushell](https://www.nushell.sh/) ≥ 0.90 (`nu` on PATH)
- Game server running:
  ```sh
  cargo run     # from the repo root, listens on :8000
  ```

## Run

```sh
cd UIs/Zellij+nushell
./start.sh
```

To point the UI at a different server:

```sh
MLFM_BASE_URL=http://localhost:9000 ./start.sh
```

Quit zellij with `Ctrl-q` (zellij default).

## Commands available in the play pane

Type `help-mlfm` once the bottom pane comes up. Summary:

**Player actions** (drive `POST /action`):

| Command                                  | API equivalent                                 |
| ---------------------------------------- | ---------------------------------------------- |
| `new-game [seed]`                        | `NewGame { seed }`                             |
| `accept <tier> <idx>`                    | `AcceptContract { tier_index, contract_index}` |
| `play <card_index>`                      | `PlayCard { card_index }`                      |
| `discard <card_index>`                   | `DiscardCard { card_index }`                   |
| `replace <target> <repl> <sacrifice>`    | `ReplaceCard { … }`                            |
| `abandon`                                | `AbandonContract`                              |

`<card_index>` is the index into the global `cards` Vec returned by
`GET /state`. Both the hand pane and the actions pane print the valid indices
explicitly so you can copy them.

**One-shot inspection** (full JSON, in case the dashboards aren't enough):

```
show state | tokens | hand | contract | contracts |
     library [tag-json] | actions | metrics | history
```

**Pane controls**:

| Command                       | Effect                                                 |
| ----------------------------- | ------------------------------------------------------ |
| `filter-library <text…>`      | only show library rows whose rendered line contains it |
| `filter-contracts <text…>`    | same, for the offered-contracts pane                   |
| `clear-filters`               | reset both filters                                     |
| `refresh <ms>`                | change the auto-refresh interval                       |

Filters are stored in `state/ui.json`; the dashboard panes pick up changes on
their next tick.

## Why a subfolder

This is one of potentially several future UIs (web, TUI variants, etc.).
Keeping each under `UIs/<name>/` makes them independent and lets them choose
their own dependencies. The Rust server is the source of truth — every UI
just speaks HTTP to it.

## Files

```
UIs/Zellij+nushell/
├── README.md
├── layout.kdl              zellij layout
├── start.sh                launcher
├── config.nu, env.nu       nu config used only by the play pane
├── lib/
│   ├── api.nu              HTTP API wrapper
│   ├── fmt.nu              card/token/requirement pretty-printers
│   └── state.nu            shared UI state (filter file)
├── panes/                  refresh loops, one per dashboard
│   ├── tokens.nu
│   ├── hand.nu
│   ├── actions.nu
│   ├── contract.nu
│   └── library.nu
├── bin/play.nu             player command module (loaded by config.nu)
└── state/ui.json           pane filters (gitignored)
```

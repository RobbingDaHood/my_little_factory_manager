# My Little Factory Manager — Roadmap

This roadmap converts the [vision](vision.md) into incremental, playable milestones. Each phase produces a minimal playable loop or a meaningful extension of the previous one.

## Technology Stack

* **Language**: Rust (nightly toolchain)
* **Web framework**: Rocket 0.5.x with JSON support
* **API documentation**: OpenAPI/Swagger via `rocket_okapi`
* **Deterministic RNG**: `rand` + `rand_pcg` (seeded random number generator)
* **Serialization**: `serde` + `serde_json` with JSON Schema support (`schemars`)
* **Testing**: Integration tests in `tests/` directory; `cargo-llvm-cov` for coverage (≥80%)
* **CI**: GitHub Actions (fmt, clippy, build, test, coverage)
* **Pre-commit**: `cargo fmt` (auto-fix) + `cargo clippy`

## Reference Project

The existing [my_little_card_game](https://github.com/RobbingDaHood/my_little_cardgame) repository serves as the primary architectural reference. Specific files to study for each phase are noted below.

### Key Lessons from the Card Game

1. **Formula-based balance from the start**: The card game suffered from exponential balancing complexity because 100+ numeric config values were tuned by trial and error. This project will derive card values from ~5-10 design-intent parameters using explicit mathematical formulas. See the [balance research notes](https://github.com/RobbingDaHood/my_little_cardgame) for the full analysis.

2. **Single mechanical system**: The card game had 5+ independent mechanical systems (combat, mining, herbalism, woodcutting, fishing), each requiring separate balance work. This project uses one unified production mechanic — contracts vary by their constraint combinations, not by different resolution formulas. This dramatically reduces the balance surface.

3. **Config-driven design**: All game configuration externalized to JSON files, embedded at compile time. Card IDs are positional — new cards always appended for save compatibility.

---

## Phase 0: Project Scaffolding

**Goal**: A Rust project that compiles, passes CI, and has the development workflow ready.

**Deliverables**:
- `Cargo.toml` with Rocket, rocket_okapi, serde, rand, rand_pcg dependencies
- `src/main.rs` — binary entry point (Rocket launch)
- `src/lib.rs` — library entry point (route mounting, `rocket_initialize()`)
- `Makefile` with `check`, `coverage`, `install-hooks` targets
- `scripts/check_all.sh` — unified validation (fmt, clippy, build, test, coverage)
- `scripts/check_clippy.sh` — clippy runner
- `scripts/install-hooks.sh` — pre-commit hook installer
- `scripts/worktree-manage.sh` — worktree management for parallel development
- `.pre-commit-config.yaml` — cargo fmt + clippy hooks
- `toolchain.toml` — nightly Rust toolchain
- `.github/workflows/ci.yml` — GitHub Actions CI pipeline
- `tests/` directory with at least one smoke test
- `GET /version` endpoint returning `{"version": "<semver>-<hash>"}` to verify the server runs

**Reference files from card game**:
- `Cargo.toml` — dependency list and feature flags
- `src/main.rs`, `src/lib.rs` — entry point pattern
- `Makefile`, `scripts/*` — build tooling
- `.pre-commit-config.yaml`, `toolchain.toml` — Rust toolchain config
- `.github/workflows/ci.yml` — CI pipeline

---

## Phase 1: Core Types

**Goal**: Define the foundational type system. No gameplay yet — just the data model that all future phases build on.

**Deliverables**:
- `src/library/types.rs` — core enums and structs:
  - `TokenType` enum — resource types (Stamina, Health, ProductionOutput, ContractProgress, etc.)
  - `ToolCardKind` — card type tags (Production, QualityControl, Transformation, SystemAdjustment, etc.)
  - `CostTier` enum — Free, Stamina, Health
  - `ContractConstraintKind` enum — OutputThreshold, ResourceBudget, TurnLimit, ToolRestriction, QualityRequirement, SequencingRule
  - `ContractTier` enum — Tier1, Tier2, Tier3, etc.
  - `CardLocation` enum — Library, Deck, Hand, Discard
- `src/library/mod.rs` — module exports
- `src/library/config.rs` — config struct definitions
- `src/library/config_loader.rs` — JSON embedding via `include_str!()`
- `configurations/general/game_rules.json` — initial game constants
- Integration tests verifying type serialization roundtrips

**Reference files from card game**:
- `src/library/types.rs` — the master type file; adapt CardKind→ToolCardKind, TokenType→factory equivalents
- `src/library/config.rs`, `src/library/config_loader.rs` — config loading pattern
- `configurations/general/` — game rules JSON structure

---

## Phase 2: Basic Game Loop

**Goal**: A playable (but minimal) game loop: draw hand → play cards → see production output.

**Deliverables**:
- `src/library/game_state.rs` — `GameState` struct with:
  - Tool card library
  - Player token balances
  - Active contract state
  - Seeded RNG (`rand_pcg::Pcg64`)
- Card drawing: shuffle deck → draw N cards to hand
- Card playing: select a card from hand → apply its production value → move to discard
- Discard for baseline benefit: discard any card for small fixed progress
- Turn resolution: check if contract constraints are met
- `POST /action` endpoint for player actions
- `GET /state` endpoint showing current game state
- Integration tests exercising a full draw → play → check cycle

**Reference files from card game**:
- `src/library/game_state.rs` — GameState initialization and state management
- `src/action/mod.rs` — action dispatch pattern
- `tests/scenario_tests.rs` — integration test style

---

## Phase 3: Contract System

**Goal**: Tier 1 contracts with simple constraints. Fail/succeed flow with new contracts offered on completion or failure.

**Deliverables**:
- `configurations/contracts/tier1.json` — Tier 1 contract definitions
- Contract generation: select a contract from the available pool
- Constraint evaluation: check all constraints simultaneously
- Contract completion: award rewards (new tool cards, tokens)
- Contract failure: no penalty beyond lost time; new contract offered
- Contract market: player chooses from 2-3 available contracts (like scouting in the card game)
- `GET /contracts/available` — list available contracts
- `POST /action` — accept a contract
- Integration tests for contract success and failure paths

**Reference files from card game**:
- `src/library/disciplines/` — encounter logic patterns (adapt to contract evaluation)
- `configurations/combat/` — config structure for encounter definitions
- Scouting system in card game — adapt to contract market selection

---

## Phase 4: REST API & Documentation Endpoints

**Goal**: Full REST API with OpenAPI documentation, following the card game's endpoint pattern.

**Deliverables**:
- All gameplay endpoints with OpenAPI annotations
- `GET /swagger/` — Swagger UI
- `GET /library/cards` — card catalogue (with filters)
- `GET /player/tokens` — token balances
- `GET /contracts/active` — current contract state
- `GET /actions/possible` — allowed actions in current state
- `GET /docs/tutorial` — new player walkthrough
- `GET /docs/hints` — strategy tips
- `GET /docs/designer` — contract/card authoring reference
- `docs/examples/api_examples.sh` — curl-based gameplay example
- `README.md` — project overview with API endpoint table

**Reference files from card game**:
- `src/lib.rs` — route mounting
- `src/library/endpoints.rs` — HTTP handler pattern
- `src/docs/tutorial.rs`, `src/docs/hints.rs`, `src/docs/designer.rs` — documentation endpoint pattern
- `README.md` — project README structure

---

## Phase 5: Deckbuilding

**Goal**: Players acquire new tool cards from contract rewards and can manage their deck composition.

**Deliverables**:
- Contract rewards include new tool cards added to library
- Player can move cards between Library and Deck
- Deck size limits enforced via token system
- Card variety: production cards, quality cards, efficiency cards, utility cards
- `configurations/tools/` — tool card definitions per category
- Integration tests for deck management actions

**Reference files from card game**:
- Card location system (Library → Deck → Hand → Discard cycle)
- `src/library/types.rs` — CardCounts struct and location tracking
- Research/Crafting discipline patterns — adapt to deckbuilding

---

## Phase 6: Contract Tier Progression

**Goal**: Tier 2+ contracts unlock after completing 10 contracts in the previous tier. Higher tiers introduce new constraint types.

**Deliverables**:
- Tier tracking via tokens (ContractsTier1Completed, etc.)
- Tier 2 contracts with 2-3 interacting constraints
- Tier 3 contracts with complex constraint combinations
- New constraint types unlocked per tier
- Rarer tool cards available at higher tiers
- `configurations/contracts/tier2.json`, `tier3.json`
- Integration tests for tier progression

**Reference files from card game**:
- Milestone progression system
- Token-based tracking patterns

---

## Phase 7: Statistics & Metrics

**Goal**: Comprehensive gameplay statistics tracking and reporting.

**Deliverables**:
- `src/library/metrics.rs` — metrics computation
- `GET /metrics` endpoint with:
  - Total contracts completed/failed (per tier)
  - Completion rates
  - Cards played (total and per type)
  - Efficiency metrics (cards per contract, resources per output)
  - Streaks (consecutive successes)
  - Strategy frequency analysis
- Action log for replay/analysis
- `src/library/action_log.rs` — append-only action recording

**Reference files from card game**:
- `src/library/metrics.rs` — metrics endpoint pattern
- `src/library/action_log.rs` — action logging

---

## Phase 8: Adaptive Balance System

**Goal**: Formula-based balance system that adjusts contract difficulty and card effectiveness based on player behavior.

**Important**: This phase uses the formula-based approach from the start. No manual tuning of 100+ parameters.

**Deliverables**:
- Balance formula documentation in `docs/vision/balances/`
- Design-intent parameters (~5-10 numbers per tier):
  - Target output per contract at each tier
  - Expected turns per contract
  - Cost distribution (free/stamina/health ratio)
  - Constraint difficulty scaling factor
- Formula system that derives card values from design-intent parameters
- Adaptive modifiers based on player statistics:
  - Frequently used card types get diminishing returns
  - Underused card types get bonus effectiveness
- Simulation test suite (behind `--features simulation` flag)
- `make balance-check` target
- `.github/skills/balance-tuning-tips/SKILL.md` — created at this point
- `.github/skills/parallel-balance-tuning/SKILL.md` — created at this point

**Reference files from card game**:
- `docs/vision/balances/*.md` — balance documentation style
- `tests/balance/` — simulation test pattern
- `.github/skills/balance-tuning-tips/SKILL.md` — adapt for formula-based approach
- `.github/skills/parallel-balance-tuning/SKILL.md` — worktree-based parallel tuning

---

## Phase 9: Advanced Contract Tiers & Polish

**Goal**: High-tier contracts with deeply multi-constraint puzzles. Polish and quality-of-life improvements.

**Deliverables**:
- Tier 4+ contracts with 4+ simultaneous constraints
- Compound constraints (constraints that interact with each other)
- Rare tool cards with unique mechanics
- Seed-based full game reproducibility (`rand_pcg` deterministic from a single seed)
- Save/load game state
- Config hash verification (`/version` endpoint includes config hash)
- Performance optimization
- Documentation polish

**Reference files from card game**:
- `src/version.rs` — version + config hash endpoint
- Deterministic RNG patterns throughout `game_state.rs`

---

## Deferred Items

These are intentionally deferred and will be addressed in future phases beyond Phase 9:

- **Multiplayer** — Not in scope for the initial game
- **Graphics/UI** — The game is a headless REST API; client development is separate
- **Story/narrative** — Not in scope; the game is purely mechanical
- **Trading/merchants** — May be added as a future contract type
- **MCP server integration** — May be configured for API testing

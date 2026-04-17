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

1. **Formula-based balance from the start**: The card game suffered from exponential balancing complexity because 100+ numeric config values were tuned by trial and error. This project will derive card effect values from ~5-10 design-intent parameters per type using explicit mathematical formulas. One definition per effect/requirement type scales with tier — not one definition per tier.

2. **Single mechanical system**: The card game had 5+ independent mechanical systems (combat, mining, herbalism, woodcutting, fishing), each requiring separate balance work. This project uses one unified production mechanic — contracts vary by their requirement combinations, not by different resolution formulas. This dramatically reduces the balance surface.

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
- `.pre-commit-config.yaml` — cargo fmt + clippy hooks
- `rust-toolchain.toml` — nightly Rust toolchain
- `.github/workflows/ci.yml` — GitHub Actions CI pipeline
- `tests/` directory with at least one smoke test
- `GET /version` endpoint returning `{"version": "<semver>"}` to verify the server runs (config hash added in Phase 1)

**Reference files from card game**:
- `Cargo.toml` — dependency list and feature flags
- `src/main.rs`, `src/lib.rs` — entry point pattern
- `Makefile`, `scripts/*` — build tooling
- `.pre-commit-config.yaml`, `rust-toolchain.toml` — Rust toolchain config
- `.github/workflows/ci.yml` — CI pipeline

---

## Phase 1: Core Types

**Goal**: Define the foundational type system. No gameplay yet — just the data model that all future phases build on.

**Deliverables**:
- `src/types.rs` — core enums and structs:
  - `TokenType` enum — resource/waste types (ProductionUnit, Energy, RawMaterial, Heat, CO2, Waste, Pollution) plus progression tracking (ContractsTierXCompleted)
  - `TokenTag` enum — Beneficial, Harmful, Progression (each token type has a list of tags)
  - `CardTag` enum — card type tags (Production, Transformation, QualityControl, SystemAdjustment, etc.)
  - `CardEffect` enum — effect variants with input/output token lists (PureProduction, Conversion, WasteRemoval, etc.)
  - `ContractRequirementKind` enum — OutputThreshold, HarmfulTokenLimit, CardTagRestriction, TurnWindow
  - `ContractTier` newtype — `ContractTier(pub u32)`, unbounded tier numbering
  - `CardLocation` enum — Library, Deck, Hand, Discard
- `src/config.rs` — config struct definitions
- `src/config_loader.rs` — JSON embedding via `include_str!()`
- `configurations/general/game_rules.json` — initial game constants
- Integration tests verifying type serialization roundtrips

**Reference files from card game**:
- `src/library/types.rs` — the master type file; adapt CardKind→CardTag, TokenType→factory equivalents
- `src/library/config.rs`, `src/library/config_loader.rs` — config loading pattern
- `configurations/general/` — game rules JSON structure

---

## Phase 2: Basic Game Loop & Determinism

**Goal**: A playable (but minimal) game loop: pick contract → play cards one at a time → auto-complete when requirements met. Fully deterministic from the start.

**Deliverables**:
- `src/game_state.rs` — `GameState` struct with:
  - Count-based card tracking (`Vec<CardEntry>` where each entry has `CardCounts { library, deck, hand, discard }`)
  - Player token balances (persisted between contracts, tracked as `Vec<TokenAmount>`)
  - Active contract state with tiered offered contracts (`Vec<TierContracts>`)
  - Seeded RNG (`rand_pcg::Pcg64`)
  - Typed `ActionResult` enum with per-action success/error variants (no generic success/message fields)
  - Action dispatch and all game mechanics
- `src/action_log.rs` — `PlayerAction` enum, `ActionEntry`, `ActionLog` for deterministic replay
- `src/endpoints.rs` — HTTP handlers: `POST /action`, `GET /state`, `GET /actions/history`
- `src/starter_cards.rs` — starter deck card definitions (pure production cards with varying output)
- Card playing: play one card from hand → apply its card effects (add/remove tokens) → draw a replacement card (weighted random from deck counts) → move played card to discard (count mutation)
- Discard for baseline benefit: discard any card for small fixed progress
- Contract auto-completion: after each card play, check if all requirements are met; if so, subtract relevant tokens, award `ContractsTierCompleted(tier)` token, and conclude the contract
- Hand persists between contracts
- Contract reward cards: completing a contract adds its reward card to the player's card library
- Deck recycling: when deck is empty and a draw is needed, discard counts are moved to deck counts (no physical shuffle needed with count-based model)
- `POST /action` endpoint for player actions
- `GET /state` endpoint showing current game state (cards with per-location counts, tiered token list)
- `GET /actions/history` endpoint listing all player actions (seed + action log = save/load)
- **Determinism guarantee**: same version + seed + action list = identical game state
- Integration tests exercising a full pick-contract → play-cards → auto-complete cycle
- Integration tests verifying deterministic reproducibility

**Reference files from card game**:
- `src/library/game_state.rs` — GameState initialization and state management
- `src/action/mod.rs` — action dispatch pattern
- `tests/scenario_tests.rs` — integration test style

---

## Phase 3: Contract System ✅

**Goal**: Tier 1 contracts with simple requirements. Formula-based generation with a 3-contract market per unlocked tier. Infrastructure supports arbitrary tiers for Phase 6.

**Deliverables**:
- `src/contract_generation.rs` — formula-based contract and reward card generation using `TierScalingFormula`:
    - Each contract has a list of enum-based requirements
    - Tier 1 contracts: 1 requirement (OutputThreshold for ProductionUnit, range [5,15])
    - Concrete requirement values generated from tier-based formulas with deterministic randomization
    - `min_tier` field on each formula gates when requirement/effect types become available
- Contract reward cards generated at contract creation time:
    - Reward card has same number of card effects as contract has requirements
    - Each effect matches the tier of a corresponding requirement
    - Tier 1: PureProduction effect producing [1,3] ProductionUnit (matches starter deck range)
    - Concrete effect values rolled from tier formulas — visible to player before accepting
- Contract market: 3 available contracts per unlocked tier, refills (not regenerates) after completion
- Contract completion: auto-completes when all requirements are met, awards the reward card
- No abandon action: contracts either auto-complete or auto-fail (auto-fail only relevant for future tiers with HarmfulTokenLimit/TurnWindow, added in Phase 6)
- `GET /contracts/available` — list available contracts (including reward card preview)
- `POST /action` — accept a contract
- `src/config.rs` — `ContractFormulasConfig` and `TierScalingFormula` structs
- `configurations/general/game_rules.json` — `contract_formulas` section with formula parameters
- `tests/contract_system_test.rs` — 11 integration tests covering market structure, validation, refill, determinism, and rewards

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
- `GET /library/cards` — card catalogue (with filters by tag)
- `GET /player/tokens` — token balances (beneficial and harmful)
- `GET /contracts/active` — current contract state
- `GET /actions/possible` — allowed actions in current state
- `GET /actions/history` — full action log for reproducibility/save-load
- `GET /docs/tutorial` — new player walkthrough
- `GET /docs/hints` — strategy tips
- `GET /docs/designer` — contract/card/token/effect authoring reference
- `docs/examples/api_examples.sh` — curl-based gameplay example
- `README.md` — project overview with API endpoint table

**Reference files from card game**:
- `src/lib.rs` — route mounting
- `src/library/endpoints.rs` — HTTP handler pattern
- `src/docs/tutorial.rs`, `src/docs/hints.rs`, `src/docs/designer.rs` — documentation endpoint pattern
- `README.md` — project README structure

---

## Phase 5: Deckbuilding

**Goal**: Players acquire new player action cards from contract rewards and can manage their deck composition.

**Deliverables**:
- Contract rewards add new player action cards to library (basic version already in Phase 2)
- Player can move cards between Library and Deck
- Card replacement: replacing a card in Deck or Discard with a different card from the Library costs destroying another Library card. Hand cards cannot be replaced — the hand must always reflect random draws from the deck.
- Deck size limits enforced via token system
- Card variety: different card effect combinations and tag sets
- `configurations/card_effects/` — card effect type definitions with tier formulas
- Integration tests for deck management actions

**Reference files from card game**:
- Card location system (Library → Deck → Hand → Discard cycle) — count-based tracking already implemented in Phase 2
- Research/Crafting discipline patterns — adapt to deckbuilding

---

## Phase 6: Contract Tier Progression

**Goal**: Tier 2+ contracts unlock after completing 10 contracts in the previous tier. Higher tiers introduce new requirement types and card effect types.

**Deliverables**:
- Tier tracking via tokens (ContractsTier1Completed, etc.)
- Tier 2 contracts: 1–3 requirements, new requirement types (e.g., harmful token limits)
- Tier 3 contracts: 2–4 requirements, new card effect types (e.g., boosted production with harmful outputs)
- Progressive introduction: each tier unlocks a small group of new effects and requirements
- Stronger player action cards available at higher tiers
- Formula-based scaling: tier X effects/requirements are usually better/harder than tier X−1
- Integration tests for tier progression and new mechanics per tier

**Reference files from card game**:
- Milestone progression system
- Token-based tracking patterns

---

## Phase 7: Statistics & Metrics

**Goal**: Comprehensive gameplay statistics tracking and reporting.

**Deliverables**:
- `src/metrics.rs` — metrics computation
- `GET /metrics` endpoint with:
  - Total contracts completed/failed (per tier)
  - Completion rates
  - Cards played (total and per tag)
  - Efficiency metrics (cards per contract, tokens spent per output)
  - Streaks (consecutive successes)
  - Strategy frequency analysis
- Action log integration (action log already exists from Phase 2)

**Reference files from card game**:
- `src/library/metrics.rs` — metrics endpoint pattern
- `src/library/action_log.rs` — action logging

---

## Phase 8: Adaptive Balance System

**Goal**: Formula-based balance system that adjusts card effectiveness based on player behavior.

**Important**: The formula-based approach is used from Phase 3 onward. This phase adds the adaptive layer on top.

**Deliverables**:
- Balance formula documentation in `docs/design/balances/`
- Design-intent parameters (~5-10 numbers per effect/requirement type):
  - Base token output per tier
  - Input/output ratios for conversion effects
  - Harmful token production/consumption tradeoff factors
  - Requirement difficulty scaling factor
- Adaptive modifiers based on player statistics:
  - Frequently used card tags get diminishing returns
  - Underused card tags get bonus effectiveness
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

**Goal**: High-tier contracts with deeply multi-requirement puzzles. Polish and quality-of-life improvements.

**Deliverables**:
- Tier 4+ contracts with 3–5+ simultaneous requirements
- Complex requirement combinations that interact with each other
- Powerful player action cards with multi-effect combinations
- Config hash verification (`/version` endpoint includes config hash)
- Performance optimization
- Documentation polish

**Reference files from card game**:
- `src/version.rs` — version + config hash endpoint
- Deterministic RNG patterns throughout `game_state.rs`

---

## Deferred Items

These are intentionally out of scope and will not be added:

- **Multiplayer** — not in scope for this game
- **Graphics/UI** — the game is a headless REST API; client development is separate
- **Story/narrative** — not in scope; the game is purely mechanical
- **Token lifecycle** — tokens have no lifecycle (aging, expiry, transformation); they are simple counters
- **Multiple resolution systems** — one unified production mechanic only
- **Quality requirements** — deferred to a future version (not in initial tiers)
- **Sequencing rules** — deferred to a future version
- **Multiple output types** — all output is "production units" for now
- **Trading/merchants** — may be added as a future contract type
- **MCP server integration** — may be configured for API testing

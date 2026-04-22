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
  - `TokenType` enum — resource/waste types (ProductionUnit, Energy, QualityPoint, Innovation, Heat, Waste, Pollution) plus progression tracking (ContractsTierXCompleted)
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
    - Requirement count per contract: `max(1, tier−1)` to `tier+1`, capped by available types
    - Each requirement's tier rolled independently: `max(1, contract_tier−1)` to `contract_tier+1`, filtered by `unlocked_at_tier`
    - Concrete requirement values generated from tier-based formulas with deterministic randomization
    - `unlocked_at_tier` field on each formula gates when requirement/effect types become available
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

## Phase 4: REST API & Documentation Endpoints ✅

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

## Phase 5: Deckbuilding ✅

**Goal**: Players acquire new player action cards from contract rewards and can manage their deck composition.

**Deliverables**:
- ✅ Contract rewards add new player action cards to library shelf (never auto-enter deck)
- ✅ ReplaceCard action: swap a card in Deck or Discard (auto-selected: Deck first) with a shelved Library card, destroying a third shelved card as sacrifice
- ✅ Sacrifice cannot be the same card as the target
- ✅ Fixed 50-card active cycle (deck + hand + discard) — DeckSlots initialized to starting_deck_size and never changes
- ✅ Starter deck: 50 cards generated via tier 1 pure_production formula (output range [2,7])
- ✅ Card variety infrastructure: config-driven effect types (`configurations/card_effects/token_definitions.json`)
- ✅ Tier 0 has only `pure_production`; additional effect types are generated combinatorially in Phase 6
- ✅ `possible_actions()` returns range-based descriptors (one entry per action type with valid index ranges) instead of enumerating all concrete combinations
- ✅ Integration tests for deckbuilding mechanics
- ✅ Updated tutorial, hints, designer docs, README

**Reference files from card game**:
- Card location system (Library → Deck → Hand → Discard cycle) — count-based tracking already implemented in Phase 2
- Research/Crafting discipline patterns — adapt to deckbuilding

---

## Phase 6: Contract Tier Progression ✅

**Goal**: Tier 2+ contracts unlock after completing 10 contracts in the previous tier. Higher tiers introduce new requirement types and card effect types via combinatorial generation and a proportional model.

**Deliverables**:
- ✅ Tier tracking via tokens (ContractsTierXCompleted)
- ✅ **Requirement count formula**: `max(1, contract_tier − 1)` to `contract_tier + 1` requirements per contract (capped by available requirement types)
- ✅ **Per-requirement tier formula**: Each requirement's tier is rolled independently from `contract_tier − 1` to `contract_tier + 1`, filtered by token type availability
- ✅ **Combinatorial effect type generator**: 7 tokens × (producer + consumer/remover) → 13 mains + 85 variations = 98 items, 2 per tier across tiers 0–48
- ✅ **Proportional model**: secondary token amounts as ratios of primary output, with 4 combo directions (direction_sign ±1), boost_factor (1.5), and efficiency_per_tier (0.02)
- ✅ **HarmfulTokenLimit** requirement generator for harmful tokens (Heat, Waste, Pollution)
- ✅ **Requirement tier-gating**: requirements only reference token types with unlocked card effects at or before the contract's tier
- ✅ **Duplicate requirement stacking**: OutputThreshold sums min_amounts for same token; HarmfulTokenLimit takes tightest max_amount
- ✅ Token type redesign: 4 beneficial (ProductionUnit, Energy, QualityPoint, Innovation) + 3 harmful (Heat, Waste, Pollution)
- ✅ `token_definitions.json` replaces `effect_types.json` — ~5 design-intent parameters per token
- ✅ Formula-based scaling: tier X effects/requirements are usually better/harder than tier X−1
- ✅ 24 integration tests covering generator correctness, proportional model, tier-gating, direction_sign, determinism
- ✅ 0-indexed tiers (tier 0 is the first tier)

**Reference files from card game**:
- Milestone progression system
- Token-based tracking patterns

---

## Phase 7: Statistics & Metrics ✅

**Goal**: Comprehensive gameplay statistics tracking and reporting.

**Deliverables**:
- `src/metrics.rs` — `MetricsTracker` (live counters) and `SessionMetrics` response type
- `GET /metrics` endpoint with:
  - Total contracts completed (per tier) with completion rates
  - Cards played (total and per tag) and cards discarded
  - Efficiency metrics (avg cards per contract, token flow per type)
  - Streaks (consecutive contract completions)
  - Strategy analysis (dominant tag, diversity score via Shannon entropy)
  - Deckbuilding stats (cards replaced)
- Live tracking integrated into `GameState` action handlers (O(1) per action)
- Metrics reset on NewGame

---

## Phase 8: Adaptive Balance System ✅

**Goal**: Contract-overlay adaptive balance system that adjusts contract difficulty based on player behavior, plus contract failure conditions.

**Deliverables**:
- ✅ `src/adaptive_balance.rs` — `AdaptiveBalanceTracker` with pressure tracking, decay, failure relaxation, and contract overlay
- ✅ **Contract failure system**: `ContractResolution` enum (Completed/Failed) with `ContractFailureReason` (HarmfulTokenLimitExceeded, TurnWindowExceeded)
- ✅ **Turn tracking**: `contract_turns_played` counter in `GameState`, exposed in state view
- ✅ **Failure-first resolution**: if same action both completes and violates, failure takes precedence
- ✅ **HarmfulTokenLimit enforcement**: after each card play/discard, token balances are checked against contract limits
- ✅ **TurnWindow enforcement**: min_turn prevents premature completion; max_turn violation fails the contract
- ✅ **Adaptive pressure tracking**: gross production per token type, EMA-based pressure accumulation
- ✅ **Contract overlay**: HarmfulTokenLimit tightened (up to 30%), OutputThreshold increased (up to 20%) based on pressure
- ✅ **Decay for unused strategies**: token pressure decays per contract when not produced
- ✅ **Failure relaxation**: all pressures multiplied by relaxation factor on contract failure
- ✅ **Transparency**: `adaptive_adjustments` field on each generated contract; `adaptive_pressure` in `/metrics`
- ✅ **Metrics updates**: `contracts_failed`, `contracts_attempted_per_tier`, real `completion_rate`
- ✅ **Configuration**: `adaptive_balance` section in `game_rules.json` (alpha, decay_rate, failure_relaxation, max_tightening_pct, max_increase_pct, normalization_factor)
- ✅ **BREAKING API change**: `contract_completed` replaced with `contract_resolution` containing `ContractResolution` enum
- ✅ Integration tests for contract failure, adaptive overlay, pressure mechanics
- ✅ Updated documentation (tutorial, hints, designer, vision, roadmap)

---

## Phase 9: Advanced Contract Tiers & Polish ✅

**Goal**: High-tier contracts with deeply multi-requirement puzzles, unified range-based requirement types, and quality-of-life improvements.

**Deliverables**:
- ✅ **BREAKING: TokenRequirement unification** — replaced `OutputThreshold` and `HarmfulTokenLimit` with a single `TokenRequirement { token_type, min: Option<u32>, max: Option<u32> }` variant; `min` = completion threshold (beneficial), `max` = failure cap (harmful); both may be set simultaneously at higher tiers
- ✅ **BREAKING: CardTagConstraint** — replaced `CardTagRestriction` with unified `CardTagConstraint { tag, min: Option<u32>, max: Option<u32> }` (ban = max 0, must-play = min N, range = both); unlocks at tier 12 (Waste→QP gap)
- ✅ **BREAKING: PlayCard/DiscardCard actions** — `hand_index` renamed to `card_index` everywhere; `card_index` is a direct index into the `/state` cards Vec (a valid play/discard requires `counts.hand > 0`); `InvalidHandIndex` renamed to `InvalidCardIndex`
- ✅ **BREAKING: possible_actions shape** — `valid_hand_index_range` replaced by `valid_card_indices: Vec<usize>` for both PlayCard and DiscardCard, listing only currently-playable/discardable card indices
- ✅ **CardTagConstraint enforcement** — `cards_played_per_tag_contract` tracking in `GameState`; banned tag plays blocked in `handle_play_card()`; min tag count enforced in `all_requirements_met()`; new `CardTagBanned` failure reason
- ✅ **TurnWindow generation** — three tier-gated variants: Only-Max/deadline (unlocks tier 6), Only-Min/earliest-start (tier 10), Both/window (tier 14); `TurnWindow` fields are now `Option<u32>` (BREAKING); formula fixes: `min_turn` rolls `[0, max_min_turn]` (0 always possible), window size decreases with tier but always has ≥2 possible values
- ✅ **CardTagConstraint generation** — three tier-gated variants: Only-Max (tier 12), Only-Min (tier 16), Both (tier 20); formula uses same window logic as TurnWindow; Only-Max rolls down to 0 (a natural ban — no special case); gated by `unlocked_card_tags()` to only use tags with available cards at the contract tier
- ✅ **Performance: effect type caching** — `token_defs` + `effect_types` cached in `GameState::new_with_rules()`; eliminated per-contract config reload
- ✅ **Config hash in /version** — `config_hash: String` field added to `VersionInfo`; FNV-1a 64-bit XOR hash of both embedded JSON configs, returned as 16-char hex
- ✅ **Documentation polish** — tutorial, hints (added tier 6 and tier 12 sections), designer guide, and README updated with new requirement type names, unlock tiers, and enforcement semantics

**Reference files from card game**:
- `src/version.rs` — version + config hash endpoint
- Deterministic RNG patterns throughout `game_state.rs`

---

## Phase 10: Game Balancing

**Goal**: Fine-tune the game so that simple, repetitive strategies perform measurably worse than adaptive, multi-dimensional strategies. Ensure the difficulty curve feels fair and purposeful across all tiers.

**Success metric**: Total actions from game start to first contract completion at milestone tiers 10, 20, 30, 40, and 50 (0-indexed). Measured by automated strategy simulation tests under `tests/simulation/` (run with `cargo test --features simulation --test simulation --release -- --nocapture`).

**Known limitation of current pressure model**: The current pressure signal tracks gross token production per token type. In a well-developed deck, most token types will be in regular use simultaneously — so nearly all token pressures grow together. The system may behave more like a global difficulty escalator than a targeted strategy-detection mechanism, tightening requirements on nearly all tokens at once rather than selectively penalizing the dominant strategy.

**Strategy identification improvement investigation**: If the above limitation proves significant in testing, explore replacing or supplementing the token-production pressure signal with a card-tag-based strategy dominance model:
- Track per-contract distribution of cards played by tag (Production, Transformation, QualityControl, SystemAdjustment)
- Compute a dominance score for the leading tag (e.g., Gini coefficient or top-tag share vs total)
- Apply elevated pressure only to requirements associated with the dominant strategy, not uniformly across all tokens
- This correctly distinguishes "spam Production cards" from a balanced mixed approach and provides targeted resistance

---

### Phase 10.1 — Simulation Infrastructure & SimpleFirst Strategy ✅

**Goal**: Create the simulation framework and one simple strategy. Report how far it gets at each tier milestone, and what blocks it.

**Deliverables**:
- `tests/simulation/` — simulation test binary (feature-gated: `--features simulation`)
  - `game_driver.rs` — drives game sessions via in-process Rocket test client
  - `runner.rs` — runs multiple seeds, aggregates milestone stats
  - `strategies/mod.rs` — `Strategy` trait
  - `strategies/simple_first.rs` — `SimpleFirstStrategy`
  - `main.rs` — `#[test]` entry point with soft blocker reporting
- `docs/design/roadmap.md` — this file updated with Phase 10 sub-phases

**SimpleFirstStrategy behaviour**: always accepts the highest available tier contract, plays the first valid card, discards when no card can be played, never deckbuilds.

**Finding from Phase 10.1**: `SimpleFirstStrategy` reaches **tier 3** and stalls permanently. After completing ~30–40 contracts (tiers 0–3), it accepts a contract whose token requirements the starter deck cannot fulfill. Because no `TurnWindow` constraint exists before tier 6, the contract neither completes nor fails — the game loop runs indefinitely on that single contract. This reveals two balance issues to address in Phase 10.5: (1) the starter deck should provide at least a minimal path through early tiers, and (2) a TurnWindow should apply at all tiers so uncompletable contracts eventually time out.

---

### Phase 10.2 — Multiple Simple Strategies

**Goal**: Add 2–3 more simple strategies for comparison. Each reads only `possible_actions`, applies no contract-aware heuristics, and never deckbuilds.

**Strategies to add**:
- `RandomStrategy` — picks uniformly at random from all valid actions (seeded RNG for reproducibility)
- `MaxProductionStrategy` — plays the card with the highest total output token production (by index as a tie-breaker); falls back to random
- `AlwaysDiscardStrategy` — always discards instead of playing (worst-case baseline)

**Expected outcome**: All simple strategies stall in the same tier range (0–5). `RandomStrategy` ≈ `SimpleFirst`; `MaxProduction` slightly better; `AlwaysDiscard` much worse or never advances.

---

### Phase 10.3 — First Advanced Strategy

**Goal**: One strategy that reads contract requirements and makes smarter decisions.

**Strategy: `ContractAwareStrategy`**:
- Reads active contract token requirements from `GET /state`
- Prefers cards that produce tokens the active contract needs
- Avoids cards whose production would trigger `HarmfulTokenLimitExceeded`
- Performs deckbuilding: replaces starter cards with reward cards between contracts

**Expected outcome**: Reaches higher tiers than all simple strategies; demonstrates that the game is beatable with informed play.

---

### Phase 10.4 — Multiple Advanced Strategies

**Additional advanced strategies**:
- `TierFocusStrategy` — always accepts the highest available tier contract, reads requirements, prioritises matching cards
- `SafePlayStrategy` — monitors harmful token levels vs. contract limits; discards instead of playing cards that would cause failures
- `DeckbuilderStrategy` — extends `ContractAware` with aggressive deckbuilding: prioritises reward cards that unlock new token types

---

### Phase 10.5 — Iterative Balancing

**Goal**: Use simulation results to tune adaptive balance parameters and verify the intended strategy hierarchy.

**Balancing targets**:
- Simple strategies should reach noticeably lower tiers per unit of actions than advanced strategies
- Advanced strategies should show measurably better tier-milestone action counts (≥20% faster to tier 20+)
- Parameters to tune: `alpha`, `decay_rate`, `failure_relaxation`, `max_tightening_pct`, `max_increase_pct`, `normalization_factor` in `game_rules.json`
- Address Phase 10.1 finding: add `TurnWindow` constraints at all tiers (not just tier 6+) so no contract can run indefinitely
- Add regression assertions once a good balance point is found (similar to card game's win-rate band assertions)

---

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

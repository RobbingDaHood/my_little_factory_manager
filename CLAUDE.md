# CLAUDE.md

## Build, test, and lint commands

- **Primary validation**: `make check` — runs formatting (auto-fix), clippy, build, tests, and coverage (80% threshold) in one pass.
- **Run dev server**: `cargo run` (listens on http://localhost:8000).
- **Single test**: `cargo test <test_name>` (substring matching supported).
- **Tests with output**: `cargo test -- --nocapture`.
- **Strategy simulation tests** (expensive, opt-in): `cargo test --features simulation --test simulation -- --include-ignored --nocapture`.
- **Coverage only**: `cargo llvm-cov --workspace --fail-under-lines 80`.
- Pre-commit hooks auto-run `cargo fmt` and `cargo clippy` on every commit.
- All tests and coverage must pass before pushing. CI enforces ≥80% line coverage. Never commit known test failures.

## High-level architecture

- Rust web API built with Rocket, exposing REST endpoints for player action cards, contracts, and factory management.
- All runtime behaviour is exposed via HTTP; most functionality is tested with integration tests that drive the API.
- OpenAPI/Swagger enabled via `rocket_okapi`; view Swagger UI at `/swagger/` when the server is running.

## Key conventions

**Tests**
- Place tests in `tests/` (not inline in `src/`). Prefer integration tests over unit tests.
- Do not make items `pub` solely to enable unit testing — test through the HTTP API.
- Aim for ≥90% coverage before committing.
- Test driven development: Write failing tests first, Then implement minimal fix, then refactor.

**Code style**
- No `unwrap()` in production code; propagate `Result` explicitly.
- Zero Clippy warnings.
- Prefer small, well-named wrapper functions over long comments. Remove comments that restate clear names.
- Breaking changes are allowed and encouraged. Prefix the commit summary with `BREAKING:` and list what changed in the body.

**Types**
- "Everything is a deck": core state is modelled as decks of player action cards moving between Shelved, Deck, Hand, and Discard states.
- Use enums for closed sets of variants (`CardTag`, `CardLocation`, `CardEffect`).
- Use newtype wrappers (`struct ContractTier(pub u32)`) for unbounded but typed values.
- Use plain strings only for truly dynamic, designer-driven values.
- Return typed `Json<T>` from handlers and derive `JsonSchema` so OpenAPI stays accurate.
- Avoid `RawJson`; map domain types to serde-serializable structs.

**Features and dependencies**: follow existing `Cargo.toml` features when adding dependencies.

## Documentation maintenance

- **OpenAPI doc comments**: update `///` comments on handler functions and action enum variants when adding or changing endpoints. Explain strategic purpose, not just the signature.
- **README.md**: add new endpoints to the API endpoint table.

## GitHub operations

Use `gh` and `git` for all repository and GitHub operations. `gh` authenticates via `GH_TOKEN` in `.env` (never commit `.env`). If `GH_TOKEN` is not set, source it: `export $(cat .env | xargs)`.

## Key project files

- `Cargo.toml` — dependencies and features
- `src/main.rs` — binary entry point
- `src/lib.rs` — library root, route mounting, `rocket_initialize()`
- `src/version.rs` — `GET /version`
- `src/types.rs` — core enums and structs
- `src/config.rs` — `GameRulesConfig`, `CardEffectTypeConfig`, `CardEffectVariation`, `ModifierRange`
- `src/config_loader.rs` — JSON config embedding and loading
- `src/game_state.rs` — `GameState`, game mechanics, action dispatch
- `src/action_log.rs` — `PlayerAction`, `ActionEntry`, `ActionLog` for deterministic replay
- `src/metrics.rs` — `MetricsTracker`, `SessionMetrics`
- `src/contract_generation.rs` — formula-based contract and reward card generation
- `src/endpoints.rs` — HTTP handlers
- `src/starter_cards.rs` — starter deck generation
- `configurations/general/game_rules.json` — externalized game constants
- `configurations/card_effects/effect_types.json` — card effect type definitions
- `Makefile` — `check`, `coverage`, `install-hooks` targets
- `scripts/check_all.sh` — unified validation script
- `scripts/worktree-manage.sh` — worktree lifecycle management
- `rust-toolchain.toml` — nightly Rust toolchain
- `.github/workflows/ci.yml` — CI pipeline
- `tests/smoke_test.rs`, `tests/game_loop_test.rs`, `tests/contract_system_test.rs`, `tests/determinism_test.rs`, `tests/api_endpoints_test.rs`, `tests/deckbuilding_test.rs`, `tests/metrics_test.rs`

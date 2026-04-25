# My Little Factory Manager

A deterministic, turn-based deckbuilding game where the player acts as a factory manager fulfilling contracts from an open market. Built as a headless REST API with OpenAPI documentation.

## Game Concept

The core mechanic revolves around **contracts** drawn from a tiered market. Each contract defines production requirements that must be satisfied by playing action cards from your hand. Cards cycle through locations: **Library → Deck → Hand → Discard**, and the hand persists between contracts.

Tokens represent persistent resources — **beneficial** (ProductionUnit, Energy, QualityPoint, Innovation), **harmful** (Heat, Waste, Pollution), and **progression** (ContractsTierCompleted, DeckSlots). Managing token balances is the strategic heart of the game.

## Features

- RESTful API with 13 endpoints for full gameplay
- Tiered contract system with formula-based balance scaling
- Contract failure conditions (TokenRequirement max violations, TurnWindow exceeded, CardTagBanned plays blocked)
- Adaptive balance overlay — contracts adjust to player behavior patterns
- Deckbuilding via ReplaceCard action — reshape your active cycle by swapping and sacrificing cards
- Active cycle size controlled by DeckSlots progression token
- Config-driven card effect types (`configurations/card_effects/token_definitions.json`)
- Deterministic replay via seed + action log (save/load)
- Externalized game-rules configuration (`configurations/general/game_rules.json`)
- Version fingerprint via `GET /version` (game version + config hash)
- OpenAPI/Swagger documentation at `/swagger/`
- Comprehensive test coverage (integration tests, ≥80% line coverage)
- Input validation and descriptive error messages

## Prerequisites

- Rust nightly (pinned in `rust-toolchain.toml`)
- Cargo (comes with Rust)

## Installation

1. Clone the repository:
```bash
git clone https://github.com/RobbingDaHood/my_little_factory_manager.git
cd my_little_factory_manager
```

2. Build the project:
```bash
cargo build --release
```

## Running the Server

Start the development server:
```bash
cargo run
```

The server will start on `http://localhost:8000` by default.

### Custom Port Configuration

```bash
ROCKET_PORT=8001 cargo run
```

See [Rocket configuration docs](https://rocket.rs/v0.5/guide/configuration/) for all `ROCKET_` options.

## API Documentation

Once the server is running, access the interactive Swagger UI at:
```
http://localhost:8000/swagger/
```

### Key Endpoints

#### Game Actions
| Endpoint | Purpose |
|----------|---------|
| `POST /action` | Submit a player action (NewGame, AcceptContract, PlayCard, DiscardCard, ReplaceCard, AbandonContract) |
| `GET /actions/possible` | List currently valid actions with index ranges |
| `GET /actions/history` | Full action history for deterministic replay |

#### Game State
| Endpoint | Purpose |
|----------|---------|
| `GET /state` | Complete game state snapshot |
| `GET /player/tokens` | Token balances grouped by beneficial/harmful/progression |
| `GET /contracts/available` | Open contract market grouped by tier |
| `GET /contracts/active` | Currently active contract (or null) |

#### Library
| Endpoint | Purpose |
|----------|---------|
| `GET /library/cards` | Card catalogue with optional `?tag=` filter (JSON object: `{"input":[],"output":["ProductionUnit"]}`, URL-encoded) |

#### Statistics
| Endpoint | Purpose |
|----------|---------|
| `GET /metrics` | Session gameplay statistics (completions, failures, efficiency, streaks, adaptive pressure) |

#### System
| Endpoint | Purpose |
|----------|---------|
| `GET /version` | Game version and config fingerprint |
| `GET /openapi.json` | OpenAPI specification |
| `GET /swagger/` | Interactive Swagger UI |

### Example: Starting a Game

```bash
# Start a new game with seed 42 (deterministic)
curl -X POST http://localhost:8000/action \
  -H "Content-Type: application/json" \
  -d '{"action_type": "NewGame", "seed": 42}'

# See what actions are available
curl http://localhost:8000/actions/possible

# View contract market
curl http://localhost:8000/contracts/available

# Accept a contract (tier 0, contract index 0)
curl -X POST http://localhost:8000/action \
  -H "Content-Type: application/json" \
  -d '{"action_type": "AcceptContract", "tier_index": 0, "contract_index": 0}'

# Play the first card in hand (use /actions/possible valid_card_indices for the actual index)
curl -X POST http://localhost:8000/action \
  -H "Content-Type: application/json" \
  -d '{"action_type": "PlayCard", "card_index": 0}'

# Check token balances
curl http://localhost:8000/player/tokens

# Replace a deck card with a shelved library card (between contracts)
curl -X POST http://localhost:8000/action \
  -H "Content-Type: application/json" \
  -d '{"action_type": "ReplaceCard", "target_card_index": 0, "replacement_card_index": 3, "sacrifice_card_index": 1}'
```

## Development

### Seeding and Reproducibility

- Provide a seed when starting a new game: `{"action_type": "NewGame", "seed": 42}`
- The server records every action in the ActionLog (`GET /actions/history`) so runs can be reproduced from seed + action sequence.

### Running Tests

Run the full validation suite (formatting, clippy, build, tests, coverage):
```bash
make check
```

Run only the test suite:
```bash
cargo test
```

Run a single test by name:
```bash
cargo test library_cards_returns_all
```

Run tests with output:
```bash
cargo test -- --nocapture
```

### Code Quality

```bash
# All-in-one validation (recommended before every commit)
make check

# Individual checks
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
cargo llvm-cov --workspace --fail-under-lines 80
```

### Project Structure

```
configurations/             # JSON game content (embedded at compile time)
├── general/
│   └── game_rules.json     # Game-wide mechanics constants
└── card_effects/
    └── token_definitions.json   # Token definitions and variation defaults

src/
├── lib.rs                  # Library entry point, route mounting
├── main.rs                 # Binary entry point
├── endpoints.rs            # HTTP handlers (action dispatch, state queries)
├── game_state.rs           # GameState struct, game mechanics, action dispatch
├── types.rs                # Core enums and structs (TokenType, CardEffect, Contract)
├── action_log.rs           # PlayerAction enum, ActionEntry, ActionLog
├── contract_generation.rs  # Combinatorial effect type generator and contract generation
├── starter_cards.rs        # Starter deck generation from generated effect types
├── config.rs               # Config struct definitions (GameRulesConfig)
├── config_loader.rs        # JSON config embedding and loading
└── version.rs              # GET /version endpoint

tests/
├── api_endpoints_test.rs       # New endpoint integration tests
├── contract_system_test.rs     # Contract generation and market tests
├── deckbuilding_test.rs        # Deckbuilding mechanics tests
├── determinism_test.rs         # Seed reproducibility tests
├── game_loop_test.rs           # Core gameplay loop tests
├── smoke_test.rs               # Basic server endpoint tests
├── starter_cards_test.rs       # Starter deck validation tests
├── tier_progression_test.rs    # Phase 6 tier progression and proportional model tests
└── types_serialization_test.rs # Type serialization roundtrip tests

docs/
└── design/
   └── vision.md            # High-level design principles
```

## Game Configuration

Card, effect, and game-rules definitions are externalized as JSON in `configurations/`. Files are embedded at compile time via `include_str!()` — no runtime file I/O is needed.

- **`general/game_rules.json`** — Game-wide constants (hand size, market size, discard bonus, tier progression thresholds, deck slot reward chance, scaling formulas for output_threshold and harmful_token_limit)
- **`card_effects/token_definitions.json`** — Defines all 7 game tokens with primary formulas, tags, and beneficial/harmful classification. The combinatorial generator auto-produces 98 card effect types (13 mains + 85 variations) from ~5 parameters per token.

## Card Locations

Cards transition through locations during gameplay:
- **Library** — the complete catalogue of owned cards (library ≥ deck + hand + discard)
- **Deck** — the player's current operational toolset
- **Hand** — actions available for the current turn
- **Discard** — used actions awaiting recycling back into the Deck
- **Shelved** — library copies not in the active cycle (library − deck − hand − discard)

When the Deck is empty, the discard pile is shuffled back into the Deck.

## Deckbuilding

Between contracts, the **ReplaceCard** action lets you swap a card in your Deck or Discard pile (auto-selected: Deck first, then Discard) with a shelved library card. A third shelved card is permanently destroyed as the cost (sacrifice). The sacrifice cannot be the same card as the target. This is the only way to change your active cycle composition.

The active cycle (Deck + Hand + Discard) is fixed at 50 cards and never changes. Reward cards always go to the library shelf — use ReplaceCard to bring them into the active cycle.

## Design Philosophy

- **Encapsulation**: Internal APIs remain private; all interactions go through public HTTP endpoints
- **Type Safety**: Leverages Rust enums and the type system for correctness
- **Self-Documenting**: The API explains itself via rich OpenAPI comments
- **Error Handling**: No panics in production code; all errors return proper HTTP status codes
- **Formula-Based Balance**: Card values and contract difficulty derive from ~5-10 design-intent parameters rather than tuning 100+ config values
- **Testing**: Comprehensive integration tests covering all endpoints and edge cases

## Contributing

Key principles:
- Zero clippy warnings
- No `unwrap()` calls in production code
- ≥80% line coverage enforced via `make check`
- Meaningful commit messages
- Pre-commit hooks auto-run `cargo fmt` and `cargo clippy`

Install hooks:
```bash
make install-hooks
```

## License

MIT — see [LICENSE](LICENSE) for details.

## Author

RobbingDaHood

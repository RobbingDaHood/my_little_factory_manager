# Game Terminology Dictionary

Canonical definitions for game terms used throughout the codebase, documentation, and API.

---

| Term | Definition |
|------|-----------|
| **Active Cycle** | The set of non-shelved cards: Deck + Hand + Discard. Fixed at `starting_deck_size` (default 50) and never changes in size. |
| **Card** | A reusable action with effects (inputs/outputs on tokens). Cards move between locations but are never consumed — only shelved copies can be destroyed via sacrifice. |
| **CardCounts** | Per-location copy counts for a single card type: `shelved`, `deck`, `hand`, `discard`. Total owned = sum of all four. |
| **CardEntry** | A unique card definition paired with its `CardCounts`. The player's card collection is a `Vec<CardEntry>`. |
| **Contract** | A challenge that requires accumulating specific token thresholds. Completing a contract awards a reward card. |
| **Deck** | The draw pile — cards available for drawing into the hand. One of the four card locations. |
| **DeckSlots** | A progression token tracking the active cycle size limit (`deck + hand + discard`). Initialized to `starting_deck_size`. |
| **Discard** | Used cards awaiting recycling. When the Deck is empty, the Discard pile is shuffled back into the Deck. |
| **Hand** | Cards currently held by the player, available to play or discard. |
| **ReplaceCard** | The deckbuilding action: swap a Deck/Discard card (target) with a Shelf card (replacement), destroying another shelved card (sacrifice). Only available between contracts. |
| **Sacrifice** | A shelved card copy destroyed as the cost of a ReplaceCard action. |
| **Shelf / Shelved** | Cards owned but not in the active cycle. Reward cards arrive here. Use ReplaceCard to move them into the Deck. `CardCounts.shelved` tracks copies on the shelf. |
| **Starter Deck** | The initial set of pure-production cards placed directly into the Deck at game start (`shelved: 0, deck: N`). |
| **Target** | The Deck or Discard card being swapped out during a ReplaceCard action. It moves to the Shelf. |
| **Tier** | A progression level for contracts and card effects. Higher tiers unlock stronger effects and harder contracts. |
| **Token** | A resource produced or consumed by card effects. Types include `ProductionUnit`, `QualityPoint`, `TransformationCharge`, and `SystemAdjustmentUnit`. |

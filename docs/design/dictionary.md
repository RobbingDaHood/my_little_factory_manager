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
| **Token** | A resource produced or consumed by card effects. Beneficial types: `ProductionUnit`, `Energy`, `QualityPoint`, `Innovation`. Harmful types: `Heat`, `Waste`, `Pollution`. Progression types: `ContractsTierCompleted`, `DeckSlots`. |
| **MainEffectDirection** | Whether a card effect type's primary token is an output (Producer) or an input (Consumer/Remover). |
| **VariationDirection** | Whether a variation's secondary token is an Input or Output on the card effect. |
| **Direction Sign** | +1 if the secondary token represents a player tradeoff (harmful output, beneficial input) → boosts primary. −1 if it represents a player advantage (harmful input, beneficial output) → penalizes primary. |
| **Proportional Model** | The system where secondary token amounts are derived as ratios of the unmodified primary output, ensuring consistent scaling across tiers. |
| **Self-Consuming Variation** | A variation where the secondary token is the same type as the primary. The secondary direction is opposite to the main (producer self-consuming = input, consumer self-consuming = output). |
| **Cross-Token Variation** | A variation pairing two different token types. Placed on the earlier token's mains to prevent duplicates. |

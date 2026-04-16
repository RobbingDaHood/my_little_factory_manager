# 🎮 My Little Factory Manager — Vision

## 🧭 Core Concept

**My Little Factory Manager** is a deterministic, turn-based deckbuilding game where the player acts as a factory manager fulfilling contracts from an open market. Each contract defines a set of production requirements that must be satisfied using player action cards drawn from a deck.

The game is built around a single unified production mechanic — contracts provide variety through their requirement combinations, not through fundamentally different resolution systems. Every contract uses the same core interaction model, and strategic depth emerges from deckbuilding, token management, and adaptation to shifting contract conditions.

---

## 🧠 Core One-Liner

> The limited hand size represents the manager's limited operational focus and execution capacity — only a small subset of available actions can be actively deployed at once.

---

## ⚙️ Core Gameplay Loop

1. The player **picks a contract** from the available market.
2. The player **plays cards one at a time** until the contract is resolved.
   * For each card played, a **new card is drawn** from the deck as replacement.
   * The **hand is persisted between contracts** — it carries over from one contract to the next.
3. As soon as **all requirements of the contract are fulfilled**, it automatically subtracts the relevant tokens and concludes.
4. **Repeat** — the player picks a new contract and continues.

The **deck** represents all available factory capabilities. The **hand** represents what the manager can realistically execute in the current operational window.

---

## 🃏 Player Action Cards

Player action cards are the player's primary interaction mechanism. Each card represents an operational action the manager can take — this includes tools, techniques, deals, and any other factory operation.

### Card Properties

Each player action card has:

* a **list of type tags** identifying its operational categories (e.g., production, transformation, quality control, system adjustment)
* a **list of card effects** — each effect is an enum variant that defines token inputs and outputs

### Card Effects

Each card effect is an enum variant with a list of **inputs** and **outputs**:

* At least one of inputs or outputs is non-empty.
* Inputs **remove tokens** from the player's persisted token list.
* Outputs **add tokens** to the player's persisted token list.

#### Tokens

Tokens are the universal currency of the game:

* The token list is **persisted between contracts** — they carry over.
* Tokens are **enum-based** and have no logic of their own — they solely exist to be referenced by card effects and contract requirements.
* There is **no lifecycle** for tokens (unlike the sibling card game) — they are simply added and removed.
* Each token type has a **list of tags**, at minimum indicating whether it is:
  * **Beneficial** — a resource, energy, production unit, etc.
  * **Harmful** — heat, CO2, waste, pollution, etc.

#### Card Effect Variants

Card effects come in many variants, including:

* **Pure production** — requires no input, produces tokens (may include harmful byproducts for higher output).
* **Conversion** — takes a beneficial token as input, produces beneficial tokens in a larger amount.
* **Waste removal** — takes a harmful token as input, produces nothing (cleans up pollution/waste).
* **Mixed variants** — combinations of the above with varying input/output ratios.

There are many possible variations. The key design principle: **more powerful beneficial output comes with tradeoffs** — either consuming valuable inputs or producing harmful byproducts.

### Card Locations

Cards move between distinct locations during gameplay:

* **Library** — the complete catalogue of available actions
* **Deck** — the player's current operational toolset
* **Hand** — actions available for the current turn
* **Discard** — used actions awaiting recycling back into the deck

When the deck is empty and a draw is required, the entire discard pile is shuffled back into the deck.

### Card Replacement (Deckbuilding)

Players can replace a card in the **Deck** or **Discard** with a different card from the **Library**, but at a cost: doing so **destroys** another card in the Library. This creates a meaningful tradeoff — improving the active deck requires permanently reducing the total card pool.

**Hand cards cannot be replaced directly.** The hand must always be the result of random draws from the deck, preserving the core randomness of the draw mechanic. The only way to influence hand composition is by shaping which cards are in the deck.

---

## 📜 Contracts

Contracts are the primary source of gameplay challenge and the driver of all strategic decisions.

### Contract Definition

Each contract has a **list of contract requirements**. Each requirement is an enum variant. A contract is completed when **all requirements are satisfied simultaneously**.

### Requirement Types

* **Output threshold** (mandatory on every contract) — produce at least N production units.
* **Harmful token limits** — complete without exceeding a maximum amount of specific harmful tokens.
* **Card tag restrictions** — certain card tags are unavailable or penalized during this contract.
* **Turn window** — the contract must be completed between turn X and turn Y (inclusive).

### Possible Future Requirements

These may be added in later versions:

* **Quality requirements** — output must meet a minimum quality level (quality is not implemented in the first version).
* **Sequencing rules** — certain operations must happen in a specific order.
* **Multiple output types** — currently all output is just "production units"; future contracts may require specific output types.

### Contract Outcomes

Contracts are:

* **failable** — not every contract can be completed with every hand
* but always followed by **new opportunities** — the market never runs dry
* failure does not end progression, only **slows it**

---

## 🏗 Contract Tier System

Contracts are organized into tiers that represent increasing structural complexity.

### Tier Structure

* A **Tier X** contract has **X−1 to X+1 requirements** (minimum of 1).
* At least one requirement must be an **output threshold**.
* Each individual requirement is of **tier X−1 to X+1** difficulty.

### Tier Progression

* Completing **10 contracts in a tier unlocks the next tier**.
* Higher tiers introduce:
  * new requirement types not seen in lower tiers
  * more complex combinations of existing requirements
  * access to stronger and more specialized player action cards

### Card Rewards

The card reward from completing a contract **matches its difficulty**:

* The reward card has the **same number of card effects as the contract had requirements**.
* Each card effect has the **same tier as a matching requirement** from the contract.
* The concrete values of each effect are **randomized within the tier's range** — so there is variation even between same-tier rewards.
* The concrete reward card is **generated when the contract is generated** — the player can see exactly what card they would earn before accepting a contract.

Higher tiers do not just add difficulty — they increase the **structural complexity of contracts**, requiring qualitatively different strategic approaches, and reward correspondingly more powerful cards.

---

## 🔧 Predefined Card Effects & Contract Requirements

There is a **predefined list of possible card effect types and contract requirement types**. Each type defines:

* A **formula** that calculates value ranges based on a given tier.
* When a "possible" effect/requirement becomes a **concrete** one, the ranges are rolled (deterministically) to produce specific values.

### Formula-Based Balancing

The formula system ensures:

* A **tier X** effect is usually better than a **tier X−1** of the same type (better exchange rates, higher numbers).
* A **tier X** requirement is usually tougher than a **tier X−1** of the same type (stricter thresholds).
* There can be **some overlap between neighboring tiers** — not every tier X is strictly better/harder than every tier X−1.
* There is **one definition per effect/requirement type that scales with tier** — not one definition per tier. Given a tier, the formula calculates the appropriate ranges, then deterministic randomization produces a concrete instance.

### Balance Rules

* A card effect that **consumes harmful tokens** should produce fewer beneficial tokens than one that does not (removing waste is its own reward).
* A card effect that **produces harmful tokens** should produce more beneficial tokens than one that does not (pollution is a meaningful tradeoff).

### Progressive Tier Introduction

* All possible card effect types and contract requirement types have a **minimum tier** where they first appear.
* **Tier 1** is very simple — perhaps just pure output production cards and basic output threshold contracts.
* **Each subsequent tier introduces a small group of new effects and requirements.** For example, tier 2 might introduce energy production and consumption (as card effects) and a "max energy use" contract requirement.

---

## 🃏 Player Discard System

Players can always discard a card for a **small baseline benefit**.

This ensures:

* no turn becomes completely unusable
* every decision has forward momentum
* suboptimal hands still allow partial progress

The discard benefit is intentionally small — it prevents dead turns without removing the incentive to play cards strategically.

---

## 🔁 Adaptive System

The game tracks player behavior continuously, including:

* cards played and their frequency
* strategy patterns across contracts
* contract outcomes (success, failure, efficiency)
* token usage patterns

Based on this data:

* frequently used strategies become **less efficient** over time within contracts
* previously underused mechanics gradually become **more valuable**

This creates a shifting strategic landscape where long-term optimization requires **adaptation rather than repetition**. The player cannot find a single dominant strategy and ride it indefinitely — the system gently pushes toward variety and exploration.

### Design Intent

The adaptive system is not punitive. Its purpose is to:

* reward players who explore multiple approaches
* prevent the game from becoming solvable with a single optimal path
* create a sense of a living, responsive factory environment
* ensure that strategic depth scales with player skill and experience

---

## 📊 Progression & Statistics System

The game tracks detailed global and per-run statistics, including:

* **total contracts completed** (overall and per tier)
* **contracts failed** (overall and per tier)
* **completion rates per tier** — success percentage
* **cards played** (total and per type tag)
* **strategy frequency** — how often specific card combinations are used
* **efficiency metrics** — cards used per contract completion, tokens spent per output unit
* **streaks** — consecutive successful contracts without failure
* **specialization metrics** — dominant strategy types and diversity scores

Statistics serve both as player feedback (visible progression) and as input to the adaptive system.

---

## 🔄 Determinism & Reproducibility

Given the **same game version**, **same seed**, and **same ordered list of player actions**, the game must **deterministically produce the exact same state**.

### Action History Endpoint

An endpoint lists all player actions taken in the current game. Feeding this same action list into a clean game with the same version and seed reproduces the exact same state. This serves as the **save/load game** feature — there is no separate save file, only the seed and action log.

---

## 🏆 Long-Term Motivation

Progression is driven by:

* **unlocking higher contract tiers** — accessing more complex and rewarding challenges
* **discovering new action cards** — expanding the catalogue of available factory capabilities
* **optimizing execution efficiency** — completing contracts with fewer cards and tokens
* **mastering adaptation** — thriving across shifting contract conditions and requirement combinations

The system supports both:

* **performance-based mastery** — speed, efficiency, consistency, high-tier completion rates
* **expression-based playstyles** — unique solution patterns, diverse strategy exploration

---

## 🎯 Core Design Goal

The game is built around a single principle:

> Success is not defined by building the strongest system, but by continuously adapting how your limited operational capacity is used under evolving contract requirements.

The factory is never "solved." Each new contract, each shift in the adaptive system, each new action card creates a fresh puzzle to approach with accumulated knowledge and a refined deck.

---

## 🚫 Deferred Items

These are **intentionally out of scope** and will not be added:

* **Multiplayer** — not in scope for this game
* **Graphics/UI** — the game is a headless REST API; client development is separate
* **Story/narrative** — not in scope; the game is purely mechanical
* **Token lifecycle** — unlike the sibling card game, tokens have no lifecycle (aging, expiry, transformation); they are simple counters
* **Multiple resolution systems** — the game uses one unified production mechanic; no separate systems for different activity types

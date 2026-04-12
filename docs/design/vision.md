# 🎮 My Little Factory Manager — Vision

## 🧭 Core Concept

**My Little Factory Manager** is a deterministic, turn-based deckbuilding game where the player acts as a factory manager fulfilling contracts from an open market. Each contract defines a set of production constraints that must be satisfied using a limited set of tools drawn from a deck.

The game is built around a single unified production mechanic — contracts provide variety through their constraint combinations, not through fundamentally different resolution systems. This means every contract uses the same core interaction model, and strategic depth emerges from deckbuilding, resource management, and adaptation to shifting contract conditions.

---

## 🧠 Core One-Liner

> The limited hand size represents the manager's limited operational focus and execution capacity each turn — only a small subset of available tools can be actively deployed at once.

---

## ⚙️ Core Gameplay Loop

Each turn, the player:

* draws a **hand of tool cards** from their deck
* selects a subset to execute within limited operational capacity
* progresses toward fulfilling the active contract's requirements
* adapts their approach based on contract constraints and available tools

Tool cards represent operational actions such as production, transformation, quality control, stabilization, or system adjustments.

The **deck** represents all available factory capabilities. The **hand** represents what the manager can realistically execute in the current operational window.

---

## 🔧 Tool Cards

Tool cards are the player's primary interaction mechanism. Each card represents an operational action the manager can take during a production turn.

### Card Properties

Each tool card has:

* a **type tag** identifying its operational category
* a **numeric value** representing its contribution to production
* a **cost tier** (free, stamina, or health) determining its resource cost
* **effects** that interact with the current contract state

### Card Locations

Cards move between distinct locations during gameplay:

* **Library** — the complete catalogue of available tools
* **Deck** — the player's current operational toolset
* **Hand** — tools available for the current turn
* **Discard** — used tools awaiting recycling back into the deck

When the deck is empty and a draw is required, the entire discard pile is shuffled back into the deck.

### Cost Tiers

* **Free cards** — no resource cost; always playable but lower impact
* **Stamina cards** — moderate cost; good efficiency for prepared managers
* **Health cards** — high cost; powerful effects but risky when resources are low

The cost tier hierarchy ensures that more powerful tools come with meaningful tradeoffs. No card should ever be a pure trap — every card must have situations where playing it is the correct strategic choice.

---

## 📜 Contracts

Contracts are the primary source of gameplay challenge and the driver of all strategic decisions.

### Contract Definition

Each contract defines:

* **required output goals** — what must be produced
* **multiple simultaneous constraints** — rules that must be satisfied together
* **operational modifiers** — conditions that affect how production works during the contract

A contract is completed when **all constraints are satisfied simultaneously**.

### Contract Outcomes

Contracts are:

* **failable** — not every contract can be completed with every hand
* but always followed by **new opportunities** — the market never runs dry
* failure does not end progression, only **slows it**

### Constraint Types

Constraints create the strategic puzzle. Examples:

* **Output threshold** — produce at least N units
* **Resource budget** — complete within a stamina/health limit
* **Turn limit** — finish within N turns
* **Tool restrictions** — certain card types are unavailable or penalized
* **Quality requirements** — output must meet a minimum quality level
* **Sequencing rules** — certain operations must happen in a specific order

Higher-tier contracts combine multiple constraint types simultaneously, creating compound puzzles that require careful planning and deck composition.

---

## 🏗 Contract Tier System

Contracts are organized into tiers that represent increasing structural complexity:

### Tier Definitions

* **Tier 1** — simple, fast contracts introducing core systems. Typically one or two constraints. Designed to teach mechanics through play.
* **Tier 2** — increasing complexity. Two to three constraints that interact with each other. Requires basic planning.
* **Tier 3+** — deeply multi-constraint contracts requiring advanced planning, deck optimization, and strategic tool selection.

### Progression Rules

* Completing **10 contracts in a tier unlocks the next tier**
* Higher tiers introduce:
  * new constraint types not seen in lower tiers
  * more complex combinations of existing constraints
  * access to rarer and more specialized tool cards

Higher tiers do not just add difficulty — they increase the **structural complexity of contracts**, requiring qualitatively different strategic approaches.

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
* resource usage patterns

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
* **cards played** (total and per type)
* **strategy frequency** — how often specific card combinations are used
* **efficiency metrics** — cards used per contract completion, resources spent per output unit
* **streaks** — consecutive successful contracts without failure
* **specialization metrics** — dominant strategy types and diversity scores

Statistics serve both as player feedback (visible progression) and as input to the adaptive system.

---

## 🏆 Long-Term Motivation

Progression is driven by:

* **unlocking higher contract tiers** — accessing more complex and rewarding challenges
* **discovering new tools** — expanding the catalogue of available factory capabilities
* **optimizing execution efficiency** — completing contracts with fewer resources and turns
* **mastering adaptation** — thriving across shifting contract conditions and constraint combinations

The system supports both:

* **performance-based mastery** — speed, efficiency, consistency, high-tier completion rates
* **expression-based playstyles** — unique solution patterns, diverse strategy exploration

---

## 🎯 Core Design Goal

The game is built around a single principle:

> Success is not defined by building the strongest system, but by continuously adapting how your limited operational capacity is used under evolving contract constraints.

The factory is never "solved." Each new contract, each shift in the adaptive system, each new tool card creates a fresh puzzle to approach with accumulated knowledge and a refined deck.

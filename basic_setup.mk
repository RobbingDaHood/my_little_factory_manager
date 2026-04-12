This is a rust based project very similar to the project at ../my_little_card_game 

I want: 
1. The same tooling setup: Rust and rocket. 
1. The same data structure with a library of cards: playing hands etc. 
1. Having the vision in the repo at /docs/design/vision.md and a roadmap.md beside it. 
1. The same setup with rest interfaces and the same documentation setup. 
1. Teh github copilot cli instructions: All about when to create pull requests, how to make branches etc, how to handle all threads in a PR etc. 
    1. The balancing skills can be left for future steps in the roadmap, but keep a reference back. 

Now I just want to do it with a game idea that is a much simpler and should be easier to balance. You can see in here ~/.copilot/session-state/9a125145-6940-4047-b0a0-d3088a6e4711/research/could-you-keep-a-dialog-open-with-me-asking-me-que.md how difficult it were to balance and what ideas I had to fix that. 

Below is an initial overview of the idea. This first task is to create the vision and roadmap for this new game. The roadmap should start with some very simple steps: remember to mention in the roadmap the places in the my_little_card_game to get inspired from. The vision document should not mention that older game, but should be focused on describing the pure idea of the game. The roadmap should mention technologies used and that should not be precent in the vision: the same with balancing steps and how to implement them, add that to the roadmap. So the vision is the pure idea and the roadmap is concrete how to get there in easy to implement steps. 

One of the first steps in the roadmap is to setup this git project in a way where it is easy to give a token I will provide during setup that can be kept pushed. I am quite sure i setup gh with the correct token, but I think upstream will not use it proper: so make sure there are skills for that. 

Summary of what to create: 
1. Vision.md
1. Roadmap.md
1. The first couple of github copilot skills as mentioned above. 

Ask if you are in doubt. 

Here is the game idea: 

# 🎮 My Little Factory Manager — Vision Summary

## 🧭 Core Concept

**My Little Factory Manager** is a deterministic, turn-based deckbuilding game where the player acts as a factory manager fulfilling contracts from an open market. Each contract defines a set of production constraints that must be satisfied using a limited set of tools drawn from a deck.

---

## 🧠 Core One-Liner

> The limited hand size represents the manager’s limited operational focus and execution capacity each turn—only a small subset of available tools can be actively deployed at once.

---

## ⚙️ Core Gameplay Loop

Each turn, the player:

* draws a **hand of cards (tools)**
* selects a subset to execute within limited capacity
* progresses toward fulfilling an active contract
* adapts their system based on constraints and outcomes

Cards represent operational actions such as production, transformation, stabilization, or system adjustments.

The deck represents all available factory capabilities, while the hand represents what the manager can realistically execute in the current operational window.

---

## 📜 Contracts (Core Challenge System)

Contracts are the primary source of gameplay challenge.

Each contract defines:

* required output goals
* multiple simultaneous constraints
* operational rules that must be satisfied together

A contract is completed when all constraints are satisfied.

Contracts are:

* **failable**
* but always followed by new opportunities
* failure does not end progression, only slows it

---

## 🏗 Contract Tier System

Contracts are organized into tiers:

* **Tier 1:** simple, fast contracts introducing core systems
* **Tier 2+:** increasing complexity and constraint density
* **High tiers:** deeply multi-constraint contracts requiring advanced planning and synergy

Progression rules:

* Completing **10 contracts in a tier unlocks the next tier**
* Higher tiers introduce:

  * new constraint types
  * more complex combinations of existing constraints
  * access to rarer cards and mechanics

Higher tiers do not just add difficulty—they increase **structural complexity of contracts**.

---

## 🔁 Adaptive System (Balance Philosophy)

The game tracks player behavior continuously, including:

* cards played
* frequency of specific strategies
* contract outcomes
* efficiency of solutions

Based on this data:

* frequently used mechanics become less efficient over time within contracts
* previously underused mechanics gradually become more valuable

This creates a shifting strategic landscape where long-term optimization requires adaptation rather than repetition.

---

## 🧾 Player Discard System

Players can always discard a card for a **small baseline benefit**.

This ensures:

* no turn becomes completely unusable
* every decision has forward momentum
* suboptimal hands still allow partial progress

---

## 📊 Progression & Statistics System

The game tracks detailed global and per-run statistics, including:

* total contracts completed
* contracts failed
* completion rates per tier
* cards played (total and per type)
* frequency of specific strategies
* efficiency metrics (cards used per contract completion)
* streaks (successful contracts without failure)
* specialization metrics (dominant strategy types)

---

## 🏆 Long-Term Motivation

Progression is driven by:

* unlocking higher contract tiers
* discovering new tools and constraints
* optimizing execution efficiency
* mastering adaptation across shifting contract conditions

The system supports both:

* **performance-based mastery** (speed, efficiency, consistency)
* **expression-based playstyles** (unique solution patterns)

---

## 🎯 Core Design Goal

The game is built around a single principle:

> Success is not defined by building the strongest system, but by continuously adapting how your limited operational capacity is used under evolving contract constraints.

---


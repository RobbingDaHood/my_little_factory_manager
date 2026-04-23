//! New-player tutorial walkthrough endpoint.

use rocket::serde::json::Json;
use rocket::serde::Serialize;
use rocket_okapi::{openapi, JsonSchema};

/// A single step in the new-player tutorial walkthrough.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct TutorialStep {
    pub step: u32,
    pub title: String,
    pub description: String,
    pub endpoint: String,
    pub method: String,
    pub example_body: Option<String>,
    pub tips: Vec<String>,
}

/// Complete new-player tutorial for learning the game through the API.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct Tutorial {
    pub title: String,
    pub introduction: String,
    pub core_concepts: Vec<String>,
    pub steps: Vec<TutorialStep>,
    pub next_steps: Vec<String>,
}

fn build_tutorial() -> Tutorial {
    Tutorial {
        title: "My Little Factory Manager — New Player Tutorial".to_string(),
        introduction: "Welcome! You are a factory manager fulfilling contracts from an \
            open market. Each contract defines production requirements that must be satisfied \
            using player action cards drawn from your deck. The game is fully deterministic \
            — the same seed and actions always produce the same result."
            .to_string(),
        core_concepts: vec![
            "Cards cycle through locations: Library → Deck → Hand → Discard → (shuffle back to Deck).".to_string(),
            "Tokens are persistent resources (beneficial or harmful) that carry between contracts.".to_string(),
            "Contracts define production goals — fulfill all requirements to complete them and earn reward cards.".to_string(),
            "Your hand persists between contracts — it carries over from one contract to the next.".to_string(),
            "Seed + action log = save/load. The same seed and actions reproduce the exact same game state.".to_string(),
            "Use /actions/possible to see what you can do right now.".to_string(),
        ],
        steps: vec![
            TutorialStep {
                step: 1,
                title: "Start a New Game".to_string(),
                description: "Initialize the game with a seed for reproducible gameplay. \
                    If you omit the seed, a random one is generated."
                    .to_string(),
                endpoint: "/action".to_string(),
                method: "POST".to_string(),
                example_body: Some(r#"{"action_type": "NewGame", "seed": 42}"#.to_string()),
                tips: vec![
                    "Use the same seed to replay a game identically.".to_string(),
                    "The response includes the seed used.".to_string(),
                ],
            },
            TutorialStep {
                step: 2,
                title: "Check Your Starting State".to_string(),
                description: "View the full game state: your hand, deck sizes, token \
                    balances, and offered contracts."
                    .to_string(),
                endpoint: "/state".to_string(),
                method: "GET".to_string(),
                example_body: None,
                tips: vec![
                    "Your starting hand has 5 cards drawn from the starter deck.".to_string(),
                    "Token balances start empty — you produce tokens by playing cards.".to_string(),
                ],
            },
            TutorialStep {
                step: 3,
                title: "Check Available Actions".to_string(),
                description: "The /actions/possible endpoint tells you exactly what \
                    you can do right now. Use it whenever you're unsure of valid moves."
                    .to_string(),
                endpoint: "/actions/possible".to_string(),
                method: "GET".to_string(),
                example_body: None,
                tips: vec![
                    "Without an active contract, you'll see AcceptContract actions.".to_string(),
                    "With an active contract, you'll see PlayCard and DiscardCard actions.".to_string(),
                    "After 5 turns on a contract, AbandonContract also appears as an emergency escape.".to_string(),
                ],
            },
            TutorialStep {
                step: 4,
                title: "Browse the Contract Market".to_string(),
                description: "View the available contracts grouped by tier. Each contract \
                    shows its requirements and the reward card you'll earn upon completion."
                    .to_string(),
                endpoint: "/contracts/available".to_string(),
                method: "GET".to_string(),
                example_body: None,
                tips: vec![
                    "Tier 0 contracts require producing a certain number of ProductionUnits.".to_string(),
                    "The reward card preview shows exactly what card you'll get.".to_string(),
                    "3 contracts are offered per unlocked tier.".to_string(),
                ],
            },
            TutorialStep {
                step: 5,
                title: "Accept a Contract".to_string(),
                description: "Pick a contract from the market by specifying its tier index \
                    and contract index."
                    .to_string(),
                endpoint: "/action".to_string(),
                method: "POST".to_string(),
                example_body: Some(
                    r#"{"action_type": "AcceptContract", "tier_index": 0, "contract_index": 0}"#
                        .to_string(),
                ),
                tips: vec![
                    "You can only have one active contract at a time.".to_string(),
                    "Choose contracts whose requirements match your available cards.".to_string(),
                ],
            },
            TutorialStep {
                step: 6,
                title: "Play Cards to Produce Tokens".to_string(),
                description: "Play a card by its index in the /state cards Vec. The card \
                    must have hand > 0. Each card's effects add or remove tokens. A \
                    replacement card is drawn from the deck. Check /actions/possible for \
                    valid_card_indices — at higher tiers, some cards may be excluded due \
                    to CardTagConstraint limits."
                    .to_string(),
                endpoint: "/action".to_string(),
                method: "POST".to_string(),
                example_body: Some(
                    r#"{"action_type": "PlayCard", "card_index": 0}"#.to_string(),
                ),
                tips: vec![
                    "Playing a card applies all its effects (inputs consumed, outputs produced).".to_string(),
                    "After playing, a new card is drawn from the deck.".to_string(),
                    "The contract auto-completes as soon as all requirements are met.".to_string(),
                    "valid_card_indices in /actions/possible already filters out banned/over-limit cards.".to_string(),
                ],
            },
            TutorialStep {
                step: 7,
                title: "Discard for Progress".to_string(),
                description: "If your hand isn't ideal, you can discard any card for a \
                    small baseline production bonus (1 ProductionUnit). Pass the card_index \
                    (into the /state cards Vec) of a card with hand > 0."
                    .to_string(),
                endpoint: "/action".to_string(),
                method: "POST".to_string(),
                example_body: Some(
                    r#"{"action_type": "DiscardCard", "card_index": 0}"#.to_string(),
                ),
                tips: vec![
                    "Discarding prevents dead turns — every card has some value.".to_string(),
                    "The bonus is small, so playing cards is usually better.".to_string(),
                ],
            },
            TutorialStep {
                step: 8,
                title: "Contract Completion and Rewards".to_string(),
                description: "When all requirements are met, the contract auto-completes. \
                    The required tokens are subtracted, you earn the reward card, and the \
                    market refills. Reward cards always go to the shelf — they \
                    are owned but not in the active cycle until you use ReplaceCard to \
                    bring them in."
                    .to_string(),
                endpoint: "/state".to_string(),
                method: "GET".to_string(),
                example_body: None,
                tips: vec![
                    "Reward cards make your deck stronger over time.".to_string(),
                    "Completing 10 contracts in a tier unlocks the next tier.".to_string(),
                    "Use ReplaceCard between contracts to bring reward cards into your active cycle.".to_string(),
                ],
            },
            TutorialStep {
                step: 9,
                title: "Contract Failure".to_string(),
                description: "Some contracts have a turn-window constraint with optional \
                    lower and upper bounds. A deadline-only contract (max_turn set) fails if \
                    you exceed max_turn. An earliest-start contract (min_turn set) cannot \
                    complete before min_turn. A full window contract has both. \
                    At tier 12+, playing a banned tag (CardTagConstraint max=0) is blocked \
                    outright — the server rejects the action before it can cause failure."
                    .to_string(),
                endpoint: "/state".to_string(),
                method: "GET".to_string(),
                example_body: None,
                tips: vec![
                    "Check TokenRequirement max bounds before accepting — exceeding them fails the contract.".to_string(),
                    "TurnWindow max_turn is a hard deadline — exceeding it is an immediate failure.".to_string(),
                    "TurnWindow min_turn prevents rushing — the contract cannot complete before that turn.".to_string(),
                    "At tier 12+, check valid_card_indices in /actions/possible to avoid banned tag plays.".to_string(),
                    "Failure is not game-over — it just means no reward and a broken streak.".to_string(),
                    "After failure, the adaptive system eases difficulty, giving you breathing room.".to_string(),
                    "The contract_turns_played field in /state shows how many turns you've used.".to_string(),
                    "AbandonContract becomes available after 5 turns as a last resort — it counts as a failure.".to_string(),
                    "Use AbandonContract only if truly stuck (e.g., all cards banned); it breaks your streak.".to_string(),
                ],
            },
            TutorialStep {
                step: 10,
                title: "Adaptive Balance".to_string(),
                description: "The game adapts to your playstyle. Contracts you see in the \
                    market are adjusted based on your behavior: if you produce a lot of \
                    Heat, future contracts tighten Heat limits; if you stop producing \
                    something, its pressure relaxes over time."
                    .to_string(),
                endpoint: "/metrics".to_string(),
                method: "GET".to_string(),
                example_body: None,
                tips: vec![
                    "Check adaptive_adjustments on each contract to see how requirements were modified.".to_string(),
                    "The /metrics endpoint shows your current adaptive_pressure per token type.".to_string(),
                    "Failing a contract relaxes all pressure — the system is forgiving.".to_string(),
                    "Diversifying your strategy keeps pressure balanced across token types.".to_string(),
                ],
            },
            TutorialStep {
                step: 11,
                title: "Manage Your Deck (Deckbuilding)".to_string(),
                description: "Between contracts, you can use ReplaceCard to swap a card \
                    in your deck or discard (auto-selected: deck first, then discard) \
                    with a shelved card. The cost is permanently \
                    destroying a third card (sacrifice from shelved copies). This is the \
                    only way to change your active cycle composition."
                    .to_string(),
                endpoint: "/action".to_string(),
                method: "POST".to_string(),
                example_body: Some(
                    r#"{"action_type": "ReplaceCard", "target_card_index": 0, "replacement_card_index": 3, "sacrifice_card_index": 1}"#.to_string(),
                ),
                tips: vec![
                    "ReplaceCard is only available between contracts (no active contract).".to_string(),
                    "The replacement card must have copies on the shelf (shelved > 0).".to_string(),
                    "The sacrifice must also come from shelved copies — you cannot sacrifice active cards.".to_string(),
                    "You cannot sacrifice the same card you are replacing.".to_string(),
                    "The target location is auto-selected: deck first, then discard.".to_string(),
                    "Use /actions/possible to see valid ReplaceCard index ranges.".to_string(),
                    "Improve your deck quality by replacing weak starter cards with strong reward cards.".to_string(),
                ],
            },
            TutorialStep {
                step: 12,
                title: "Browse Your Card Catalogue".to_string(),
                description: "View all your cards and their location counts. Filter by \
                    tag to find specific card types."
                    .to_string(),
                endpoint: "/library/cards".to_string(),
                method: "GET".to_string(),
                example_body: None,
                tips: vec![
                    "Use ?tag=Production to see only production cards.".to_string(),
                    "Card counts show how many copies are in each location.".to_string(),
                    "shelved = copies on the shelf; deck+hand+discard = copies in the active cycle.".to_string(),
                ],
            },
            TutorialStep {
                step: 13,
                title: "Save and Replay".to_string(),
                description: "The action history endpoint returns every action taken. \
                    Combined with the seed from /state, replaying these actions on a \
                    fresh game produces the exact same state."
                    .to_string(),
                endpoint: "/actions/history".to_string(),
                method: "GET".to_string(),
                example_body: None,
                tips: vec![
                    "This is the save/load mechanism — no separate save file needed.".to_string(),
                    "Same version + same seed + same actions = identical game state.".to_string(),
                ],
            },
        ],
        next_steps: vec![
            "Keep completing contracts to unlock higher tiers with new challenges.".to_string(),
            "Check /metrics to track your gameplay statistics — completions, efficiency, and streaks.".to_string(),
            "Check /docs/hints for strategies and tips.".to_string(),
            "Check /docs/designer for how contracts, cards, and tokens work.".to_string(),
            "Explore /swagger/ for the complete interactive API documentation.".to_string(),
            "Use /player/tokens to monitor your resource balances.".to_string(),
        ],
    }
}

/// New-player tutorial that walks through a first game session.
///
/// Returns a structured walkthrough covering: starting a game, checking state,
/// browsing contracts, accepting contracts, playing cards, discarding, contract
/// completion, browsing the card catalogue, and saving/replaying.
/// Each step includes the endpoint to call, example request body, and tips.
#[openapi]
#[get("/docs/tutorial")]
pub fn get_tutorial() -> Json<Tutorial> {
    Json(build_tutorial())
}

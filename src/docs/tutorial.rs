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
                    "Tier 1 contracts require producing a certain number of ProductionUnits.".to_string(),
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
                description: "Play cards from your hand by index. Each card's effects \
                    add or remove tokens. A replacement card is drawn from the deck."
                    .to_string(),
                endpoint: "/action".to_string(),
                method: "POST".to_string(),
                example_body: Some(
                    r#"{"action_type": "PlayCard", "hand_index": 0}"#.to_string(),
                ),
                tips: vec![
                    "Playing a card applies all its effects (inputs consumed, outputs produced).".to_string(),
                    "After playing, a new card is drawn from the deck.".to_string(),
                    "The contract auto-completes as soon as all requirements are met.".to_string(),
                ],
            },
            TutorialStep {
                step: 7,
                title: "Discard for Progress".to_string(),
                description: "If your hand isn't ideal, you can discard any card for a \
                    small baseline production bonus (1 ProductionUnit)."
                    .to_string(),
                endpoint: "/action".to_string(),
                method: "POST".to_string(),
                example_body: Some(
                    r#"{"action_type": "DiscardCard", "hand_index": 0}"#.to_string(),
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
                    The required tokens are subtracted, you earn the reward card (added to \
                    your library and deck), and the market refills."
                    .to_string(),
                endpoint: "/state".to_string(),
                method: "GET".to_string(),
                example_body: None,
                tips: vec![
                    "Reward cards make your deck stronger over time.".to_string(),
                    "Completing 10 contracts in a tier unlocks the next tier.".to_string(),
                    "Check /contracts/active to verify no contract is active after completion.".to_string(),
                ],
            },
            TutorialStep {
                step: 9,
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
                    "library count = total owned; deck+hand+discard = current distribution.".to_string(),
                ],
            },
            TutorialStep {
                step: 10,
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

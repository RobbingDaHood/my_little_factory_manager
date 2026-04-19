//! Starter deck generation.
//!
//! Generates the starting deck by round-robin rolling cards from all
//! effect types unlocked at tier 0, using the game's seeded RNG.

use rand_pcg::Pcg64;

use crate::config_loader::load_token_definitions;
use crate::contract_generation::{generate_effect_types, roll_base_effect};
use crate::types::{add_card_to_entries, CardEntry, CardLocation, PlayerActionCard};

/// Build the starter deck by round-robin rolling `count` cards from all
/// effect types unlocked at tier 0.
///
/// Duplicate cards (same effects and tags) are grouped into a single
/// `CardEntry` with the appropriate copy count.
pub fn create_starter_deck(count: u32, rng: &mut Pcg64) -> Vec<CardEntry> {
    let token_defs = load_token_definitions().expect("embedded token definitions must parse");
    let effect_types = generate_effect_types(&token_defs);

    let available: Vec<_> = effect_types
        .iter()
        .filter(|et| et.available_at_tier == 0)
        .collect();

    assert!(
        !available.is_empty(),
        "at least one effect type must be unlocked at tier 0"
    );

    let mut entries: Vec<CardEntry> = Vec::new();

    for i in 0..count {
        let selected = available[i as usize % available.len()];

        let effect = roll_base_effect(0, selected, rng);

        let card = PlayerActionCard {
            tags: selected.tags.clone(),
            effects: vec![effect],
        };

        add_card_to_entries(&mut entries, &card, CardLocation::Deck);
    }

    entries
}

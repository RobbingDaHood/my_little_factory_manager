//! Starter deck generation.
//!
//! Generates the starting deck by round-robin rolling cards from all
//! effect types unlocked at tier ≤ 1, using the game's seeded RNG.

use rand_pcg::Pcg64;

use crate::config_loader::load_effect_types;
use crate::contract_generation::{parse_card_tag, roll_base_effect};
use crate::types::{add_card_to_entries, CardEntry, CardLocation, CardTag, PlayerActionCard};

/// Build the starter deck by round-robin rolling `count` cards from all
/// effect types unlocked at tier ≤ 1.
///
/// Duplicate cards (same effects and tags) are grouped into a single
/// `CardEntry` with the appropriate copy count.
pub fn create_starter_deck(count: u32, rng: &mut Pcg64) -> Vec<CardEntry> {
    let effect_types = load_effect_types().expect("embedded effect types must parse");

    let available: Vec<_> = effect_types
        .iter()
        .filter(|et| et.unlocked_at_tier <= 1)
        .collect();

    assert!(
        !available.is_empty(),
        "at least one effect type must be unlocked at tier 1"
    );

    let mut entries: Vec<CardEntry> = Vec::new();

    for i in 0..count {
        let selected = available[i as usize % available.len()];

        let effect = roll_base_effect(1, selected, rng);

        let tags: Vec<CardTag> = selected
            .tags
            .iter()
            .filter_map(|s| parse_card_tag(s))
            .collect();

        let card = PlayerActionCard {
            tags: if tags.is_empty() {
                vec![CardTag::Production]
            } else {
                tags
            },
            effects: vec![effect],
        };

        add_card_to_entries(&mut entries, &card, CardLocation::Deck);
    }

    entries
}

//! Tests for the starter deck definitions.

use my_little_factory_manager::starter_cards::create_starter_deck;
use my_little_factory_manager::types::{CardTag, TokenType};
use rand::SeedableRng;
use rand_pcg::Pcg64;
use std::collections::BTreeSet;

#[test]
fn starter_deck_has_correct_size() {
    let mut rng = Pcg64::seed_from_u64(42);
    let entries = create_starter_deck(50, &mut rng);
    let total_cards: u32 = entries.iter().map(|e| e.counts.deck).sum();
    assert_eq!(total_cards, 50, "starter deck should have 50 total cards");
}

#[test]
fn all_copies_start_in_deck() {
    let mut rng = Pcg64::seed_from_u64(42);
    let entries = create_starter_deck(50, &mut rng);
    for entry in &entries {
        assert_eq!(entry.counts.shelved, 0);
        assert_eq!(entry.counts.hand, 0);
        assert_eq!(entry.counts.discard, 0);
        assert!(entry.counts.deck > 0);
    }
}

#[test]
fn all_starter_cards_are_pure_production() {
    let mut rng = Pcg64::seed_from_u64(42);
    let entries = create_starter_deck(50, &mut rng);
    for entry in &entries {
        assert_eq!(
            entry.card.tags,
            vec![CardTag {
                input: BTreeSet::new(),
                output: BTreeSet::from([TokenType::ProductionUnit]),
            }]
        );
        assert_eq!(entry.card.effects.len(), 1);
        assert!(entry.card.effects[0].inputs.is_empty());
        assert_eq!(entry.card.effects[0].outputs.len(), 1);
        assert_eq!(
            entry.card.effects[0].outputs[0].token_type,
            TokenType::ProductionUnit
        );
    }
}

#[test]
fn starter_cards_production_amounts_in_tier0_range() {
    let mut rng = Pcg64::seed_from_u64(123);
    let entries = create_starter_deck(50, &mut rng);
    for entry in &entries {
        let amount = entry.card.effects[0].outputs[0].amount;
        // Tier 0 pure_production: base_min=1 + 0*per_tier_min=1 = 1,
        //                         base_max=5 + 0*per_tier_max=2 = 5
        assert!(
            (1..=5).contains(&amount),
            "production amount {amount} should be in [1, 5]"
        );
    }
}

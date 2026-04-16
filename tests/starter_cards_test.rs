//! Tests for the starter deck definitions.

use my_little_factory_manager::starter_cards::create_starter_deck;
use my_little_factory_manager::types::{CardTag, TokenType};

#[test]
fn starter_deck_has_correct_size() {
    let entries = create_starter_deck();
    let total_cards: u32 = entries.iter().map(|e| e.counts.library).sum();
    assert_eq!(total_cards, 10, "starter deck should have 10 total cards");
    assert_eq!(entries.len(), 3, "starter library should have 3 card types");
}

#[test]
fn all_copies_start_in_deck() {
    let entries = create_starter_deck();
    for entry in &entries {
        assert_eq!(entry.counts.deck, entry.counts.library);
        assert_eq!(entry.counts.hand, 0);
        assert_eq!(entry.counts.discard, 0);
    }
}

#[test]
fn all_starter_cards_are_pure_production() {
    let entries = create_starter_deck();
    for entry in &entries {
        assert_eq!(entry.card.tags, vec![CardTag::Production]);
        assert_eq!(entry.card.effects.len(), 1);
        assert!(entry.card.effects[0].inputs.is_empty());
        assert_eq!(entry.card.effects[0].outputs.len(), 1);
        assert_eq!(
            entry.card.effects[0].outputs[0].token_type,
            TokenType::ProductionUnit
        );
    }
}

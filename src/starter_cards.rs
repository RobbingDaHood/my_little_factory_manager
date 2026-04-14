//! Starter deck definitions for Phase 2.
//!
//! All starter cards are pure production (no inputs). They vary only
//! in the amount of ProductionUnit they produce.

use crate::types::{
    CardCounts, CardEffect, CardEntry, CardTag, PlayerActionCard, TokenAmount, TokenType,
};

/// A starter card definition: how many copies and the card template.
struct StarterCardDef {
    copies: u32,
    card: PlayerActionCard,
}

fn production_card(output_amount: u32) -> PlayerActionCard {
    PlayerActionCard {
        tags: vec![CardTag::Production],
        effects: vec![CardEffect::new(
            vec![],
            vec![TokenAmount {
                token_type: TokenType::ProductionUnit,
                amount: output_amount,
            }],
        )
        .expect("pure production effect is always valid")],
    }
}

fn starter_card_defs() -> Vec<StarterCardDef> {
    vec![
        StarterCardDef {
            copies: 4,
            card: production_card(1),
        },
        StarterCardDef {
            copies: 4,
            card: production_card(2),
        },
        StarterCardDef {
            copies: 2,
            card: production_card(3),
        },
    ]
}

/// Build the starter card library as a list of `CardEntry` values.
///
/// Each entry starts with all copies in `deck` (and `library` set to match).
pub fn create_starter_deck() -> Vec<CardEntry> {
    starter_card_defs()
        .into_iter()
        .map(|def| CardEntry {
            card: def.card,
            counts: CardCounts {
                library: def.copies,
                deck: def.copies,
                hand: 0,
                discard: 0,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

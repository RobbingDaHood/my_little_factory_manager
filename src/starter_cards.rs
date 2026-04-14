//! Starter deck definitions for Phase 2.
//!
//! All starter cards are pure production (no inputs). They vary only
//! in the amount of ProductionUnit they produce.

use crate::types::{CardEffect, CardTag, PlayerActionCard, TokenAmount, TokenType};

/// A starter card definition: how many copies and the card template.
struct StarterCardDef {
    copies: usize,
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

/// Build the starter card library and return (library, deck_indices).
///
/// The library contains one entry per unique card template.
/// The deck indices list references into the library, with the correct
/// number of copies for each template.
pub fn create_starter_deck() -> (Vec<PlayerActionCard>, Vec<usize>) {
    let defs = starter_card_defs();
    let mut library = Vec::new();
    let mut deck_indices = Vec::new();

    for def in defs {
        let library_index = library.len();
        library.push(def.card);
        for _ in 0..def.copies {
            deck_indices.push(library_index);
        }
    }

    (library, deck_indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_deck_has_correct_size() {
        let (library, deck) = create_starter_deck();
        assert_eq!(deck.len(), 10, "starter deck should have 10 cards");
        assert_eq!(library.len(), 3, "starter library should have 3 card types");
    }

    #[test]
    fn all_deck_indices_valid() {
        let (library, deck) = create_starter_deck();
        for &idx in &deck {
            assert!(idx < library.len(), "deck index {idx} out of bounds");
        }
    }

    #[test]
    fn all_starter_cards_are_pure_production() {
        let (library, _) = create_starter_deck();
        for card in &library {
            assert_eq!(card.tags, vec![CardTag::Production]);
            assert_eq!(card.effects.len(), 1);
            assert!(card.effects[0].inputs.is_empty());
            assert_eq!(card.effects[0].outputs.len(), 1);
            assert_eq!(
                card.effects[0].outputs[0].token_type,
                TokenType::ProductionUnit
            );
        }
    }
}

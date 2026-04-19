//! Starter deck generation.
//!
//! Generates the starting deck by rolling each card's production amount
//! from the tier 1 pure-production formula using the game's seeded RNG.

use rand::RngCore;
use rand_pcg::Pcg64;

use crate::config::TierScalingFormula;
use crate::config_loader::load_effect_types;
use crate::types::{
    CardCounts, CardEffect, CardEntry, CardTag, PlayerActionCard, TokenAmount, TokenType,
};

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

/// Roll a value from a tier-scaling formula for the given tier.
fn roll_from_formula(tier: u32, formula: &TierScalingFormula, rng: &mut Pcg64) -> u32 {
    let min = formula.base_min + tier * formula.per_tier_min;
    let max = formula.base_max + tier * formula.per_tier_max;
    if min >= max {
        return min;
    }
    let range = max - min + 1;
    min + (rng.next_u32() % range)
}

/// Build the starter deck by rolling `count` cards using the tier 1
/// pure-production output formula from `effect_types.json`.
///
/// Duplicate cards (same production amount) are grouped into a single
/// `CardEntry` with the appropriate copy count.
pub fn create_starter_deck(count: u32, rng: &mut Pcg64) -> Vec<CardEntry> {
    let effect_types = load_effect_types().expect("embedded effect types must parse");

    // Find the tier 1 pure-production output formula (first effect type
    // with min_tier <= 1 that has a ProductionUnit output).
    let output_formula = effect_types
        .iter()
        .filter(|et| et.min_tier <= 1)
        .flat_map(|et| et.outputs.iter())
        .find(|ef| ef.token_type == "ProductionUnit")
        .map(|ef| ef.formula.clone())
        .expect("tier 1 must have a ProductionUnit output formula");

    let mut entries: Vec<CardEntry> = Vec::new();

    for _ in 0..count {
        let amount = roll_from_formula(1, &output_formula, rng);
        let card = production_card(amount);

        if let Some(entry) = entries.iter_mut().find(|e| e.card == card) {
            entry.counts.shelved += 1;
            entry.counts.deck += 1;
        } else {
            entries.push(CardEntry {
                card,
                counts: CardCounts {
                    shelved: 1,
                    deck: 1,
                    hand: 0,
                    discard: 0,
                },
            });
        }
    }

    entries
}

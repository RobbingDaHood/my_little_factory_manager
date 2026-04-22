use my_little_factory_manager::config_loader::{load_game_rules, load_game_rules_from_json};
use my_little_factory_manager::types::{
    CardEffect, CardLocation, CardTag, Contract, ContractRequirementKind, ContractTier,
    PlayerActionCard, TokenAmount, TokenTag, TokenType,
};

// ---------------------------------------------------------------------------
// TokenType tests
// ---------------------------------------------------------------------------

#[test]
fn token_type_serialization_roundtrip() {
    let tokens = vec![
        TokenType::ProductionUnit,
        TokenType::Energy,
        TokenType::QualityPoint,
        TokenType::Innovation,
        TokenType::Heat,
        TokenType::Waste,
        TokenType::Pollution,
        TokenType::ContractsTierCompleted(1),
        TokenType::ContractsTierCompleted(5),
    ];
    for token in &tokens {
        let json = serde_json::to_string(token).expect("serialize TokenType");
        let roundtrip: TokenType = serde_json::from_str(&json).expect("deserialize TokenType");
        assert_eq!(token, &roundtrip, "roundtrip failed for {json}");
    }
}

#[test]
fn beneficial_tokens_have_beneficial_tag() {
    let beneficial = vec![
        TokenType::ProductionUnit,
        TokenType::Energy,
        TokenType::QualityPoint,
        TokenType::Innovation,
    ];
    for token in &beneficial {
        let tags = token.tags();
        assert!(
            tags.contains(&TokenTag::Beneficial),
            "{token:?} should have Beneficial tag"
        );
    }
}

#[test]
fn harmful_tokens_have_harmful_tag() {
    let harmful = vec![TokenType::Heat, TokenType::Waste, TokenType::Pollution];
    for token in &harmful {
        let tags = token.tags();
        assert!(
            tags.contains(&TokenTag::Harmful),
            "{token:?} should have Harmful tag"
        );
    }
}

#[test]
fn progression_tokens_have_progression_tag() {
    let progression = vec![
        TokenType::ContractsTierCompleted(1),
        TokenType::ContractsTierCompleted(2),
        TokenType::ContractsTierCompleted(3),
        TokenType::ContractsTierCompleted(10),
        TokenType::ContractsTierCompleted(100),
    ];
    for token in &progression {
        let tags = token.tags();
        assert!(
            tags.contains(&TokenTag::Progression),
            "{token:?} should have Progression tag"
        );
    }
}

#[test]
fn every_token_type_has_at_least_one_tag() {
    let all_tokens = vec![
        TokenType::ProductionUnit,
        TokenType::Energy,
        TokenType::QualityPoint,
        TokenType::Innovation,
        TokenType::Heat,
        TokenType::Waste,
        TokenType::Pollution,
        TokenType::ContractsTierCompleted(1),
        TokenType::ContractsTierCompleted(2),
        TokenType::ContractsTierCompleted(99),
    ];
    for token in &all_tokens {
        assert!(
            !token.tags().is_empty(),
            "{token:?} should have at least one tag"
        );
    }
}

// ---------------------------------------------------------------------------
// CardEffect tests
// ---------------------------------------------------------------------------

#[test]
fn card_effect_outputs_only_serialization_roundtrip() {
    let effect = CardEffect::new(
        vec![],
        vec![TokenAmount {
            token_type: TokenType::ProductionUnit,
            amount: 5,
        }],
    )
    .expect("valid effect");
    let json = serde_json::to_string(&effect).expect("serialize CardEffect");
    let roundtrip: CardEffect = serde_json::from_str(&json).expect("deserialize CardEffect");
    assert_eq!(effect, roundtrip);
}

#[test]
fn card_effect_inputs_and_outputs_serialization_roundtrip() {
    let effect = CardEffect::new(
        vec![TokenAmount {
            token_type: TokenType::Energy,
            amount: 2,
        }],
        vec![TokenAmount {
            token_type: TokenType::ProductionUnit,
            amount: 8,
        }],
    )
    .expect("valid effect");
    let json = serde_json::to_string(&effect).expect("serialize CardEffect");
    let roundtrip: CardEffect = serde_json::from_str(&json).expect("deserialize CardEffect");
    assert_eq!(effect, roundtrip);
}

#[test]
fn card_effect_inputs_only_serialization_roundtrip() {
    let effect = CardEffect::new(
        vec![TokenAmount {
            token_type: TokenType::Heat,
            amount: 3,
        }],
        vec![],
    )
    .expect("valid effect");
    let json = serde_json::to_string(&effect).expect("serialize CardEffect");
    let roundtrip: CardEffect = serde_json::from_str(&json).expect("deserialize CardEffect");
    assert_eq!(effect, roundtrip);
}

#[test]
fn card_effect_empty_inputs_and_outputs_rejected() {
    assert!(
        CardEffect::new(vec![], vec![]).is_err(),
        "CardEffect with no inputs and no outputs should be rejected"
    );
}

#[test]
fn card_effect_empty_json_deserialization_rejected() {
    let json = r#"{}"#;
    assert!(
        serde_json::from_str::<CardEffect>(json).is_err(),
        "Deserializing CardEffect with no inputs and no outputs should fail"
    );
}

// ---------------------------------------------------------------------------
// ContractRequirementKind tests
// ---------------------------------------------------------------------------

#[test]
fn token_requirement_min_serialization_roundtrip() {
    let req = ContractRequirementKind::TokenRequirement {
        token_type: TokenType::ProductionUnit,
        min: Some(20),
        max: None,
    };
    let json = serde_json::to_string(&req).expect("serialize TokenRequirement(min)");
    let roundtrip: ContractRequirementKind =
        serde_json::from_str(&json).expect("deserialize TokenRequirement(min)");
    assert_eq!(req, roundtrip);
}

#[test]
fn token_requirement_max_serialization_roundtrip() {
    let req = ContractRequirementKind::TokenRequirement {
        token_type: TokenType::Heat,
        min: None,
        max: Some(5),
    };
    let json = serde_json::to_string(&req).expect("serialize TokenRequirement(max)");
    let roundtrip: ContractRequirementKind =
        serde_json::from_str(&json).expect("deserialize TokenRequirement(max)");
    assert_eq!(req, roundtrip);
}

#[test]
fn card_tag_constraint_serialization_roundtrip() {
    let req = ContractRequirementKind::CardTagConstraint {
        tag: CardTag::Production,
        min: None,
        max: Some(0),
    };
    let json = serde_json::to_string(&req).expect("serialize CardTagConstraint");
    let roundtrip: ContractRequirementKind =
        serde_json::from_str(&json).expect("deserialize CardTagConstraint");
    assert_eq!(req, roundtrip);
}

#[test]
fn turn_window_serialization_roundtrip() {
    let variants = vec![
        ContractRequirementKind::TurnWindow {
            min_turn: None,
            max_turn: Some(8),
        },
        ContractRequirementKind::TurnWindow {
            min_turn: Some(3),
            max_turn: None,
        },
        ContractRequirementKind::TurnWindow {
            min_turn: Some(3),
            max_turn: Some(11),
        },
    ];
    for req in &variants {
        let json = serde_json::to_string(req).expect("serialize TurnWindow");
        let roundtrip: ContractRequirementKind =
            serde_json::from_str(&json).expect("deserialize TurnWindow");
        assert_eq!(req, &roundtrip);
    }
}

// ---------------------------------------------------------------------------
// ContractTier tests
// ---------------------------------------------------------------------------

#[test]
fn contract_tier_serialization_roundtrip() {
    let tiers = vec![
        ContractTier(1),
        ContractTier(2),
        ContractTier(3),
        ContractTier(10),
    ];
    for tier in &tiers {
        let json = serde_json::to_string(tier).expect("serialize ContractTier");
        let roundtrip: ContractTier =
            serde_json::from_str(&json).expect("deserialize ContractTier");
        assert_eq!(tier, &roundtrip, "roundtrip failed for {json}");
    }
}

// ---------------------------------------------------------------------------
// CardLocation tests
// ---------------------------------------------------------------------------

#[test]
fn card_location_serialization_roundtrip() {
    let locations = vec![
        CardLocation::Shelved,
        CardLocation::Deck,
        CardLocation::Hand,
        CardLocation::Discard,
    ];
    for loc in &locations {
        let json = serde_json::to_string(loc).expect("serialize CardLocation");
        let roundtrip: CardLocation =
            serde_json::from_str(&json).expect("deserialize CardLocation");
        assert_eq!(loc, &roundtrip, "roundtrip failed for {json}");
    }
}

// ---------------------------------------------------------------------------
// CardTag tests
// ---------------------------------------------------------------------------

#[test]
fn card_tag_serialization_roundtrip() {
    let tags = vec![
        CardTag::Production,
        CardTag::Transformation,
        CardTag::QualityControl,
        CardTag::SystemAdjustment,
    ];
    for tag in &tags {
        let json = serde_json::to_string(tag).expect("serialize CardTag");
        let roundtrip: CardTag = serde_json::from_str(&json).expect("deserialize CardTag");
        assert_eq!(tag, &roundtrip, "roundtrip failed for {json}");
    }
}

// ---------------------------------------------------------------------------
// Composite type tests
// ---------------------------------------------------------------------------

#[test]
fn player_action_card_serialization_roundtrip() {
    let card = PlayerActionCard {
        tags: vec![CardTag::Production, CardTag::Transformation],
        effects: vec![
            CardEffect::new(
                vec![],
                vec![TokenAmount {
                    token_type: TokenType::ProductionUnit,
                    amount: 3,
                }],
            )
            .expect("valid effect"),
            CardEffect::new(
                vec![TokenAmount {
                    token_type: TokenType::Energy,
                    amount: 1,
                }],
                vec![TokenAmount {
                    token_type: TokenType::ProductionUnit,
                    amount: 5,
                }],
            )
            .expect("valid effect"),
        ],
    };
    let json = serde_json::to_string(&card).expect("serialize PlayerActionCard");
    let roundtrip: PlayerActionCard =
        serde_json::from_str(&json).expect("deserialize PlayerActionCard");
    assert_eq!(card, roundtrip);
}

#[test]
fn contract_serialization_roundtrip() {
    let contract = Contract {
        tier: ContractTier(2),
        requirements: vec![
            ContractRequirementKind::TokenRequirement {
                token_type: TokenType::ProductionUnit,
                min: Some(15),
                max: None,
            },
            ContractRequirementKind::TokenRequirement {
                token_type: TokenType::Heat,
                min: None,
                max: Some(10),
            },
        ],
        reward_card: PlayerActionCard {
            tags: vec![CardTag::Production],
            effects: vec![CardEffect::new(
                vec![],
                vec![
                    TokenAmount {
                        token_type: TokenType::ProductionUnit,
                        amount: 8,
                    },
                    TokenAmount {
                        token_type: TokenType::Heat,
                        amount: 2,
                    },
                ],
            )
            .expect("valid effect")],
        },
        adaptive_adjustments: vec![],
    };
    let json = serde_json::to_string(&contract).expect("serialize Contract");
    let roundtrip: Contract = serde_json::from_str(&json).expect("deserialize Contract");
    assert_eq!(contract, roundtrip);
}

// ---------------------------------------------------------------------------
// Config loading tests
// ---------------------------------------------------------------------------

#[test]
fn load_embedded_game_rules() {
    let config = load_game_rules().expect("load embedded game rules");
    assert_eq!(config.general.starting_hand_size, 5);
    assert_eq!(config.general.starting_deck_size, 50);
    assert_eq!(config.general.contracts_per_tier_to_advance, 10);
    assert_eq!(config.general.contract_market_size_per_tier, 3);
    assert_eq!(config.general.discard_production_unit_bonus, 1);
}

#[test]
fn load_custom_game_rules_json() {
    let json = r#"{
        "general": {
            "starting_hand_size": 7,
            "starting_deck_size": 15,
            "contracts_per_tier_to_advance": 5,
            "contract_market_size_per_tier": 4,
            "discard_production_unit_bonus": 2,
            "min_turns_before_abandon": 8
        },
        "contract_formulas": {
            "output_threshold": {
                "base_min": 4,
                "base_max": 10,
                "per_tier_min": 1,
                "per_tier_max": 5
            },
            "harmful_token_limit": {
                "base_min": 5,
                "base_max": 15,
                "per_tier_min": 2,
                "per_tier_max": 4
            }
        },
        "adaptive_balance": {
            "alpha": 0.3,
            "decay_rate": 0.9,
            "failure_relaxation": 0.7,
            "max_tightening_pct": 0.30,
            "max_increase_pct": 0.20,
            "normalization_factor": 50.0
        }
    }"#;
    let config = load_game_rules_from_json(json).expect("parse custom game rules");
    assert_eq!(config.general.starting_hand_size, 7);
    assert_eq!(config.general.starting_deck_size, 15);
    assert_eq!(config.general.contracts_per_tier_to_advance, 5);
    assert_eq!(config.general.contract_market_size_per_tier, 4);
    assert_eq!(config.general.discard_production_unit_bonus, 2);
    assert_eq!(config.general.min_turns_before_abandon, 8);
}

#[test]
fn invalid_game_rules_json_returns_error() {
    let bad_json = r#"{ "general": { "starting_hand_size": "not a number" } }"#;
    assert!(
        load_game_rules_from_json(bad_json).is_err(),
        "invalid JSON should return an error"
    );
}

// ---------------------------------------------------------------------------
// TokenTag serialization test
// ---------------------------------------------------------------------------

#[test]
fn token_tag_serialization_roundtrip() {
    let tags = vec![
        TokenTag::Beneficial,
        TokenTag::Harmful,
        TokenTag::Progression,
    ];
    for tag in &tags {
        let json = serde_json::to_string(tag).expect("serialize TokenTag");
        let roundtrip: TokenTag = serde_json::from_str(&json).expect("deserialize TokenTag");
        assert_eq!(tag, &roundtrip, "roundtrip failed for {json}");
    }
}

// ---------------------------------------------------------------------------
// TokenAmount test
// ---------------------------------------------------------------------------

#[test]
fn token_amount_serialization_roundtrip() {
    let amount = TokenAmount {
        token_type: TokenType::Energy,
        amount: 42,
    };
    let json = serde_json::to_string(&amount).expect("serialize TokenAmount");
    let roundtrip: TokenAmount = serde_json::from_str(&json).expect("deserialize TokenAmount");
    assert_eq!(amount, roundtrip);
}

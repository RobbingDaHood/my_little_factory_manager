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
        TokenType::RawMaterial,
        TokenType::Heat,
        TokenType::CO2,
        TokenType::Waste,
        TokenType::Pollution,
        TokenType::ContractsTier1Completed,
        TokenType::ContractsTier5Completed,
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
        TokenType::RawMaterial,
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
    let harmful = vec![
        TokenType::Heat,
        TokenType::CO2,
        TokenType::Waste,
        TokenType::Pollution,
    ];
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
        TokenType::ContractsTier1Completed,
        TokenType::ContractsTier2Completed,
        TokenType::ContractsTier3Completed,
        TokenType::ContractsTier4Completed,
        TokenType::ContractsTier5Completed,
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
        TokenType::RawMaterial,
        TokenType::Heat,
        TokenType::CO2,
        TokenType::Waste,
        TokenType::Pollution,
        TokenType::ContractsTier1Completed,
        TokenType::ContractsTier2Completed,
        TokenType::ContractsTier3Completed,
        TokenType::ContractsTier4Completed,
        TokenType::ContractsTier5Completed,
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
fn pure_production_serialization_roundtrip() {
    let effect = CardEffect::PureProduction {
        outputs: vec![TokenAmount {
            token_type: TokenType::ProductionUnit,
            amount: 5,
        }],
    };
    let json = serde_json::to_string(&effect).expect("serialize PureProduction");
    let roundtrip: CardEffect = serde_json::from_str(&json).expect("deserialize PureProduction");
    assert_eq!(effect, roundtrip);
}

#[test]
fn conversion_serialization_roundtrip() {
    let effect = CardEffect::Conversion {
        inputs: vec![TokenAmount {
            token_type: TokenType::Energy,
            amount: 2,
        }],
        outputs: vec![TokenAmount {
            token_type: TokenType::ProductionUnit,
            amount: 8,
        }],
    };
    let json = serde_json::to_string(&effect).expect("serialize Conversion");
    let roundtrip: CardEffect = serde_json::from_str(&json).expect("deserialize Conversion");
    assert_eq!(effect, roundtrip);
}

#[test]
fn waste_removal_serialization_roundtrip() {
    let effect = CardEffect::WasteRemoval {
        inputs: vec![TokenAmount {
            token_type: TokenType::Heat,
            amount: 3,
        }],
    };
    let json = serde_json::to_string(&effect).expect("serialize WasteRemoval");
    let roundtrip: CardEffect = serde_json::from_str(&json).expect("deserialize WasteRemoval");
    assert_eq!(effect, roundtrip);
}

// ---------------------------------------------------------------------------
// ContractRequirementKind tests
// ---------------------------------------------------------------------------

#[test]
fn output_threshold_serialization_roundtrip() {
    let req = ContractRequirementKind::OutputThreshold {
        token_type: TokenType::ProductionUnit,
        min_amount: 20,
    };
    let json = serde_json::to_string(&req).expect("serialize OutputThreshold");
    let roundtrip: ContractRequirementKind =
        serde_json::from_str(&json).expect("deserialize OutputThreshold");
    assert_eq!(req, roundtrip);
}

#[test]
fn harmful_token_limit_serialization_roundtrip() {
    let req = ContractRequirementKind::HarmfulTokenLimit {
        token_type: TokenType::Heat,
        max_amount: 5,
    };
    let json = serde_json::to_string(&req).expect("serialize HarmfulTokenLimit");
    let roundtrip: ContractRequirementKind =
        serde_json::from_str(&json).expect("deserialize HarmfulTokenLimit");
    assert_eq!(req, roundtrip);
}

#[test]
fn card_tag_restriction_serialization_roundtrip() {
    let req = ContractRequirementKind::CardTagRestriction {
        restricted_tag: CardTag::Production,
    };
    let json = serde_json::to_string(&req).expect("serialize CardTagRestriction");
    let roundtrip: ContractRequirementKind =
        serde_json::from_str(&json).expect("deserialize CardTagRestriction");
    assert_eq!(req, roundtrip);
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
        CardLocation::Library,
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
            CardEffect::PureProduction {
                outputs: vec![TokenAmount {
                    token_type: TokenType::ProductionUnit,
                    amount: 3,
                }],
            },
            CardEffect::Conversion {
                inputs: vec![TokenAmount {
                    token_type: TokenType::Energy,
                    amount: 1,
                }],
                outputs: vec![TokenAmount {
                    token_type: TokenType::ProductionUnit,
                    amount: 5,
                }],
            },
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
            ContractRequirementKind::OutputThreshold {
                token_type: TokenType::ProductionUnit,
                min_amount: 15,
            },
            ContractRequirementKind::HarmfulTokenLimit {
                token_type: TokenType::CO2,
                max_amount: 10,
            },
        ],
        reward_card: PlayerActionCard {
            tags: vec![CardTag::Production],
            effects: vec![CardEffect::PureProduction {
                outputs: vec![
                    TokenAmount {
                        token_type: TokenType::ProductionUnit,
                        amount: 8,
                    },
                    TokenAmount {
                        token_type: TokenType::Heat,
                        amount: 2,
                    },
                ],
            }],
        },
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
    assert_eq!(config.general.starting_deck_size, 10);
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
            "discard_production_unit_bonus": 2
        }
    }"#;
    let config = load_game_rules_from_json(json).expect("parse custom game rules");
    assert_eq!(config.general.starting_hand_size, 7);
    assert_eq!(config.general.starting_deck_size, 15);
    assert_eq!(config.general.contracts_per_tier_to_advance, 5);
    assert_eq!(config.general.contract_market_size_per_tier, 4);
    assert_eq!(config.general.discard_production_unit_bonus, 2);
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

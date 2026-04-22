#!/bin/bash
# Example API walkthrough for My Little Factory Manager
# Start the server with `cargo run` before running these examples.
# Requires: curl, jq
#
# This script demonstrates a complete gameplay loop:
#   NewGame → view market → accept contract → play cards → complete → deckbuilding → tokens

set -euo pipefail
BASE_URL="http://localhost:8000"

echo "=== My Little Factory Manager — Full Gameplay Walkthrough ==="
echo

# ── Step 1: Read the tutorial ──────────────────────────────────────
echo "1. Read the new-player tutorial:"
curl -s "${BASE_URL}/docs/tutorial" | jq '.steps | length' | xargs -I{} echo "   Tutorial has {} steps"
echo

# ── Step 2: Start a new game (deterministic seed) ─────────────────
echo "2. Start a new game with seed 42:"
curl -s -X POST "${BASE_URL}/action" \
  -H "Content-Type: application/json" \
  -d '{"action_type": "NewGame", "seed": 42}' | jq '.'
echo

# ── Step 3: Check game state ─────────────────────────────────────
echo "3. Check initial game state (hand, tokens, contracts):"
curl -s "${BASE_URL}/state" | jq '{
  seed: .seed,
  hand_size: (.hand // [] | length),
  deck_size: .deck_size,
  discard_size: .discard_size,
  active_contract: .active_contract,
  offered_tiers: (.offered_contracts // [] | length)
}'
echo

# ── Step 4: See available actions ─────────────────────────────────
echo "4. Available actions:"
curl -s "${BASE_URL}/actions/possible" | jq '[.[] | .description]'
echo

# ── Step 5: View contract market ──────────────────────────────────
echo "5. Contract market:"
curl -s "${BASE_URL}/contracts/available" | jq '[ .[] | { tier: .tier, contracts: (.contracts | length) } ]'
echo

# ── Step 6: Accept the first contract ─────────────────────────────
echo "6. Accept the first tier-0 contract:"
curl -s -X POST "${BASE_URL}/action" \
  -H "Content-Type: application/json" \
  -d '{"action_type": "AcceptContract", "tier_index": 0, "contract_index": 0}' | jq '.'
echo

# ── Step 7: Check active contract ─────────────────────────────────
echo "7. Active contract:"
curl -s "${BASE_URL}/contracts/active" | jq '.'
echo

# ── Step 8: Check token balances ──────────────────────────────────
echo "8. Token balances (before playing cards):"
curl -s "${BASE_URL}/player/tokens" | jq '.'
echo

# ── Step 9: Play cards until contract completes ───────────────────
echo "9. Playing cards until contract resolves..."
for i in $(seq 1 20); do
  ACTIVE=$(curl -s "${BASE_URL}/contracts/active")

  if [ "$ACTIVE" = "null" ]; then
    echo "   Contract completed after ${i} card play(s)"
    break
  fi

  RESULT=$(curl -s -X POST "${BASE_URL}/action" \
    -H "Content-Type: application/json" \
    -d "{\"action_type\": \"PlayCard\", \"card_index\": $(curl -s "${BASE_URL}/actions/possible" | jq '[ .[] | select(.action_type == "PlayCard") ] | .[0].valid_card_indices[0] // 0')}")
  echo "   Play ${i}: $(echo "$RESULT" | jq -r '.result_type // .error_type // "unknown"')"
done
echo

# ── Step 10: Check tokens after contract ──────────────────────────
echo "10. Token balances after contract:"
curl -s "${BASE_URL}/player/tokens" | jq '.'
echo

# ── Step 10b: Deckbuilding — ReplaceCard (if available) ───────────
echo "10b. Check for ReplaceCard opportunities:"
POSSIBLE=$(curl -s "${BASE_URL}/actions/possible")
HAS_REPLACE=$(echo "$POSSIBLE" | jq '[ .[] | select(.action_type == "ReplaceCard") ] | length')
echo "   ReplaceCard available: $([ "$HAS_REPLACE" -gt 0 ] && echo 'yes' || echo 'no')"
if [ "$HAS_REPLACE" -gt 0 ]; then
  REPLACE_INFO=$(echo "$POSSIBLE" | jq -c '[ .[] | select(.action_type == "ReplaceCard") ] | .[0]')
  echo "   ReplaceCard ranges: $REPLACE_INFO"
  echo "   Example ReplaceCard (first valid indices):"
  TARGET=$(echo "$REPLACE_INFO" | jq '.valid_target_card_indices[0]')
  REPLACEMENT=$(echo "$REPLACE_INFO" | jq '.valid_replacement_card_indices[0]')
  SACRIFICE=$(echo "$REPLACE_INFO" | jq '.valid_sacrifice_card_indices[0]')
  curl -s -X POST "${BASE_URL}/action" \
    -H "Content-Type: application/json" \
    -d "{\"action_type\": \"ReplaceCard\", \"target_card_index\": $TARGET, \"replacement_card_index\": $REPLACEMENT, \"sacrifice_card_index\": $SACRIFICE}" | jq '.'
fi
echo

# ── Step 11: Browse card catalogue ────────────────────────────────
echo "11. Card catalogue (all cards):"
curl -s "${BASE_URL}/library/cards" | jq '[ .[] | { name: .card.name, tags: .card.tags } ]'
echo

echo "   Cards filtered by Production tag:"
curl -s "${BASE_URL}/library/cards?tag=Production" | jq '[ .[] | .card.name ]'
echo

# ── Step 12: View action history ──────────────────────────────────
echo "12. Action log (for deterministic replay):"
curl -s "${BASE_URL}/actions/history" | jq 'length' | xargs -I{} echo "   {} actions recorded"
echo

# ── Step 13: Check version ────────────────────────────────────────
echo "13. Server version:"
curl -s "${BASE_URL}/version" | jq '.'
echo

# ── Bonus: Documentation endpoints ───────────────────────────────
echo "=== Documentation Endpoints ==="
echo
echo "Tutorial steps:"
curl -s "${BASE_URL}/docs/tutorial" | jq '[ .steps[] | .title ]'
echo

echo "Hints — tier strategies covered:"
curl -s "${BASE_URL}/docs/hints" | jq '{ general_tips: (.general_tips | length), tiers: [ .tiers[] | .tier ] }'
echo

echo "Designer guide — sections:"
curl -s "${BASE_URL}/docs/designer" | jq '[ .sections[] | .title ]'
echo

# ── Step 14: Check gameplay metrics ───────────────────────────────
echo "14. Gameplay metrics:"
curl -s "${BASE_URL}/metrics" | jq '{
  contracts_completed: .total_contracts_completed,
  contracts_failed: .total_contracts_failed,
  cards_played: .total_cards_played,
  cards_discarded: .total_cards_discarded,
  avg_cards_per_contract: .avg_cards_per_contract,
  current_streak: .current_streak,
  best_streak: .best_streak,
  dominant_strategy: .dominant_strategy,
  diversity_score: .strategy_diversity_score,
  cards_replaced: .total_cards_replaced,
  adaptive_pressure: .adaptive_pressure
}'
echo

echo "=== Walkthrough complete ==="
echo "Run the server (cargo run) and try these commands interactively!"
echo "Interactive Swagger UI: ${BASE_URL}/swagger/"

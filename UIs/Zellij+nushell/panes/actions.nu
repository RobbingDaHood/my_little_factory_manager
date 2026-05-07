#!/usr/bin/env nu
# Possible-actions pane — shows what the API says is currently legal.

use ../lib/api.nu
use ../lib/state.nu
use ../lib/fmt.nu *

def render [] {
    clear
    let acts = (api possible-actions)
    if ($acts == null) {
        print "ACTIONS — server unreachable"
        return
    }
    print $"POSSIBLE ACTIONS  (date now | format date '%H:%M:%S')"
    print "─────────────────────────────────────────"
    if (($acts | length) == 0) {
        print "(none)"
        return
    }
    for a in $acts {
        match $a.action_type {
            "NewGame" => { print "  · new-game [seed]" }
            "PlayCard" => {
                let idxs = ($a.valid_card_indices | default [] | str join ", ")
                print $"  · play <idx>            indices: ($idxs)"
            }
            "DiscardCard" => {
                let idxs = ($a.valid_card_indices | default [] | str join ", ")
                print $"  · discard <idx>         indices: ($idxs)"
            }
            "AcceptContract" => {
                print "  · accept <tier> <idx>"
                for t in ($a.valid_tiers | default []) {
                    let r = $t.valid_contract_index_range
                    print $"      tier ($t.tier_index): contracts ($r.min)…($r.max)"
                }
            }
            "ReplaceCard" => { print "  · replace <target> <replacement> <sacrifice>" }
            "AbandonContract" => { print "  · abandon" }
            _ => { print $"  · ($a.action_type)" }
        }
    }
}

let s = (state load)
let interval = (($s.refresh_ms? | default 1000) * 1ms)
loop {
    render
    sleep $interval
}

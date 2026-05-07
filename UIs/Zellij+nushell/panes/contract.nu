#!/usr/bin/env nu
# Contract pane — when a contract is active, shows its requirements,
# the reward card, and turn count. Otherwise lists offered contracts grouped
# by tier, filtered by the `contracts_filter` substring set from the command
# pane (`filter-contracts <text>`).

use ../lib/api.nu
use ../lib/state.nu
use ../lib/fmt.nu *

def render-active [c: record, turns: int] {
    print $"ACTIVE CONTRACT  tier ($c.tier)  turns played: ($turns)"
    print "─────────────────────────────────────────"
    print "Requirements:"
    $c.requirements | enumerate | each {|r|
        $"  - [($r.index)] (fmt-requirement $r.item)"
    } | str join "\n" | print
    print "\nReward card:"
    print $"  (fmt-card $c.reward_card)"

    let adj = ($c.adaptive_adjustments? | default [])
    if (($adj | length) > 0) {
        print "\nAdaptive adjustments:"
        $adj | each {|a|
            $"  req#($a.requirement_index): ($a.original_value) → ($a.adjusted_value) ((($a.adjustment_percent))%)"
        } | str join "\n" | print
    }
}

def render-offers [tiers: list, filter: string] {
    print $"OFFERED CONTRACTS  filter=\"($filter)\""
    print "─────────────────────────────────────────"
    if (($tiers | length) == 0) {
        print "(no contracts offered)"
        return
    }
    for t in $tiers {
        print $"\n· tier ($t.tier)"
        $t.contracts | enumerate | each {|c|
            let summary = ($c.item.requirements | each {|r| fmt-requirement $r } | str join " ; ")
            let reward = (fmt-card $c.item.reward_card)
            let line = $"  [t($t.tier).c($c.index)]  reqs: ($summary)  | reward: ($reward)"
            if ($filter | is-empty) or ($line | str downcase | str contains ($filter | str downcase)) {
                $line
            } else { null }
        } | where { |x| $x != null } | str join "\n" | print
    }
    print "\nAccept with: `accept <tier_index> <contract_index>` from the command pane."
}

def render [] {
    clear
    let st = (api state)
    if ($st == null) {
        print "CONTRACT — server unreachable"
        return
    }
    print $"(date now | format date '%H:%M:%S')"
    let active = $st.active_contract
    if ($active != null) {
        render-active $active ($st.contract_turns_played | default 0)
    } else {
        let s = (state load)
        render-offers ($st.offered_contracts | default []) ($s.contracts_filter | default "")
    }
}

let s = (state load)
let interval = (($s.refresh_ms? | default 1000) * 1ms)
loop {
    render
    sleep $interval
}

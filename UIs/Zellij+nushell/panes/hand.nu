#!/usr/bin/env nu
# Hand pane — shows cards currently in hand with their original card-index
# (the index used by `play N` / `discard N`).

use ../lib/api.nu
use ../lib/state.nu
use ../lib/fmt.nu *

def render [] {
    clear
    let st = (api state)
    if ($st == null) {
        print "HAND — server unreachable"
        return
    }
    print $"HAND  (date now | format date '%H:%M:%S')"
    print "─────────────────────────────────────────"

    let in_hand = (
        $st.cards
        | enumerate
        | where { |row| ($row.item.counts.hand? | default 0) > 0 }
    )

    if (($in_hand | length) == 0) {
        print "(empty)"
        return
    }

    $in_hand | each {|row|
        let idx = $row.index
        let cnt = $row.item.counts.hand
        let line = (fmt-card $row.item.card)
        $"  [($idx)] x($cnt)  ($line)"
    } | str join "\n" | print

    print "\nTip: `play N` or `discard N` from the command pane."
}

let s = (state load)
let interval = (($s.refresh_ms? | default 1000) * 1ms)
loop {
    render
    sleep $interval
}

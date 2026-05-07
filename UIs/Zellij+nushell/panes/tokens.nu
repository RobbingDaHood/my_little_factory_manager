#!/usr/bin/env nu
# Tokens pane — refreshes the player's token balances every refresh tick.

use ../lib/api.nu
use ../lib/state.nu
use ../lib/fmt.nu *

def render [] {
    clear
    let v = (api version)
    if ($v == null) {
        print $"TOKENS — server unreachable at (api base-url)"
        return
    }
    let t = (api tokens)
    if ($t == null) {
        print "TOKENS — fetch failed"
        return
    }
    print $"TOKENS  (date now | format date '%H:%M:%S')"
    print "─────────────────────────────────────────"

    let cats = [
        [label rows];
        [beneficial  ($t.beneficial  | default [])]
        [harmful     ($t.harmful     | default [])]
        [progression ($t.progression | default [])]
    ]
    for c in $cats {
        print $"\n· ($c.label)"
        if (($c.rows | length) == 0) {
            print "    (none)"
        } else {
            $c.rows | each {|r|
                $"    (fmt-token-type $r.token_type) = ($r.amount)"
            } | str join "\n" | print
        }
    }
}

let s = (state load)
let interval = (($s.refresh_ms? | default 1000) * 1ms)
loop {
    render
    sleep $interval
}

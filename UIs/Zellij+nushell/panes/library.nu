#!/usr/bin/env nu
# Library pane — lists every card in the catalogue with its location counts.
# Filterable via `filter-library <substring>` from the command pane.

use ../lib/api.nu
use ../lib/state.nu
use ../lib/fmt.nu *

def render [] {
    clear
    let cards = (api library)
    if ($cards == null) {
        print "LIBRARY — server unreachable"
        return
    }
    let s = (state load)
    let f = ($s.library_filter | default "" | str downcase)

    print $"LIBRARY  filter=\"($f)\"  total=(($cards | length))"
    print "─────────────────────────────────────────"
    if (($cards | length) == 0) {
        print "(empty)"
        return
    }

    let rows = (
        $cards
        | enumerate
        | each {|row|
            let c = $row.item
            let line = (fmt-card $c.card)
            let counts = $"S:($c.counts.shelved) D:($c.counts.deck) H:($c.counts.hand) X:($c.counts.discard)"
            { idx: $row.index, counts: $counts, card: $line }
        }
        | where { |r|
            ($f | is-empty) or (($r.card | str downcase) | str contains $f)
        }
    )

    if (($rows | length) == 0) {
        print "(no matches)"
        return
    }

    $rows | each {|r|
        $"  [($r.idx | fill --alignment right --width 3)]  ($r.counts)  ($r.card)"
    } | str join "\n" | print

    print "\nLegend: S=shelved D=deck H=hand X=discard.  Use `filter-library <text>` or `clear-filters`."
}

let s = (state load)
let interval = (($s.refresh_ms? | default 1000) * 1ms)
loop {
    render
    sleep $interval
}

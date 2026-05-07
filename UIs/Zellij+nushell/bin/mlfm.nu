# Player command module — exported commands that the bottom thin pane
# uses to drive the game and adjust the other panes.
#
# Loaded automatically by the bottom pane's nu config (see ../config.nu).
# Every command prints a one-line summary so the thin pane stays readable.

use ../lib/api.nu
use ../lib/state.nu
use ../lib/fmt.nu *

# --- Helpers ------------------------------------------------------------

def ok [msg: string] { print $"✓ ($msg)" }
def err [msg: string] { print $"✗ ($msg)" }

def show-result [r: any] {
    if ($r == null) {
        err "no response (server down?)"
        return
    }
    if (($r | describe | str starts-with "record") and ($r | get -o error) != null) {
        err $r.error
        return
    }
    let outcome = ($r.outcome? | default "?")
    let kind = ($r.detail?.result_type? | default ($r.detail?.error_type? | default ($r.result_type? | default "?")))
    print $"→ ($outcome) · ($kind)"
    let extra = ($r.detail? | default $r)
    let trimmed = ($extra | reject -o result_type error_type)
    if (($trimmed | columns | length) > 0) { $trimmed | to nuon | print }
}

# --- Player actions -----------------------------------------------------

# Start a new game. Optional seed for deterministic replays.
export def "new-game" [seed?: int] {
    let r = (api new-game $seed)
    show-result $r
}

# Accept the contract at (tier_index, contract_index) from the offered list.
export def "accept" [tier: int, idx: int] {
    let r = (api accept-contract $tier $idx)
    show-result $r
}

# Play card from the global card list by its index.
export def "play" [card_index: int] {
    let r = (api play-card $card_index)
    show-result $r
}

# Discard a card from hand by its index.
export def "discard" [card_index: int] {
    let r = (api discard-card $card_index)
    show-result $r
}

# Replace a card in the active cycle. Provide three card indices.
export def "replace" [target: int, replacement: int, sacrifice: int] {
    let r = (api replace-card $target $replacement $sacrifice)
    show-result $r
}

# Abandon the active contract (counts as a failure).
export def "abandon" [] {
    let r = (api abandon-contract)
    show-result $r
}

# --- Inspect (one-shot) -------------------------------------------------

export def "show state" [] { api state | to nuon | print }
export def "show tokens" [] { api tokens | to nuon | print }
export def "show hand" [] {
    let st = (api state)
    if ($st == null) { err "server unreachable"; return }
    $st.cards
    | enumerate
    | where { |row| ($row.item.counts.hand? | default 0) > 0 }
    | each {|row| $"[($row.index)] x($row.item.counts.hand)  (fmt-card $row.item.card)" }
    | str join "\n"
    | print
}
export def "show contract" [] { api active-contract | to nuon | print }
export def "show contracts" [] { api available-contracts | to nuon | print }
export def "show library" [tag?: string] { api library $tag | to nuon | print }
export def "show actions" [] { api possible-actions | to nuon | print }
export def "show metrics" [] { api metrics | to nuon | print }
export def "show history" [] { api history | to nuon | print }

# --- Pane filters -------------------------------------------------------

# Filter the library pane to entries whose rendered line contains <text>.
export def "filter-library" [...text: string] {
    let s = ($text | str join " ")
    state set-field "library_filter" $s | ignore
    ok $"library filter = \"($s)\""
}

# Filter the offered-contracts pane similarly.
export def "filter-contracts" [...text: string] {
    let s = ($text | str join " ")
    state set-field "contracts_filter" $s | ignore
    ok $"contracts filter = \"($s)\""
}

# Clear all pane filters.
export def "clear-filters" [] {
    state set-field "library_filter" "" | ignore
    state set-field "contracts_filter" "" | ignore
    ok "filters cleared"
}

# Set how often the panes refresh, in milliseconds.
export def "refresh" [ms: int] {
    state set-field "refresh_ms" $ms | ignore
    ok $"refresh = ($ms) ms (panes pick this up on their next tick)"
}

# --- Help ---------------------------------------------------------------

export def "help-mlfm" [] {
    print "Player actions:"
    print "  new-game [seed]                       start a new game"
    print "  accept <tier> <idx>                   accept an offered contract"
    print "  play <card_index>                     play card by global index"
    print "  discard <card_index>                  discard card by global index"
    print "  replace <target> <repl> <sacrifice>   replace a card in the cycle"
    print "  abandon                               abandon the active contract"
    print ""
    print "Inspect:"
    print "  show state | tokens | hand | contract | contracts |"
    print "       library [tag-json] | actions | metrics | history"
    print ""
    print "Pane controls:"
    print "  filter-library <text…>"
    print "  filter-contracts <text…>"
    print "  clear-filters"
    print "  refresh <ms>"
    print ""
    print "Server URL: env MLFM_BASE_URL (default http://localhost:8000)"
}

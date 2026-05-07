# API helper module — wraps the My Little Factory Manager HTTP endpoints.
#
# All commands return parsed records / lists. The base URL can be overridden
# with the MLFM_BASE_URL environment variable (default: http://localhost:8000).

export def base-url [] {
    $env.MLFM_BASE_URL? | default "http://localhost:8000"
}

def get [path: string] {
    let url = $"(base-url)($path)"
    try { http get --max-time 2sec $url } catch { null }
}

def post [path: string, body: any] {
    let url = $"(base-url)($path)"
    try { http post --max-time 2sec --content-type application/json $url $body } catch { |e|
        { error: $e.msg }
    }
}

# --- GETs ---------------------------------------------------------------

export def state [] { get "/state" }
export def tokens [] { get "/player/tokens" }
export def active-contract [] { get "/contracts/active" }
export def available-contracts [] { get "/contracts/available" }
export def possible-actions [] { get "/actions/possible" }
export def library [tag?: string] {
    if ($tag | is-empty) {
        get "/library/cards"
    } else {
        let q = ($tag | url encode)
        get $"/library/cards?tag=($q)"
    }
}
export def history [] { get "/actions/history" }
export def metrics [] { get "/metrics" }
export def version [] { get "/version" }

# --- POST /action variants ---------------------------------------------

export def new-game [seed?: int] {
    let body = if ($seed == null) {
        { action_type: "NewGame", seed: null }
    } else {
        { action_type: "NewGame", seed: $seed }
    }
    post "/action" $body
}

export def accept-contract [tier_index: int, contract_index: int] {
    post "/action" {
        action_type: "AcceptContract",
        tier_index: $tier_index,
        contract_index: $contract_index,
    }
}

export def play-card [card_index: int] {
    post "/action" { action_type: "PlayCard", card_index: $card_index }
}

export def discard-card [card_index: int] {
    post "/action" { action_type: "DiscardCard", card_index: $card_index }
}

export def replace-card [target: int, replacement: int, sacrifice: int] {
    post "/action" {
        action_type: "ReplaceCard",
        target_card_index: $target,
        replacement_card_index: $replacement,
        sacrifice_card_index: $sacrifice,
    }
}

export def abandon-contract [] {
    post "/action" { action_type: "AbandonContract" }
}

# --- Health -------------------------------------------------------------

export def alive [] {
    let v = (version)
    $v != null
}

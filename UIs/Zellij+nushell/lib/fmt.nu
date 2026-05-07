# Pretty-printers for game data structures returned by the HTTP API.

# Render a CardTag {input:[…], output:[…]} as "in→out"
export def fmt-tag [tag: record] {
    let ins = ($tag.input | default [] | str join ",")
    let outs = ($tag.output | default [] | str join ",")
    let l = (if ($ins | is-empty) { "·" } else { $ins })
    let r = (if ($outs | is-empty) { "·" } else { $outs })
    $"($l)→($r)"
}

# Render a list of CardEffects as "2A,1B → 3C"
export def fmt-effect [eff: record] {
    let ins = ($eff.inputs | default [] | each {|t| $"($t.amount)·($t.token_type)" } | str join "+")
    let outs = ($eff.outputs | default [] | each {|t| $"($t.amount)·($t.token_type)" } | str join "+")
    let l = (if ($ins | is-empty) { "·" } else { $ins })
    let r = (if ($outs | is-empty) { "·" } else { $outs })
    $"($l) → ($r)"
}

export def fmt-card [card: record] {
    let tags = ($card.tags | default [] | each {|t| fmt-tag $t } | str join " | ")
    let eff = ($card.effects | default [] | each {|e| fmt-effect $e } | str join " ; ")
    $"[($tags)]  ($eff)"
}

# Render a TokenType, which is either a plain string ("ProductionUnit") or
# a tagged record like {ContractsTierCompleted: 3}. Returns a short display.
export def fmt-token-type [t: any] {
    let type = ($t | describe)
    if ($type | str starts-with "record") {
        let cols = ($t | columns)
        let key = ($cols | first)
        let val = ($t | get $key)
        $"($key)\(($val)\)"
    } else {
        $t
    }
}

export def fmt-requirement [r: record] {
    let kind = ($r.requirement_type? | default "?")
    match $kind {
        "TokenRequirement" => {
            let lo = ($r.min? | default "")
            let hi = ($r.max? | default "")
            let bounds = (
                if ($lo | is-empty) and ($hi | is-empty) { "" }
                else if ($lo | is-empty) { $"≤ ($hi)" }
                else if ($hi | is-empty) { $"≥ ($lo)" }
                else { $"($lo)…($hi)" }
            )
            $"token (fmt-token-type $r.token_type) ($bounds)"
        }
        "CardTagConstraint" => {
            let lo = ($r.min? | default "")
            let hi = ($r.max? | default "")
            let bounds = (
                if ($lo | is-empty) and ($hi | is-empty) { "" }
                else if ($lo | is-empty) { $"≤ ($hi)" }
                else if ($hi | is-empty) { $"≥ ($lo)" }
                else { $"($lo)…($hi)" }
            )
            $"play-tag (fmt-tag $r.tag) ($bounds)"
        }
        "TurnWindow" => {
            let lo = ($r.min_turn? | default "")
            let hi = ($r.max_turn? | default "")
            let bounds = (
                if ($lo | is-empty) and ($hi | is-empty) { "" }
                else if ($lo | is-empty) { $"by turn ($hi)" }
                else if ($hi | is-empty) { $"after turn ($lo)" }
                else { $"turns ($lo)…($hi)" }
            )
            $"window ($bounds)"
        }
        _ => ($r | to nuon)
    }
}

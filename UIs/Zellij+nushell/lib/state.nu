# Shared UI state — written by the bottom command pane, read by the
# refresh loops in the other panes. Plain JSON file in ./state/ui.json
# (relative to the Zellij+nushell directory).

export def state-file [] {
    let dir = ($env.MLFM_UI_DIR? | default $env.PWD)
    $"($dir)/state/ui.json"
}

# Default record shape persisted to disk.
def default-state [] {
    {
        library_filter: "",
        contracts_filter: "",
        refresh_ms: 1000,
    }
}

export def load [] {
    let f = (state-file)
    if ($f | path exists) {
        try { open $f } catch { default-state }
    } else {
        default-state
    }
}

export def save [s: record] {
    let f = (state-file)
    mkdir ($f | path dirname)
    $s | to json | save -f $f
}

# Update one field and persist. `field` is a string like "library_filter".
export def set-field [field: string, value: any] {
    let cur = (load)
    let merged = ($cur | upsert $field $value)
    save $merged
    $merged
}

export def reset [] {
    save (default-state)
    default-state
}

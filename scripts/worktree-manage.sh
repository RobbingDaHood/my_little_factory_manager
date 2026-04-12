#!/usr/bin/env bash
set -euo pipefail

# Worktree management script for parallel AI development.
# Worktrees are created as sibling directories under ../my_little_factory_managers/
# relative to the main repo checkout.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Resolve the main repo root via git's common dir (works from any worktree
# and regardless of the caller's working directory).
REPO_DIR="$(cd "$SCRIPT_DIR" && cd "$(git rev-parse --git-common-dir)/.." && pwd)"
WORKTREES_DIR="$(cd "$REPO_DIR/.." && pwd)/my_little_factory_managers"

usage() {
    cat <<EOF
Usage: $(basename "$0") <command> [args]

Commands:
  list                List all worktrees
  add <name>          Create a new worktree at ../my_little_factory_managers/<name>
                      on branch worktree/<name> from latest origin/main
  remove <name>       Remove a worktree and delete its branch
  reset <name>        Hard-reset worktree branch to latest origin/main

Examples:
  $(basename "$0") add feature-xyz
  $(basename "$0") reset feature-xyz
  $(basename "$0") remove feature-xyz
EOF
}

ensure_name() {
    if [[ -z "${1:-}" ]]; then
        echo "Error: worktree name is required." >&2
        usage >&2
        exit 1
    fi
}

cmd_list() {
    git -C "$REPO_DIR" worktree list
}

cmd_add() {
    ensure_name "${1:-}"
    local name="$1"
    local wt_path="$WORKTREES_DIR/$name"
    local branch="worktree/$name"

    if [[ -d "$wt_path" ]]; then
        echo "Error: worktree directory already exists: $wt_path" >&2
        exit 1
    fi

    echo "Fetching latest from origin..."
    git -C "$REPO_DIR" fetch origin

    mkdir -p "$WORKTREES_DIR"
    echo "Creating worktree '$name' at $wt_path (branch: $branch) from origin/main..."
    git -C "$REPO_DIR" worktree add "$wt_path" -b "$branch" origin/main

    echo "Done. Worktree ready at: $wt_path"
    echo "To publish the branch and set upstream, run:"
    echo "  git -C \"$wt_path\" push -u origin \"$branch\""
}

cmd_remove() {
    ensure_name "${1:-}"
    local name="$1"
    local wt_path="$WORKTREES_DIR/$name"
    local branch="worktree/$name"

    if [[ ! -d "$wt_path" ]]; then
        echo "Error: worktree directory does not exist: $wt_path" >&2
        exit 1
    fi

    echo "Removing worktree '$name'..."
    git -C "$REPO_DIR" worktree remove "$wt_path"
    git -C "$REPO_DIR" worktree prune

    if git -C "$REPO_DIR" rev-parse --verify "$branch" &>/dev/null; then
        echo "Deleting branch '$branch'..."
        git -C "$REPO_DIR" branch -D "$branch"
    fi

    echo "Done. Worktree '$name' removed."
}

cmd_reset() {
    ensure_name "${1:-}"
    local name="$1"
    local wt_path="$WORKTREES_DIR/$name"
    local branch="worktree/$name"

    if [[ ! -d "$wt_path" ]]; then
        echo "Error: worktree directory does not exist: $wt_path" >&2
        exit 1
    fi

    echo "Fetching latest from origin..."
    git -C "$REPO_DIR" fetch origin

    echo "Resetting worktree '$name' to origin/main..."
    git -C "$wt_path" checkout "$branch"
    git -C "$wt_path" reset --hard origin/main
    git -C "$wt_path" clean -fd

    echo "Done. Worktree '$name' reset to origin/main."
}

case "${1:-}" in
    list)   cmd_list ;;
    add)    cmd_add "${2:-}" ;;
    remove) cmd_remove "${2:-}" ;;
    reset)  cmd_reset "${2:-}" ;;
    *)      usage ;;
esac

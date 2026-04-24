All work must be done in a dedicated worktree under `my_little_factory_managers/`. Never commit, build, test, or modify files directly in the main `my_little_factory_manager/` checkout. Multiple agents and manual work can run in parallel; working in the main repo causes conflicts and data loss.

## Starting new work

1. Confirm the current working directory is inside `my_little_factory_managers/`, not `my_little_factory_manager/`. If you are in the main repo, stop and create a worktree first.
2. Create a worktree from the latest `origin/main`:
   ```bash
   scripts/worktree-manage.sh add <descriptive-name>
   ```
   The worktree folder name should describe the work being done.
3. Use the `EnterWorktree` tool to switch Claude Code context into the new worktree directory.
4. All edits, builds, tests, and commits happen inside `my_little_factory_managers/<descriptive-name>/`.
5. Ask the user whether to create a pull request when the work is done.

## Continuing existing work

Use an existing worktree already on the target branch, or create a new one pointing at it. Always start with:

```bash
git fetch origin && git rebase origin/main
```

## Branching rules

- Always base new branches on the latest remote `origin/main` (handled automatically by `worktree-manage.sh add`).
- Commit small, isolated commits — each must pass `make check`.
- Rebase on `origin/main` before pushing: `git fetch origin && git rebase origin/main`.

## Worktree management

```bash
scripts/worktree-manage.sh list           # list all worktrees
scripts/worktree-manage.sh add <name>     # create worktree from latest origin/main
scripts/worktree-manage.sh remove <name>  # remove worktree and delete its branch
scripts/worktree-manage.sh reset <name>   # hard-reset to latest origin/main
```

Remove worktrees after work is merged to keep the workspace clean.

## Worktree layout

```
Projects/
  my_little_factory_manager/     ← main repo checkout (DO NOT modify directly)
  my_little_factory_managers/    ← worktree parent folder
    feature-xyz/                 ← worktree for feature-xyz
    fix-something/               ← worktree for a bugfix
```

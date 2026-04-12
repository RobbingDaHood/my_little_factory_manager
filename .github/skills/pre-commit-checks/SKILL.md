---
name: pre-commit-checks
description: Run all required pre-commit checks for my_little_factory_manager. Use this before every commit to verify the code is ready.
---

Run the unified check command:

```bash
make check
```

This runs all validations in one pass (format, clippy, build, tests) and reports all errors at the end. All tests must pass — never accept or commit known test failures.

If any check fails, fix the issue and re-run `make check` until it passes.

## Commit message rules

- Always include the co-author trailer at the end of every commit message:
  ```
  Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>
  ```
- If the commit contains breaking changes (API, data format, struct layout), prefix the summary line with `BREAKING:` and list what changed in the commit body.
- Keep commits small and isolated — each commit must pass `make check` on its own.

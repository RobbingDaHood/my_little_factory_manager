Run the unified validation command:

```bash
make check
```

This runs formatting (auto-fix), clippy, build, tests, and coverage in one pass and reports all errors at the end.

**Do not commit until `make check` passes completely.** Fix all failures before proceeding.

## Commit rules

- Co-author trailer required at the end of every commit message:
  ```
  Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
  ```
- If the commit contains breaking changes (API, data format, struct layout), prefix the summary line with `BREAKING:` and list what changed in the commit body.
- Keep commits small and isolated — each commit must pass `make check` on its own.

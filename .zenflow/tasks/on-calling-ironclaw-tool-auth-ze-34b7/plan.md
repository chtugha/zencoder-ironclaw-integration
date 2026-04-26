# Fix bug

## Configuration
- **Artifacts Path**: {@artifacts_path} → `.zenflow/tasks/{task_id}`

---

## Agent Instructions

---

## Workflow Steps

### [x] Step: Investigation and Planning
<!-- chat-id: a50f8cec-6fc8-4841-9a9b-9ddc33f518c4 -->

Analyze the bug report and design a solution.

1. Review the bug description, error messages, and logs
2. Clarify reproduction steps with the user if unclear
3. Check existing tests for clues about expected behavior
4. Locate relevant code sections and identify root cause
5. Propose a fix based on the investigation
6. Consider edge cases and potential side effects

Save findings to `{@artifacts_path}/investigation.md` with:
- Bug summary
- Root cause analysis
- Affected components
- Proposed solution

### [x] Step: Implementation
<!-- chat-id: 5a2ee2f1-4a6b-40c4-8598-f425cb732d47 -->
Read `{@artifacts_path}/investigation.md`
Implement the bug fix.

### [x] Step: Codebase audit hardening
- Idempotent retries only (POST/PATCH no longer retried) [HIGH]
- README curl rewritten to use jq --arg + `--stdin` (no argv leak, no quote-injection) [HIGH]
- Bash helper: jq-first JSON body builder, control-char refusal, fixed sed extractor for numeric fields, loud warning on positional fallback [MED]
- workflow_id length validation in solve_coding_problem [MED]
- check_solution_status distinguishes plan-fetch failure from empty plan [MED]
- task_status surfaces "unparseable" + WARN log on missing field [MED]
- Magic numbers (retry budget, timeout, rate-limit threshold, UUID lens, schedule bounds, title cap) extracted to named consts [MED]
- description() / AUTH_NOT_CONFIGURED_ERROR / 401 handler recommend `--stdin` to avoid argv leak [MED]
- non-2xx error now echoes a length-bounded body preview [LOW]
- url_encode_path preserves `~` (RFC 3986 unreserved) [LOW]
- New regression tests for tilde encoding and workflow_id length

1. Add/adjust regression test(s) that fail before the fix and pass after
2. Implement the fix
3. Run relevant tests
4. Update `{@artifacts_path}/investigation.md` with implementation notes and test results

If blocked or uncertain, ask the user for direction.

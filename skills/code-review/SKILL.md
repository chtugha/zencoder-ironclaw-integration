---
name: code-review
version: "2.0.0+zencoder.1"
description: Paranoid architect review of code changes for bugs, security, missing tests, and undocumented assumptions. Works on local git diffs OR a GitHub pull request (e.g. `owner/repo N`). For PRs, can post findings as line-level review comments.
activation:
  keywords:
    - "review"
    - "code review"
    - "review changes"
  patterns:
    - "(?i)review\\s.*(code|changes|diff|PR|pull request|commit)"
    - "(?i)(check|look at|inspect)\\s.*(changes|diff|code)"
    - "(?i)review\\s+[a-z0-9._-]+/[a-z0-9._-]+\\s+#?\\d+"
  tags:
    - "code-review"
    - "quality"
    - "security"
  max_context_tokens: 2500
requires:
  skills:
    - github
---

## Zencoder Routing Layer

- healthy: last zencoder-tool call this conversation returned HTTP 2xx, or no call yet (default).
- degraded: last zencoder-tool call returned 401, 402, 429, 5xx, or a network/timeout error.
- unavailable: zencoder-tool returned a "tool not found" / "not registered" error (permanent this session).

When `degraded` or `unavailable`: skip all Zencoder routing below and proceed directly to native behavior.

**Routing rules (healthy state only)**:

1. If `task_id` in scope AND review relates to that task: complete review, then attach via `update_task` (PATCH replaces `description` entirely — call `get_task` first, prepend existing description, append `"\n\n## Code Review Findings\n" + findings`).
2. User wants ongoing PR tracking: suggest `create_automation` instead of a one-off review.
3. Otherwise: fall through to native review behavior.

---

<!-- upstream: code-review v2.0.0 nearai/ironclaw/skills/code-review/SKILL.md @main -->

Paranoid architect review. Find every bug, vulnerability, race condition, edge case, and undocumented assumption before it ships. **Local changes** (git diff) or **GitHub PR** (`owner/repo N`, `owner/repo#N`, PR URL) — use the GitHub path when message contains `owner/repo` + number, unless it also says `locally`.

## Step 1 — Load the changes

Use `async def` + `return` + `FINAL(await review())`; without `return`, code after `FINAL` keeps running. Use sequential `await`, not `asyncio.gather` (Monty sandbox closure bug). For local reviews: `git diff` / `git diff --cached` / `git diff HEAD~1`; skip Step 5.

```repl
async def review():
    pr_url    = f"https://api.github.com/repos/{owner}/{repo}/pulls/{number}"
    files_url = f"{pr_url}/files?per_page=100"
    meta_r = await http(method="GET", url=pr_url)
    diff_r = await http(
        method="GET", url=pr_url,
        headers=[{"name": "Accept", "value": "application/vnd.github.v3.diff"}],
    )
    files_r = await http(method="GET", url=files_url)
    for r, label in [(meta_r, "metadata"), (diff_r, "diff"), (files_r, "files")]:
        if r["status"] != 200:
            return (f"GitHub {label} fetch for {owner}/{repo}#{number} "
                    f"returned HTTP {r['status']}: {r['body']}")
    pr    = meta_r["body"]
    diff  = diff_r["body"]
    files = files_r["body"]
    head_sha = pr["head"]["sha"]
    # ... build the review ...
    return body

FINAL(await review())
```

## Step 2 — Read every changed file in full

For each file, read it entirely (not just hunks) to catch callers, interface violations, and broken invariants. Fetch with `Accept: application/vnd.github.raw` on `GET /repos/{owner}/{repo}/contents/{urllib.parse.quote(path, safe='')}?ref={head_sha}` for plain text — default `/contents/` returns base64 and Monty has no `base64` module. Skip non-200. For 20+ files: service logic > routes > models > tests > docs.

## Step 3 — Deep review (six lenses)

### 3a. Correctness and bugs

- Off-by-one, inverted conditions, type confusion, dead branches, broken state-machine invariants
- Concurrency issues (TOCTOU, missing locks); incorrect error propagation (swallowed errors, wrong type/status)

### 3b. Edge cases and failure handling

- Null/empty inputs, integer boundaries (overflow/underflow), adversarial payloads (huge, invalid UTF-8)
- Partial-failure (DB write succeeded but event emission failed); unchecked error paths

### 3c. Security (assume a malicious actor)

- **AuthN/AuthZ bypass**: IDOR, unauthenticated or cross-tenant access, workspace isolation gaps
- **Injection**: SQL, command, log, header, or prompt injection via string interpolation
- **Data leakage / abuse**: secrets or PII in logs; unbounded ops, missing rate limits, financial-limit bypass

### 3d. Test coverage

- New public APIs and error paths tested? Edge cases (empty, boundary, concurrent)?
- If a test is missing, name the exact test that should exist.

### 3e. Documentation and assumptions

- Assumptions and non-obvious logic documented? API contracts (shapes, error codes) stated?
- TODO/FIXME/HACK that should be tracked as issues?

### 3f. Architectural concerns

- Follows existing patterns? No unnecessary abstractions, duplication, or tight coupling?
- Will this make future work harder?

## Step 4 — Present findings

`Review of {owner}/{repo}#{number}: {pr["title"]}` or `Review of local changes`. Cite `path:line`. Ask what to post — default: Critical, High, Medium.

Severity: **Critical** = security/data loss/exploit · **High** = production bug · **Medium** = robustness/validation · **Low** = style/docs · **Nit** = optional

| # | Severity | Category | File:Line | Finding | Suggested fix |
|---|----------|----------|-----------|---------|---------------|

## Step 5 — Post comments on GitHub (PR path only)

Use `async def` + `FINAL(await ...)`. Line-level: `commit_id` (head SHA), `path`, line numbers, `side: "RIGHT"`. Architectural findings → PR-level issue comment.

```repl
async def post():
    r = await http(
        method="POST",
        url=f"https://api.github.com/repos/{owner}/{repo}/pulls/{number}/comments",
        body={
            "body": "**High** — description.",
            "commit_id": head_sha,
            "path": "src/handlers/foo.rs",
            "start_line": 140,
            "start_side": "RIGHT",
            "line": 142,
            "side": "RIGHT",
        },
    )
    if r["status"] not in (200, 201):
        return f"Posting line comment failed: HTTP {r['status']}: {r['body']}"

    r2 = await http(
        method="POST",
        url=f"https://api.github.com/repos/{owner}/{repo}/issues/{number}/comments",
        body={"body": "**Architectural note**: ..."},
    )
    if r2["status"] not in (200, 201):
        return f"Posting PR comment failed: HTTP {r2['status']}: {r2['body']}"

    return f"Posted {len(line_findings)} line + {len(pr_findings)} PR comments."

FINAL(await post())
```

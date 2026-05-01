---
name: commit
version: "1.0.0+zencoder.1"
description: Generate git commit messages from staged changes
activation:
  keywords:
    - "commit"
    - "git commit"
  patterns:
    - "(?i)(create|make|write|generate)\\s.*commit"
    - "(?i)commit\\s.*(message|changes|staged)"
  tags:
    - "git"
    - "version-control"
  max_context_tokens: 1000
---

## Zencoder Routing Layer

- healthy: last zencoder-tool call this conversation returned HTTP 2xx, or no call yet (default).
- degraded: last zencoder-tool call returned 401, 402, 429, 5xx, or a network/timeout error.
- unavailable: zencoder-tool returned a "tool not found" / "not registered" error (permanent this session).

When `degraded` or `unavailable`: skip all Zencoder routing below and proceed directly to native behavior.

**Routing rules (healthy state only)**:

1. Check conversation context for the most recently mentioned Zencoder task and its status.
2. If a `task_id` is in scope AND that task's last known status is `inprogress` or `inreview`: do NOT commit locally — the remote agent owns the branch. Warn the user once and suggest calling `check_solution_status` to verify the task state before committing.
3. If the user explicitly confirms they want to commit anyway after the warning: proceed with native commit behavior.
4. If the task status is `done`, `cancelled`, or no `task_id` is in scope: fall through to native commit behavior.

---

<!-- upstream: commit v1.0.0 nearai/ironclaw/skills/commit/SKILL.md @main -->

# Git Commit Workflow

When the user asks to create a commit:

1. Run `shell` with `git status` to see what files are staged and unstaged.
2. Run `shell` with `git diff --cached` to see the exact changes that will be committed.
3. Run `shell` with `git log --oneline -5` to understand the repo's commit message style.
4. Analyze the staged changes and draft a commit message:
   - Summarize the nature of the change (new feature, bug fix, refactor, etc.)
   - Keep it concise: 1-2 sentences focusing on **why**, not **what**
   - Match the repo's existing commit message style
5. **Do not commit files that likely contain secrets** (`.env`, `credentials.json`, API keys). Warn the user if such files are staged.
6. Show the proposed commit message to the user and **ask for confirmation** before running `git commit`.
7. Stage any requested files with `git add <specific files>` (never use `git add -A` or `git add .`).

## Commit Message Format

If the repo doesn't have a clear style, use:
```
<type>: <concise description>

<optional body explaining why>
```

Where type is: fix, feat, refactor, test, docs, chore.

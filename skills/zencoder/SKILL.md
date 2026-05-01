---
name: zencoder
version: "1.3.0+slim.1"
description: Routes Zencoder/Zenflow operations through the zencoder-tool WASM extension. Slim core — routing logic for coding, review, plan, commit, and delegation is embedded in the respective replacement skills.
activation:
  keywords:
    - "zencoder"
    - "zenflow"
    - "zencoder-tool"
    - "solve_coding_problem"
    - "check_solution_status"
    - "list_projects"
    - "create_task"
    - "list_tasks"
    - "update_task"
    - "get_plan"
    - "update_plan_step"
    - "create_automation"
    - "delegate to zencoder"
    - "delegate to zenflow"
    - "zencoder task"
    - "zenflow task"
    - "zencoder project"
    - "zenflow project"
    - "api.zencoder.ai"
    - "auth.zencoder.ai"
  patterns:
    - "(?i)zen(coder|flow)"
    - "(?i)(delegate|hand[- ]off|send|push|forward)\\s.*(to|on)\\s+zen(coder|flow)"
    - "(?i)(have|let|ask|tell|get)\\s+zen(coder|flow)"
    - "(?i)solve[_ ]coding[_ ]problem"
    - "(?i)zen(coder|flow)\\s+(project|task|plan|workflow|automation|branch|status)"
  tags:
    - "zencoder"
    - "zenflow"
    - "delegation"
    - "coding"
    - "task-management"
    - "automation"
  max_context_tokens: 1000
requires:
  skills:
    - coding
    - code-review
    - plan-mode
    - commit
    - delegation
---

# Zencoder Integration

## Tool Overview

`zencoder-tool` exposes 17 actions in 6 categories:

- **Projects**: `list_projects`, `get_project`
- **Tasks**: `create_task`, `list_tasks`, `get_task`, `update_task`
- **Workflows**: `list_workflows`
- **Plans**: `get_plan`, `create_plan`, `update_plan_step`, `add_plan_steps`
- **Automations**: `list_automations`, `create_automation`, `toggle_automation`, `list_task_automations`
- **Convenience**: `solve_coding_problem`, `check_solution_status`

## Authentication

Generate a personal access token at `https://auth.zencoder.ai`, run `scripts/zencoder-auth.sh` (or `.ps1` on Windows), then paste the JWT with `ironclaw tool auth zencoder-tool`. Re-run on expiry or 401.

## solve_coding_problem

1. If no `project_id` is known, call `list_projects` first and select the project.
2. Call `solve_coding_problem` with `project_id` and a detailed `description`. Do **not** call `create_task` manually; do **not** edit the codebase locally.
3. Report the returned `task_id` to the user and retain it for follow-up queries.

## check_solution_status

1. Call `check_solution_status` with `project_id` and `task_id` from the original delegation.
2. Summarize `task_status`, `progress` (e.g. "3 of 5 steps completed"), and `branch` if present.
3. Flag any step stuck in `InProgress` — do not retry unless asked. (`Pending` = not yet started, not stuck.)

## Task Management

- `list_tasks`: pass a `status` filter (`todo`, `inprogress`, `inreview`, `done`, `cancelled`).
- `get_task`: requires `project_id` + `task_id`; returns current task including `description`.
- `update_task`: requires `project_id` + `task_id`; at least one of `title`, `description`, or `status` must be non-empty. **PATCH fully replaces `description`** — call `get_task` first, then append.
- `get_plan`: requires `project_id` + `task_id`.

## Automation Management

- `list_automations`: lists all global automations; supports optional `enabled` filter.
- `create_automation`: requires `name`; for scheduled runs add `schedule_time` (`HH:MM` 24-hour) and `schedule_days_of_week` (integers 0–6, Sunday=0).
- `toggle_automation`: pause/resume without deleting.
- `list_task_automations`: requires `task_id`; lists automations scoped to a specific task.

## Input Constraints

- All ID fields must be valid UUIDs (8-4-4-4-12 hex).
- Status values are case-sensitive: task statuses are lowercase (`inprogress`, `done`); step statuses are PascalCase (`InProgress`, `Completed`).
- All text fields are capped at 64 KiB.

## Error Handling

| Code | Action |
|---|---|
| 401 | Re-authenticate: run `scripts/zencoder-auth.sh` → `ironclaw tool auth zencoder-tool`. |
| 402 | Quota/billing limit — wait for user to confirm resolution, then probe. |
| 429 | Wait for `Retry-After` interval, then probe once. |
| 5xx | Exponential backoff: wait 1 turn after 1st failure, 2 after 2nd, 4 after 3rd; cap at 8 turns; reset on success. |
| network | Probe after 5 turns pass, a different HTTP call succeeds, or user confirms connectivity. |
| unavailable | "tool not found" / "not registered" error — permanent this session; never probe. |

## Resilience State Machine

| State | Trigger | Probe rule |
|---|---|---|
| `healthy` | HTTP 2xx; default at conversation start | — |
| `degraded` | 401/402/429/5xx/network error | Per error-handling table above |
| `unavailable` | "tool not found" / "not registered" error | Never probe |

When `degraded` or `unavailable`: skip all Zencoder calls and fall through to native skill behavior. Inform the user once per turn which fallback ran.

## Anti-patterns

- Do not construct raw HTTP to `api.zencoder.ai` — always use the tool.
- Do not ask for credentials in chat — point to the helper script and `ironclaw tool auth`.
- Do not call `create_task` + manual start for coding — use `solve_coding_problem`.
- Do not mirror plan state between Zencoder and local `plan-mode` simultaneously.

Routing logic for `coding`, `code-review`, `plan-mode`, `commit`, and `delegation` is embedded in the respective replacement skills.

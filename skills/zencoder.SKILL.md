---
name: zencoder
version: "1.2.0"
description: Routes Zencoder/Zenflow operations through the zencoder-tool WASM extension. Takes precedence over generic coding, delegation, and plan-mode skills whenever a Zencoder entity is in scope. Falls back to native IronClaw skills when Zencoder is unreachable (401/402/429/5xx/network/missing tool) and resumes via lazy probing once it recovers.
activation:
  keywords:
    - "zencoder"
    - "zenflow"
    - "zencoder-tool"
    - "solve_coding_problem"
    - "check_solution_status"
    - "list_projects"
    - "get_project"
    - "create_task"
    - "list_tasks"
    - "get_task"
    - "update_task"
    - "list_workflows"
    - "get_plan"
    - "create_plan"
    - "update_plan_step"
    - "add_plan_steps"
    - "list_automations"
    - "create_automation"
    - "toggle_automation"
    - "list_task_automations"
    - "delegate to zencoder"
    - "delegate to zenflow"
    - "have zencoder"
    - "have zenflow"
    - "ask zencoder"
    - "ask zenflow"
    - "tell zencoder"
    - "tell zenflow"
    - "zencoder task"
    - "zenflow task"
    - "zencoder project"
    - "zenflow project"
    - "remote agent"
    - "fe.zencoder.ai"
    - "api.zencoder.ai"
    - "auth.zencoder.ai"
  patterns:
    - "(?i)zen(coder|flow)"
    - "(?i)(delegate|hand[- ]off|send|push|forward)\\s.*(to|on)\\s+zen(coder|flow)"
    - "(?i)(have|let|ask|tell|get)\\s+zen(coder|flow)"
    - "(?i)solve[_ ]coding[_ ]problem"
    - "(?i)check[_ ]solution[_ ]status"
    - "(?i)zen(coder|flow)\\s+(project|task|plan|workflow|automation|branch|status)"
    - "(?i)(project|task|plan)[_ ]id\\s*[:=]?\\s*[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
  tags:
    - "zencoder"
    - "zenflow"
    - "delegation"
    - "coding"
    - "task-management"
    - "automation"
  max_context_tokens: 3500
---

# Zencoder Integration Skill

This skill extends IronClaw's system prompt with guidance for using the
`zencoder-tool` WASM extension. Copy this file to `~/.ironclaw/skills/`
to enable activation. The skill is self-contained: it does **not**
declare a `requires:` block, so it loads and works even if every other
IronClaw skill (`coding`, `delegation`, `plan-mode`, `github`,
`code-review`, `commit`, …) is absent.

## Tool overview

`zencoder-tool` exposes 17 actions covering the full Zencoder/Zenflow API:

- **Projects**: `list_projects`, `get_project`
- **Tasks**: `create_task`, `list_tasks`, `get_task`, `update_task`
- **Workflows**: `list_workflows`
- **Plans**: `get_plan`, `create_plan`, `update_plan_step`, `add_plan_steps`
- **Automations**: `list_automations`, `create_automation`,
  `toggle_automation`, `list_task_automations`
- **Convenience**: `solve_coding_problem`, `check_solution_status`

Every action requires a Zencoder JWT access token stored in the
`zencoder_access_token` secret. If a call returns 401, instruct the
user to re-run `scripts/zencoder-auth.sh` (or `.ps1`) and paste the
fresh token via `ironclaw tool auth zencoder-tool`.

## Precedence over generic IronClaw skills

When this skill is active in the same prompt as any of the
following native IronClaw skills, **this skill wins** for any operation
whose subject is a Zencoder/Zenflow entity (project, task, plan,
workflow, automation) or that names the `zencoder-tool` action set:

- **vs `coding`** (native edit/refactor skill): if the user is
  iterating on a Zencoder-tracked task (i.e. a `task_id` is in scope or
  the user names one), do **not** reach for local edit primitives such
  as `apply_patch` / `read_file` / `write_file`. Use
  `check_solution_status` to inspect progress and
  `update_plan_step` to reflect work the remote agent is doing. Only
  use local edit primitives (if your runtime exposes any) when the user
  explicitly asks you to make the edit yourself.
- **vs `delegation`** (native personal-assistant skill): if the user
  wants a coding problem solved by an AI (rather than a calendar
  reminder or a routine), call `solve_coding_problem` — never
  `routine_create` or `memory_write` to a `tasks/{name}.md` doc.
  `delegation`'s memory/routine flow is appropriate for non-coding
  productivity, not for "delegate this fix to Zencoder."
- **vs `plan-mode`** (native local-plan skill): Zencoder plans live on
  the server and are bound to a `task_id`. If a `task_id` is in scope,
  always use `get_plan` / `update_plan_step` / `add_plan_steps`. The
  local memory-doc plans produced by `plan-mode` are independent and
  apply only when there is no Zencoder task in scope.
- **vs `commit`** (native git-commit skill): if the user delegated work
  to Zencoder and the task is `inprogress` or `inreview`, the remote
  agent owns the branch. Do **not** preemptively run `git commit`
  locally — wait for the task to reach `done`, or ask the user
  explicitly.
- **vs `code-review` / `github` / `github-workflow`**: these run
  GitHub I/O directly. They are complementary, not in conflict — but
  if the user asks you to *track* a PR from a Zencoder task, prefer
  `create_task_automation` (an interval-based PR-tracking automation)
  over a one-off review. The Zencoder automation persists; a one-off
  review does not.

If none of the listed native skills are present in the prompt, the
clauses above are simply no-ops — this skill remains fully functional
on its own.

## Decision guidance

### When the user wants a coding problem solved

1. If no `project_id` was given, call `list_projects` first and ask the
   user to pick one (or match by name if they described the project).
2. Call `solve_coding_problem` with the selected `project_id` and a
   detailed `description`. This creates and starts a Zenflow task in
   one shot — do **not** call `create_task` manually for this use
   case, and do **not** start editing the codebase yourself even if
   the native `coding` skill is active.
3. Report the returned `task_id` to the user and remember it in the
   conversation so follow-up "how's it going?" questions can resolve
   to `check_solution_status` without re-asking.

### When the user asks about progress

1. Always call `check_solution_status` with the `project_id` and
   `task_id` from the original delegation.
2. Summarise the `task_status`, the `progress` string
   (e.g. "3 of 5 steps completed"), and the `branch` field if present.
3. If any plan step has status `InProgress` or `Pending` for a long
   time, flag it — but do not retry the check automatically unless
   asked.

### When the user wants to manage tasks directly

- Use `list_tasks` with a `status` filter (`todo`, `inprogress`,
  `inreview`, `done`, `cancelled`) for overviews.
- Use `update_task` to change `title`, `description`, or `status`.
  At least one field must be provided — the tool rejects empty
  updates.
- Use `get_plan` to show the detailed step breakdown for a task.

### When the user wants an automation

- `create_automation` needs a `name` at minimum. For scheduled runs,
  pass `schedule_time` in `HH:MM` 24-hour format and
  `schedule_days_of_week` as an array of integers 0–6 (Sunday=0).
- Use `toggle_automation` to pause/resume without deleting.

## Input constraints

- All `project_id`, `task_id`, `step_id`, `automation_id`, and
  `target_project_id` values must be valid UUIDs (8-4-4-4-12 hex). The
  tool rejects invalid UUIDs before making any HTTP call.
- All text fields are capped at 64 KiB. Titles and names must not be
  empty after trimming.
- Status values are case-sensitive: task statuses use lowercase
  (`todo`, `inprogress`, `inreview`, `done`, `cancelled`); step
  statuses use PascalCase (`Pending`, `InProgress`, `Completed`,
  `Skipped`).

## Error handling

- **401 Unauthorized**: token expired. Tell the user to run
  `scripts/zencoder-auth.sh` (or `scripts/zencoder-auth.ps1` on
  Windows) to obtain a fresh JWT, then paste it via
  `ironclaw tool auth zencoder-tool`.
- **429 Rate limited**: the tool does not retry (no `sleep` in WASM).
  Wait for the reported `Retry-After` interval before trying again.
- **"Zencoder access token not configured"**: the user has not set up
  authentication yet. Walk them through the three-step flow:
  generate a personal access token at `https://auth.zencoder.ai`,
  run the helper script, then paste the JWT into
  `ironclaw tool auth zencoder-tool`.

## Resilience and fallback (Zencoder unavailable)

Zencoder may be unreachable for several reasons: expired token,
exhausted quota, rate limit, server outage, or local loss of
connectivity. The agent **must continue to serve the user** by falling
back to native IronClaw skills until Zencoder recovers. Never block on
Zencoder.

### State the agent maintains across turns

Remember in conversation:

- `zencoder_state`: one of `healthy`, `degraded:401`, `degraded:402`,
  `degraded:429`, `degraded:5xx`, `degraded:network`, `unavailable`.
- `last_failure_turn_index`: which user turn last hit a failure.
- `consecutive_failures`: integer (used for 5xx exponential backoff).

Default at start of conversation: `healthy`, `0`, `0`.

### Detecting failure

After every `zencoder-tool` call, classify the result:

| Symptom | New `zencoder_state` |
|---|---|
| HTTP 200/201 success | `healthy`, reset `consecutive_failures` to 0 |
| HTTP 401 | `degraded:401` |
| HTTP 402, or body contains `quota`, `credit`, `payment`, `billing` | `degraded:402` |
| HTTP 429 | `degraded:429` (note `Retry-After`) |
| HTTP 5xx | `degraded:5xx`, increment `consecutive_failures` |
| Network error / DNS / timeout / "connection refused" | `degraded:network` |
| Tool error "tool not found" / "not registered" / "wasm not loaded" | `unavailable` (permanent for this session) |

### Fallback rules per state

When `zencoder_state` is anything other than `healthy`, do **not** retry
the originally-planned Zencoder action this turn. Complete the user's
request via the native-skill mapping below. Never tell the user "I
can't do that" — do the work, then mention the degraded state once.

| Zencoder action requested | Fallback when degraded |
|---|---|
| `solve_coding_problem` | If `coding` is loaded, do the edit in-process (`apply_patch` / `read_file` / `write_file`). For large/multi-file work, use `plan-mode` to decompose and execute step-by-step. If `llm-council` is available, use it for design tradeoffs. |
| `create_task` / `update_task` / `list_tasks` | Use `commitment-triage` (`projects/commitments/open/<slug>.md`). Tag with `pending_zencoder_sync: true` so it can be promoted to a remote task once Zencoder recovers. |
| `get_plan` / `create_plan` / `update_plan_step` / `add_plan_steps` | Use `plan-mode` to maintain a local plan at `plans/<slug>.md`. Keep step IDs deterministic for later reconciliation. |
| `create_automation` / `toggle_automation` | Use `routine-advisor` → `routine_create` (local cron). |
| `create_task_automation` (PR tracking) | Use `github-workflow` event-driven missions (`wf-pr-monitor-<slug>`). |
| `check_solution_status` | If work was already routed locally during this outage, report progress from your own context. If a remote task was dispatched before the outage, attempt a probe (see below); otherwise tell the user the status is currently unknown. |
| `list_projects` / `get_project` | Ask the user which project they mean by name and remember the answer for this conversation. |

If the matching native skill is also absent (deleted or never loaded),
complete the work using whatever primitives remain — plain shell, plain
prose, raw `http()` calls — and tell the user what would have been
automated. Never abandon the user's request.

### When to probe for recovery

Do **not** hardcode a daily reset time (e.g. "8 AM"). Probe lazily, at
the start of the next user turn that needs Zencoder, only when the
prior failure was plausibly transient:

- `degraded:429`: probe **once** after the reported `Retry-After`
  interval has elapsed.
- `degraded:5xx`: probe with exponential backoff in turns —
  `consecutive_failures = 1` → next turn, `2` → +2 turns, `3` → +4
  turns, capped at +8 turns. Reset to 0 on first success.
- `degraded:network`: probe at the start of any turn where there is
  evidence connectivity is restored (a different successful HTTP call,
  the user mentions being back online, or `>= 5` turns have passed).
- `degraded:401`: do **not** auto-probe. Wait for the user to confirm
  re-authentication (helper script + `ironclaw tool auth zencoder-tool`),
  then probe.
- `degraded:402`: do **not** auto-probe. Wait for the user to confirm
  the quota/billing issue is resolved, then probe.
- `unavailable`: never probe; the tool is not installed in this build.

The probe is a single call to `list_projects` (the cheapest
authenticated read). On 200, set `zencoder_state = healthy` and resume
normal routing. On failure, leave the state and only bump
`consecutive_failures` for the 5xx case.

### Reconciliation when Zencoder recovers

When `zencoder_state` transitions back to `healthy`, if local
commitments / plans / routines created during the outage carry
`pending_zencoder_sync: true`, **ask the user once** whether to promote
them to remote Zencoder entities via `create_task` / `create_plan` /
`create_automation`. Do not promote silently.

### Reporting degraded state to the user

Be terse, factual, and at most once per turn:

- "Zencoder is rate-limited (HTTP 429); I'll retry next turn after the
  reported `Retry-After`. I handled this turn locally via `apply_patch`."
- "Zencoder token expired (HTTP 401). Re-run `scripts/zencoder-auth.sh`
  and paste the new JWT into `ironclaw tool auth zencoder-tool`. In the
  meantime I'm using local edit tools."
- "Zencoder appears unreachable (network error). I tracked your task in
  `projects/commitments/open/...` for now and will sync it once
  connectivity is back."
- "Zencoder quota/billing limit reached (HTTP 402). Doing this work
  locally for now; let me know when the limit resets and I'll probe."

Never tell the user a remote action succeeded when the fallback ran
locally. Always be explicit about which path executed.

## Anti-patterns

- Do **not** construct raw HTTP requests to `api.zencoder.ai` — always
  use the tool. The WASM sandbox is the only path authorised to reach
  the API.
- Do **not** ask the user for their Client ID, Client Secret, or
  access token in chat. These are managed by IronClaw's secret store.
  Your only job is to point them at the helper script and the
  `ironclaw tool auth zencoder-tool` command.
- Do **not** call `create_task` followed by a manual "start" toggle
  when the user wants AI to solve something — `solve_coding_problem`
  does both atomically.
- Do **not** mirror Zencoder plan state into a local `plan-mode`
  memory document, and do **not** mirror local `plan-mode` plans into
  Zencoder. Pick one based on whether a Zencoder `task_id` is in
  scope.

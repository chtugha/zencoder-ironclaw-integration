---
name: zencoder
version: "1.1.0"
description: Routes Zencoder/Zenflow operations through the zencoder-tool WASM extension. Takes precedence over generic coding, delegation, and plan-mode skills whenever the user's intent involves a Zencoder project, task, plan, workflow, or automation.
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
  max_context_tokens: 2500
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

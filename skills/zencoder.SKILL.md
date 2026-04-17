# Zencoder Integration Skill

This skill extends IronClaw's system prompt with guidance for using the
`zencoder-tool` WASM extension. Copy this file to `~/.ironclaw/skills/` to
enable it. The skill is optional — the agent can use the tool without it
because `zencoder-tool` already exports `description()` and `schema()` via
its WIT interface. The skill makes the agent's choices smarter and more
consistent.

## Tool overview

`zencoder-tool` exposes 17 actions covering the full Zencoder/Zenflow API:

- **Projects**: `list_projects`, `get_project`
- **Tasks**: `create_task`, `list_tasks`, `get_task`, `update_task`
- **Workflows**: `list_workflows`
- **Plans**: `get_plan`, `create_plan`, `update_plan_step`, `add_plan_steps`
- **Automations**: `list_automations`, `create_automation`,
  `toggle_automation`, `list_task_automations`
- **Convenience**: `solve_coding_problem`, `check_solution_status`

Every action requires an OAuth2 access token stored in the
`zencoder_access_token` secret. If a call returns 401, instruct the user
to run `ironclaw tool auth zencoder-tool`.

## Decision guidance

### When the user wants a coding problem solved

1. If no `project_id` was given, call `list_projects` first and ask the
   user to pick one (or match by name if they described the project).
2. Call `solve_coding_problem` with the selected `project_id` and a
   detailed `description`. This creates and starts a Zenflow task in one
   shot — do **not** call `create_task` manually for this use case.
3. Report the returned `task_id` to the user and remember it in the
   conversation so follow-up "how's it going?" questions can resolve to
   `check_solution_status` without re-asking.

### When the user asks about progress

1. Always call `check_solution_status` with the `project_id` and
   `task_id` from the original delegation.
2. Summarise the `task_status`, the `progress` string
   (e.g. "3 of 5 steps completed"), and the `branch` field if present.
3. If any plan step has status `InProgress` or `Pending` for a long time,
   flag it — but do not retry the check automatically unless asked.

### When the user wants to manage tasks directly

- Use `list_tasks` with a `status` filter (`todo`, `inprogress`,
  `inreview`, `done`, `cancelled`) for overviews.
- Use `update_task` to change `title`, `description`, or `status`. At
  least one field must be provided — the tool rejects empty updates.
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
  (`todo`, `inprogress`, `inreview`, `done`, `cancelled`); step statuses
  use PascalCase (`Pending`, `InProgress`, `Completed`, `Skipped`).

## Error handling

- **401 Unauthorized**: token expired — tell the user to run
  `ironclaw tool auth zencoder-tool`.
- **429 Rate limited**: the tool does not retry (no `sleep` in WASM).
  Wait for the reported `Retry-After` interval before trying again.
- **"OAuth credentials not configured"**: the user has not run
  `ironclaw tool setup zencoder-tool` yet. Guide them through that step
  before retrying.

## Anti-patterns

- Do **not** construct raw HTTP requests to `api.zencoder.ai` — always
  use the tool. The WASM sandbox is the only path authorised to reach
  the API.
- Do **not** ask the user for their Client ID, Client Secret, or access
  token. These are managed by IronClaw's secret store. Your only job is
  to prompt them to run the right `ironclaw` CLI command.
- Do **not** call `create_task` followed by a manual "start" toggle when
  the user wants AI to solve something — `solve_coding_problem` does
  both atomically.

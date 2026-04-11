# Zencoder Tool for IronClaw

An IronClaw extension that integrates Zencoder/Zenflow's AI-powered coding capabilities. Delegate complex coding problems to Zenflow agents, manage projects, tasks, plans, workflows, and automations — all from within IronClaw.

## What It Does

This tool gives IronClaw agents access to Zencoder's Zenflow platform through 17 actions:

| Category | Actions |
|---|---|
| **Projects** | `list_projects`, `get_project` |
| **Tasks** | `create_task`, `list_tasks`, `get_task`, `update_task` |
| **Workflows** | `list_workflows` |
| **Plans** | `get_plan`, `create_plan`, `update_plan_step`, `add_plan_steps` |
| **Automations** | `list_automations`, `create_automation`, `toggle_automation`, `list_task_automations` |
| **Convenience** | `solve_coding_problem`, `check_solution_status` |

The two convenience actions are the primary interface: `solve_coding_problem` creates and starts a Zenflow task that AI agents work on autonomously, and `check_solution_status` reports progress including plan steps and branch name.

## Prerequisites

- [Rust](https://rustup.rs/) with the `wasm32-wasip1` target
- [IronClaw CLI](https://github.com/nearai/ironclaw)
- A Zencoder API key (get one from [app.zencoder.ai/settings](https://app.zencoder.ai/settings))

### Install the WASM target

```bash
rustup target add wasm32-wasip1
```

## Build

```bash
cd zencoder-tool
cargo build --target wasm32-wasip1 --release
```

The compiled WASM binary is at `zencoder-tool/target/wasm32-wasip1/release/zencoder_tool.wasm` (~253KB).

## Install into IronClaw

### 1. Register the tool

```bash
ironclaw tool register \
  --name zencoder-tool \
  --wasm ./zencoder-tool/target/wasm32-wasip1/release/zencoder_tool.wasm \
  --capabilities ./zencoder-tool/zencoder-tool.capabilities.json
```

### 2. Set your API key

```bash
ironclaw secret set zencoder_api_key <your-api-key>
```

Your key starts with `zen_` and is obtained from [app.zencoder.ai/settings](https://app.zencoder.ai/settings). The key is stored securely by IronClaw and injected as a Bearer token on API requests — it never enters the WASM sandbox.

### 3. Verify

```bash
ironclaw tool list
```

You should see `zencoder-tool` in the output.

## Usage Examples

Once installed, IronClaw agents can invoke the tool. Here are example payloads:

### Delegate a coding problem

```json
{
  "action": "solve_coding_problem",
  "project_id": "550e8400-e29b-41d4-a716-446655440000",
  "description": "Fix the authentication middleware to properly validate JWT expiration"
}
```

Returns a `task_id` for tracking.

### Check solution progress

```json
{
  "action": "check_solution_status",
  "project_id": "550e8400-e29b-41d4-a716-446655440000",
  "task_id": "da1d251c-0cea-4fe6-a744-ec2986035c35"
}
```

Returns task status, plan step progress, and branch name.

### List projects

```json
{
  "action": "list_projects"
}
```

### Create a task manually

```json
{
  "action": "create_task",
  "project_id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Refactor database layer",
  "description": "Extract connection pooling into a shared module",
  "workflow_id": "default-auto-workflow",
  "start": true
}
```

### Create a scheduled automation

```json
{
  "action": "create_automation",
  "name": "Daily test run",
  "target_project_id": "550e8400-e29b-41d4-a716-446655440000",
  "task_name": "Run test suite",
  "schedule_time": "09:00",
  "schedule_days_of_week": [1, 2, 3, 4, 5]
}
```

## Development

### Run tests

```bash
cd zencoder-tool
cargo test
```

### Lint and format

```bash
cd zencoder-tool
cargo fmt --check
cargo clippy --target wasm32-wasip1 --all --all-features
```

## Security

- The WASM sandbox never sees your API key — IronClaw injects it via credential injection
- HTTP requests are restricted to `api.zencoder.ai` with `GET`, `POST`, `PATCH` methods only (no `PUT` or `DELETE`)
- All UUID inputs are validated before URL interpolation to prevent path traversal
- All string inputs are length-bounded (64KB max)
- URL path segments are percent-encoded
- Rate limited to 60 requests/minute, 1000 requests/hour
- No `unsafe` code

## Project Structure

```
zencoder-tool/
  Cargo.toml                          # Rust package config (cdylib WASM target)
  zencoder-tool.capabilities.json     # IronClaw capability manifest
  src/
    lib.rs                            # All implementation (1800+ lines, 73 tests)
wit/
  tool.wit                            # near:agent@0.3.0 sandboxed-tool WIT interface
```

## License

MIT OR Apache-2.0

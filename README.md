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

- [Rust](https://rustup.rs/) 1.85+ with the `wasm32-wasip2` target
- [IronClaw CLI](https://github.com/nearai/ironclaw) 0.25+
- A Zencoder personal access token (Client ID + Client Secret from [auth.zencoder.ai](https://auth.zencoder.ai))

## Installation Guide (Debian 12)

These steps assume a fresh Debian 12 (Bookworm) system. Adjust if you already have some components installed.

### 1. Install system dependencies

```bash
sudo apt update
sudo apt install -y curl build-essential pkg-config libssl-dev git
```

### 2. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

Verify the installation:

```bash
rustc --version   # should show 1.85.x or later
cargo --version
```

### 3. Add the WASM target

```bash
rustup target add wasm32-wasip2
```

Verify:

```bash
rustup target list --installed | grep wasm32-wasip2
```

### 4. Install IronClaw

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/nearai/ironclaw/releases/latest/download/ironclaw-installer.sh | sh
```

If the installer doesn't add `ironclaw` to your PATH automatically, add it:

```bash
export PATH="$HOME/.ironclaw/bin:$PATH"
```

Verify:

```bash
ironclaw --version
```

If this is your first time using IronClaw, run the setup wizard:

```bash
ironclaw onboard
```

### 5. Clone and build the extension

```bash
git clone https://github.com/chtugha/zencoder-ironclaw-integration.git
cd zencoder-ironclaw-integration/zencoder-tool
cargo build --target wasm32-wasip2 --release
```

The compiled WASM binary is at `target/wasm32-wasip2/release/zencoder_tool.wasm` (~253KB).

### 6. Install the tool into IronClaw

```bash
ironclaw tool install \
  --name zencoder-tool \
  target/wasm32-wasip2/release/zencoder_tool.wasm \
  --capabilities zencoder-tool.capabilities.json \
  --skip-build
```

Alternatively, install directly from the source directory (IronClaw builds it for you):

```bash
cd ..  # back to zencoder-ironclaw-integration/
ironclaw tool install ./zencoder-tool
```

### 7. Configure Zencoder OAuth credentials

First, generate a personal access token at [auth.zencoder.ai](https://auth.zencoder.ai):

1. Log in to **auth.zencoder.ai**
2. Go to **Administration > Settings > Personal Tokens**
3. Create a new token — copy the **Client ID** and **Client Secret** immediately (the secret is only shown once)

Then provide the credentials to IronClaw:

```bash
ironclaw tool setup zencoder-tool
```

When prompted, paste the **Client ID** and **Client Secret** from the previous step.

### 8. Authenticate (OAuth token exchange)

```bash
ironclaw tool auth zencoder-tool
```

This exchanges your Client ID and Client Secret for a JWT access token via Zencoder's OAuth2 `client_credentials` flow. IronClaw handles token injection automatically — credentials never enter the WASM sandbox.

To re-authenticate after token expiry, run `ironclaw tool auth zencoder-tool` again.

**Manual token fallback**: If `ironclaw tool auth` is not supported in your IronClaw version, you can obtain a token manually and set it directly:

```bash
# Prompt for credentials without echoing or storing in shell history
read -rp "Client ID: " ZENCODER_CLIENT_ID
read -rs -p "Client Secret: " ZENCODER_CLIENT_SECRET && echo

# Exchange for an access token
curl -s -X POST https://fe.zencoder.ai/oauth/token \
  -H "Content-Type: application/json" \
  -d "{\"client_id\": \"$ZENCODER_CLIENT_ID\", \"client_secret\": \"$ZENCODER_CLIENT_SECRET\", \"grant_type\": \"client_credentials\"}"

# Copy the access_token value from the JSON response, then:
ironclaw secret set zencoder_access_token <paste-access-token-here>

# Clear the variables
unset ZENCODER_CLIENT_ID ZENCODER_CLIENT_SECRET
```

### 9. Verify the installation

```bash
ironclaw tool list
```

You should see `zencoder-tool` in the output. For detailed info:

```bash
ironclaw tool info zencoder-tool
```

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
cargo clippy --target wasm32-wasip2 --all --all-features
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

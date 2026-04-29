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

> **Already installed?** IronClaw refuses to overwrite an existing tool unless you pass `--force`. To upgrade after a `git pull`, either pass `--force` to the install commands above, or remove the previous copy first:
>
> ```bash
> ironclaw tool remove zencoder-tool   # deletes wasm + capabilities sidecar
> ```
>
> (The subcommand is `remove`, not `uninstall` — IronClaw 0.25/0.26 does not register `uninstall` as an alias.) After remove + reinstall, confirm the new build is live:
>
> ```bash
> ironclaw tool info zencoder-tool
> # Hash should change every time the wasm or capabilities JSON changes.
> ```

### 7. Obtain and install a Zencoder access token

> **Why the helper script exists.** IronClaw's `ironclaw tool auth` supports two flows: browser OAuth (`authorization_code` + PKCE) and a manual "paste the token" prompt. Zencoder's token endpoint only supports `client_credentials` — a server-to-server grant with no browser redirect — which IronClaw cannot perform. The bundled helper does the credential exchange and prints the resulting JWT; you then paste it into `ironclaw tool auth`.

**Step 1 — generate a personal access token at [auth.zencoder.ai](https://auth.zencoder.ai):**

1. Log in to **auth.zencoder.ai**
2. Go to **Administration > Settings > Personal Tokens**
3. Create a new token — copy the **Client ID** and **Client Secret** immediately (the secret is only shown once)

**Step 2 — run the helper to get the JWT:**

**Linux / macOS / WSL (bash):**

```bash
./scripts/zencoder-auth.sh
# Zencoder Client ID: <paste Client ID>
# Zencoder Client Secret: <paste Client Secret, hidden>
#
# ╔══════════════════════════════════════════════════════════════╗
# ║  ✓ JWT obtained successfully                                 ║
# ╚══════════════════════════════════════════════════════════════╝
#
#   Your Zencoder access token:
#
#   eyJhbGciOiJSUz...   ← copy this
#
#   Next step — paste it into IronClaw:
#     ironclaw tool auth zencoder-tool
```

**Windows (PowerShell):**

```powershell
.\scripts\zencoder-auth.ps1
```

The bash script falls back through `jq` → `python3` → `python` → `sed` to parse the response, so `jq` is recommended but not required. The PowerShell script uses `Invoke-RestMethod` and has no extra dependencies.

Useful flags (bash / PowerShell):

| Flag | Purpose |
|---|---|
| `--client-id ID` / `-ClientId ID` | Skip the Client ID prompt (e.g. for CI). |
| `--client-secret SECRET` / `-ClientSecret <SecureString>` | Skip the Client Secret prompt. **Avoid in interactive shells** — leaks into history / process list. |
| `--token-url URL` / `-TokenUrl URL` | Override the OAuth endpoint (e.g. for a staging tenant). |

**Step 3 — paste the JWT into IronClaw:**

```bash
ironclaw tool auth zencoder-tool
```

At the prompt:
- Press **`s`** to skip the "open browser" step (required on headless containers — there is no `xdg-open`)
- **Paste the token** printed by the helper when asked `Paste your token:`
- IronClaw validates it against `GET https://api.zencoder.ai/api/v1/projects` and reports `✓` or `✗`


PowerShell manual method:

```powershell
$cid = Read-Host 'Client ID'
$sec = Read-Host 'Client Secret' -AsSecureString
$secPlain = [System.Net.NetworkCredential]::new('', $sec).Password
$body = @{ client_id = $cid; client_secret = $secPlain; grant_type = 'client_credentials' } | ConvertTo-Json -Compress
$resp = Invoke-RestMethod -Uri 'https://fe.zencoder.ai/oauth/token' -Method Post -ContentType 'application/json' -Body $body
$resp.access_token   # copy and paste into: ironclaw tool auth zencoder-tool
```

`cmd.exe` has no JSON-friendly quoting — use PowerShell, WSL, or the helper script.

To rotate after expiry (the JWT lives ~24h), re-run the helper script and repeat Step 3.

### 8. Verify the installation

```bash
ironclaw tool list
```

You should see `zencoder-tool` in the output. For detailed info:

```bash
ironclaw tool info zencoder-tool
```

## How It Works Inside IronClaw

### Automatic tool discovery — no skill required

IronClaw's agent automatically discovers all installed WASM tools via its **Tool Registry**. Once you run `ironclaw tool install`, the `zencoder-tool` is registered and the agent can call it immediately — **no additional skill, plugin, or configuration is needed**.

The tool exports three WIT interface functions that IronClaw reads at startup:

| Function | Purpose |
|---|---|
| `description()` | Short text explaining what the tool does — included in the agent's system prompt so it knows the tool exists |
| `schema()` | Full JSON Schema describing all 17 actions and their parameters — the LLM uses this to construct valid tool calls |
| `execute(request)` | Receives the action JSON, validates inputs, calls the Zencoder API, and returns the result |

When you ask IronClaw something that involves coding tasks, project management, or automation, the agent recognizes that `zencoder-tool` is relevant from its description and schema, constructs the appropriate JSON payload, and calls `execute()` — all transparently.

### Using the tool in chat

Talk to IronClaw in natural language. The agent translates your intent into the correct tool action automatically.

**Delegate a coding problem:**
```
You:  Fix the JWT expiration validation bug in the auth middleware
      of project 550e8400-e29b-41d4-a716-446655440000

Iron: I'll delegate this to Zencoder's AI agents.
      [calls zencoder-tool with action: solve_coding_problem]
      Task created: da1d251c-... — Zenflow agents are working on it.
```

**Check progress:**
```
You:  How's that JWT fix going?

Iron: [calls zencoder-tool with action: check_solution_status]
      Status: in_progress
      Plan: 3 of 5 steps completed
      Branch: fix/jwt-expiration-validation
```

**Browse your projects:**
```
You:  Show me my Zencoder projects

Iron: [calls zencoder-tool with action: list_projects]
      1. my-backend-api (550e8400-...)
      2. frontend-app (661f9511-...)
```

**Create an automation:**
```
You:  Set up a daily test run for my backend project at 9am on weekdays

Iron: [calls zencoder-tool with action: create_automation]
      Automation "Daily test run" created — runs Mon-Fri at 09:00.
```

**Manage tasks directly:**
```
You:  Create a task to refactor the database layer in my backend project

Iron: [calls zencoder-tool with action: create_task]
      Task "Refactor database layer" created and started.
```

### Optional: enhance with a skill

While not required, you can create an IronClaw **skill** to give the agent richer context about Zencoder workflows. Skills are `.md` files placed in `~/.ironclaw/skills/` that extend the agent's system prompt with domain-specific instructions.

Example `~/.ironclaw/skills/zencoder.SKILL.md`:

```markdown
# Zencoder Integration Skill

When the user asks to solve a coding problem, fix a bug, or implement a feature:
1. Use `list_projects` to find the relevant project if no project_id is given
2. Use `solve_coding_problem` to delegate the work to Zenflow agents
3. Proactively check progress with `check_solution_status` after a few minutes
4. Report the branch name so the user can review the changes

When the user asks about task progress:
1. Use `check_solution_status` to get the current state
2. Summarize the plan steps and highlight any that are stuck

For project management:
- Use `list_tasks` with status filters to show task overviews
- Use `update_task` to change task status when the user requests it
- Use `get_plan` to show detailed step breakdowns
```

This is entirely optional — the agent works with the tool out of the box. A skill just helps it make smarter decisions about when and how to use each action.

### Raw JSON payloads (advanced)

If you prefer to construct tool calls manually (e.g., via the HTTP webhook channel or for debugging), here are the raw JSON payloads the tool accepts:

**Delegate a coding problem:**
```json
{
  "action": "solve_coding_problem",
  "project_id": "550e8400-e29b-41d4-a716-446655440000",
  "description": "Fix the authentication middleware to properly validate JWT expiration"
}
```

**Check solution progress:**
```json
{
  "action": "check_solution_status",
  "project_id": "550e8400-e29b-41d4-a716-446655440000",
  "task_id": "da1d251c-0cea-4fe6-a744-ec2986035c35"
}
```

**List projects:**
```json
{
  "action": "list_projects"
}
```

**Create a task:**
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

**Create a scheduled automation:**
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

All 17 actions are documented in the tool's JSON schema (viewable with `ironclaw tool info zencoder-tool`).

### Known limitation

IronClaw's WIT-based auto-extraction of tool schemas from WASM binaries is currently stubbed (see [issue #649](https://github.com/nearai/ironclaw/issues/649)). This tool works around this by exporting the schema via the `schema()` function in the WIT `tool` interface. If your IronClaw version doesn't pick up the schema automatically, the agent may need a skill file (see above) to understand the tool's capabilities.

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

- The WASM sandbox never sees your access token — IronClaw injects it via credential injection
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
    lib.rs                            # All implementation (2000+ lines, 78 tests)
wit/
  tool.wit                            # near:agent@0.3.0 sandboxed-tool WIT interface
```

## License

MIT OR Apache-2.0

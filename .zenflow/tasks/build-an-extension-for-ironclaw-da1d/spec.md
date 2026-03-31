# Technical Specification: Zencoder WASM Tool for IronClaw

## 1. Technical Context

### 1.1 Language & Toolchain

- **Language**: Rust (edition 2021)
- **Target**: `wasm32-wasip1`
- **WIT interface**: `near:agent@0.3.0` (`sandboxed-tool` world)
- **IronClaw compatibility**: v0.23.0+

### 1.2 Dependencies (Cargo.toml)

| Crate | Version | Purpose |
|---|---|---|
| `serde` | 1.0 (features: `derive`) | JSON serialization/deserialization |
| `serde_json` | 1.0 | JSON parsing and generation |
| `wit-bindgen` | 0.41.0 | WIT binding code generation |

No additional crates (no `uuid`, `url`, or HTTP client crates). UUID validation is done via a hand-written function. URL construction uses custom percent-encoding helpers (same pattern as the GitHub tool). HTTP requests go through the host-provided `near::agent::host::http_request`.

### 1.3 Release Profile

```toml
[profile.release]
opt-level = "s"
lto = true
strip = true
codegen-units = 1
```

## 2. Architecture Overview

```
IronClaw Agent
    │
    ▼
┌─────────────────────────────────┐
│  zencoder-tool (WASM)           │
│  ┌───────────────────────────┐  │
│  │ execute(req) dispatcher   │  │
│  │   ├─ parse action JSON    │  │
│  │   ├─ validate inputs      │  │
│  │   └─ call api_request()   │  │
│  ├───────────────────────────┤  │
│  │ api_request()             │  │
│  │   ├─ build URL            │  │
│  │   ├─ host::http_request   │  │
│  │   ├─ retry (429/5xx)      │  │
│  │   ├─ parse envelope       │  │
│  │   └─ return data or error │  │
│  └───────────────────────────┘  │
└─────────────────────────────────┘
    │ (host boundary)
    ▼
IronClaw Host Runtime
    ├─ credential injection (Bearer token)
    ├─ endpoint allowlist check
    ├─ leak detection scan
    └─ HTTP to api.zencoder.ai
```

The tool follows the exact same architectural pattern as the existing GitHub tool:
1. A single `execute()` entry point deserializes JSON params into a tagged enum (`ZencoderAction`)
2. Each variant maps to a function that builds a URL, calls `api_request()`, and returns the result
3. `api_request()` handles HTTP method dispatch, retry logic, rate-limit monitoring, and response envelope unwrapping

## 3. Source Code Structure

```
zencoder-tool/
├── Cargo.toml
├── src/
│   └── lib.rs            # All tool logic in a single file (matches GitHub tool pattern)
└── zencoder-tool.capabilities.json
```

The WIT file is referenced from the IronClaw repository root at `../../wit/tool.wit` (same relative path as the GitHub tool). When building standalone, a local copy or symlink is used.

### 3.1 Module Organization (within lib.rs)

The single `lib.rs` file is organized into these sections:

1. **WIT bindings** - `wit_bindgen::generate!` macro
2. **Constants** - `MAX_TEXT_LENGTH`, `API_BASE_URL`
3. **Validation helpers** - `validate_input_length`, `validate_uuid`, `validate_status`
4. **URL encoding** - `url_encode_path`, `url_encode_query` (percent-encode all bytes except `A-Za-z0-9-_.`, no external crates)
5. **Action enum** - `ZencoderAction` with serde tagged dispatch
6. **Auth check** - `ensure_auth_configured() -> Result<(), String>` — calls `secret_exists("zencoder_api_key")`, returns `Ok(())` or setup instructions. Never reads or returns the secret value. Called at the top of `execute()` before action dispatch; if it returns `Err`, `execute()` returns immediately with the setup instructions string.
7. **Tool trait impl** - `execute()`, `schema()`, `description()`
8. **API client** - `api_request()` with retry and envelope parsing. Does NOT set an Authorization header — the host injects Bearer token via credential injection.
9. **Action handlers** - One function per action (e.g., `list_projects`, `create_task`)
10. **JSON schema** - `SCHEMA` const string
11. **Tests** - `#[cfg(test)]` module

## 4. Action Dispatch (ZencoderAction Enum)

```rust
#[derive(Deserialize)]
#[serde(tag = "action")]
enum ZencoderAction {
    // Project management
    #[serde(rename = "list_projects")]
    ListProjects,

    #[serde(rename = "get_project")]
    GetProject {
        project_id: String,
    },

    // Task management
    #[serde(rename = "create_task")]
    CreateTask {
        project_id: String,
        title: String,
        description: Option<String>,
        workflow_id: Option<String>,
        start: Option<bool>,
    },
    #[serde(rename = "list_tasks")]
    ListTasks {
        project_id: String,
        status: Option<String>,
        limit: Option<u32>,
    },
    #[serde(rename = "get_task")]
    GetTask {
        project_id: String,
        task_id: String,
    },
    #[serde(rename = "update_task")]
    UpdateTask {
        project_id: String,
        task_id: String,
        title: Option<String>,
        description: Option<String>,
        status: Option<String>,
    },

    // Workflow management
    #[serde(rename = "list_workflows")]
    ListWorkflows {
        project_id: Option<String>,
    },

    // Plan management
    #[serde(rename = "get_plan")]
    GetPlan {
        project_id: String,
        task_id: String,
    },
    #[serde(rename = "create_plan")]
    CreatePlan {
        project_id: String,
        task_id: String,
        steps: Vec<PlanStep>,
    },
    #[serde(rename = "update_plan_step")]
    UpdatePlanStep {
        project_id: String,
        task_id: String,
        step_id: String,
        status: Option<String>,
        name: Option<String>,
        description: Option<String>,
    },
    #[serde(rename = "add_plan_steps")]
    AddPlanSteps {
        project_id: String,
        task_id: String,
        steps: Vec<PlanStep>,
        after_step_id: Option<String>,
    },

    // Automation management
    #[serde(rename = "list_automations")]
    ListAutomations {
        enabled: Option<bool>,
    },
    #[serde(rename = "create_automation")]
    CreateAutomation {
        name: String,
        target_project_id: Option<String>,
        task_name: Option<String>,
        task_description: Option<String>,
        task_workflow: Option<String>,
        schedule_time: Option<String>,
        schedule_days_of_week: Option<Vec<u8>>,
    },
    #[serde(rename = "toggle_automation")]
    ToggleAutomation {
        automation_id: String,
        enabled: bool,
    },

    // Task automation (read-only for v1)
    #[serde(rename = "list_task_automations")]
    ListTaskAutomations {
        project_id: String,
        task_id: String,
    },

    // High-level convenience actions
    #[serde(rename = "solve_coding_problem")]
    SolveCodingProblem {
        project_id: String,
        description: String,
        workflow_id: Option<String>,
    },
    #[serde(rename = "check_solution_status")]
    CheckSolutionStatus {
        project_id: String,
        task_id: String,
    },
}
```

### 4.1 Supporting Types

```rust
#[derive(Deserialize, Serialize)]
struct PlanStep {
    name: String,
    description: String,
}

#[derive(Deserialize, Serialize)]
struct PlanStepSummary {
    name: String,
    status: String,
}

#[derive(Serialize)]
struct SolutionStatus {
    task_status: String,
    plan_steps: Vec<PlanStepSummary>,
    progress: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
}
```

## 5. API Client Layer

### 5.1 Base URL

```rust
const API_BASE_URL: &str = "https://api.zencoder.ai";
```

### 5.2 Request Function

```rust
fn api_request(method: &str, path: &str, body: Option<String>) -> Result<String, String>
```

Behavior (mirrors GitHub tool pattern):
1. Constructs full URL: `format!("{}{}", API_BASE_URL, path)`
2. Sets headers: `Content-Type: application/json`, `User-Agent: IronClaw-Zencoder-Tool`
3. Authorization header is injected by the host via credential injection (never touched by WASM)
4. Retry loop: up to 3 total attempts (1 initial + up to 2 retries) for status 429 or 5xx. Retries happen without delay — the WASM runtime does not provide a sleep mechanism (`poll_oneoff` is not available in IronClaw's WASI preview 1). A delay should be added if the host exposes a sleep function in the future. The retry is primarily useful for transient 5xx errors; for 429, the `Retry-After` header value (if present) is logged as a warning and included in the error message returned after all retries are exhausted.
5. On success (2xx): returns the response body as a string
6. Rate-limit monitoring: after each response, check for `X-RateLimit-Remaining` (or `x-ratelimit-remaining`, case-insensitive) in response headers. If present and the value parses to an integer < 10, log a warning via `near::agent::host::log(Warn, ...)`. Also check for `Retry-After` header on 429 responses and include its value in the error message.

### 5.3 Response Envelope

The Zencoder API returns responses in an envelope format. The tool returns the raw JSON response to the caller (IronClaw's agent), letting the LLM parse the structured response. This avoids unnecessary re-serialization and keeps the tool simple.

For the `check_solution_status` convenience action, the tool makes two API calls (get task + get plan) and assembles a summary response.

## 6. API Endpoint Mapping

### 6.1 API Discovery (Implementation Pre-requisite)

**API Discovery Results (completed):**

1. `GET https://api.zencoder.ai/openapi.json` → HTTP 401 (endpoint exists, requires auth)
2. `GET https://api.zencoder.ai/api/v1/openapi.json` → HTTP 401 (endpoint exists, requires auth)
3. `GET https://api.zencoder.ai/swagger.json` → HTTP 401 (endpoint exists, requires auth)
4. Zencoder CLI (`zencoder`) is not available in this environment
5. Zencoder docs reference an OpenAPI spec at `https://docs.zencoder.ai/api-reference/openapi.json` but it is behind Cloudflare protection and returns a 404 after challenge bypass

**Conclusions:**
- The REST API at `api.zencoder.ai` is confirmed to exist (401 responses, not 404)
- The `/api/v1/` path prefix is confirmed (401 on `/api/v1/openapi.json`)
- No public OpenAPI spec could be retrieved without authentication
- The provisional endpoint table below remains the best-guess mapping based on Zenflow MCP tool signatures
- The `task_id` field in creation responses is assumed to be at `data.id` (standard REST pattern)
- The branch field in task responses is assumed to be `branch` (default)
- `list_workflows` is assumed to support both global (`/api/v1/workflows`) and project-scoped (`/api/v1/projects/{project_id}/workflows`) paths
- `toggle_automation` is assumed to use `POST` method

### 6.2 Endpoint Table

All endpoints are relative to `API_BASE_URL`. These paths are based on Zenflow MCP tool signatures and confirmed API existence (401 responses).

| Action | Method | Path |
|---|---|---|
| `list_projects` | GET | `/api/v1/projects` |
| `get_project` | GET | `/api/v1/projects/{project_id}` |
| `create_task` | POST | `/api/v1/projects/{project_id}/tasks` |
| `list_tasks` | GET | `/api/v1/projects/{project_id}/tasks?status={s}&limit={n}` |
| `get_task` | GET | `/api/v1/projects/{project_id}/tasks/{task_id}` |
| `update_task` | PATCH | `/api/v1/projects/{project_id}/tasks/{task_id}` |
| `list_workflows` (global) | GET | `/api/v1/workflows` (when `project_id` is omitted) |
| `list_workflows` (project) | GET | `/api/v1/projects/{project_id}/workflows` (when `project_id` is provided) |
| `get_plan` | GET | `/api/v1/projects/{project_id}/tasks/{task_id}/plan` |
| `create_plan` | POST | `/api/v1/projects/{project_id}/tasks/{task_id}/plan` |
| `update_plan_step` | PATCH | `/api/v1/projects/{project_id}/tasks/{task_id}/plan/steps/{step_id}` |
| `add_plan_steps` | POST | `/api/v1/projects/{project_id}/tasks/{task_id}/plan/steps` |
| `list_automations` | GET | `/api/v1/automations?enabled={bool}` |
| `create_automation` | POST | `/api/v1/automations` |
| `toggle_automation` | POST | `/api/v1/automations/{automation_id}/toggle` |
| `list_task_automations` | GET | `/api/v1/projects/{project_id}/tasks/{task_id}/automations` |

## 7. Input Validation

### 7.1 UUID Validation

All `project_id`, `task_id`, `step_id`, and `automation_id` fields are validated against UUID v4 format before use in URL paths:

```rust
fn validate_uuid(s: &str, field_name: &str) -> Result<(), String> {
    if s.len() != 36 { return Err(...); }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 { return Err(...); }
    if parts.iter().any(|p| !p.chars().all(|c| c.is_ascii_hexdigit())) {
        return Err(...);
    }
    // Validate segment lengths: 8-4-4-4-12
    let expected = [8, 4, 4, 4, 12];
    for (part, &len) in parts.iter().zip(&expected) {
        if part.len() != len { return Err(...); }
    }
    Ok(())
}
```

### 7.2 String Length Validation

Reuses `validate_input_length` from GitHub tool pattern (max 65536 chars) for:
- `title`, `description`, `name` fields
- `task_name`, `task_description` in automation creation

Required string fields (`title` in `create_task`, `name` in `create_automation`) must also be validated as non-empty after trimming. Return an error like `"title must not be empty"` if the trimmed value is zero-length.

### 7.2.1 Non-Empty Array Validation

`create_plan` and `add_plan_steps` must reject an empty `steps` array with a clear error message ("at least one step is required") before making the API call.

### 7.2.2 At-Least-One-Field Validation

`update_plan_step` and `update_task` have all-optional body fields. If all optional fields are `None`, return an error (e.g., `"update_plan_step requires at least one of: status, name, description"`) before making the API call. This prevents sending a PATCH with an empty body.

### 7.3 Status Validation

Task status values are validated against the allowed set:

```rust
fn validate_task_status(s: &str) -> Result<(), String> {
    match s {
        "todo" | "inprogress" | "inreview" | "done" | "cancelled" => Ok(()),
        _ => Err(format!("Invalid status: '{}'. Must be one of: todo, inprogress, inreview, done, cancelled", s)),
    }
}
```

Plan step status validation:

```rust
fn validate_step_status(s: &str) -> Result<(), String> {
    match s {
        "Pending" | "InProgress" | "Completed" | "Skipped" => Ok(()),
        _ => Err(format!("Invalid step status: '{}'. Must be one of: Pending, InProgress, Completed, Skipped", s)),
    }
}
```

### 7.4 Schedule Validation

```rust
fn validate_schedule_time(s: &str) -> Result<(), String> {
    if !s.is_ascii() || s.len() != 5 {
        return Err(format!("Invalid schedule_time '{}': must be HH:MM format (ASCII)", s));
    }
    let bytes = s.as_bytes();
    if bytes[2] != b':' {
        return Err(format!("Invalid schedule_time '{}': must be HH:MM format", s));
    }
    let hour: u32 = s[..2].parse().map_err(|_| format!("Invalid hour in '{}'", s))?;
    let minute: u32 = s[3..5].parse().map_err(|_| format!("Invalid minute in '{}'", s))?;
    if hour > 23 {
        return Err(format!("Invalid hour {}: must be 0-23", hour));
    }
    if minute > 59 {
        return Err(format!("Invalid minute {}: must be 0-59", minute));
    }
    Ok(())
}

fn validate_days_of_week(days: &[u8]) -> Result<(), String> {
    for &d in days {
        if d > 6 {
            return Err(format!("Invalid day_of_week {}: must be 0 (Sun) - 6 (Sat)", d));
        }
    }
    Ok(())
}
```

### 7.5 Workflow ID Validation

The `workflow_id` field in `CreateTask` and `task_workflow` in `CreateAutomation` accept both UUID strings and workflow slugs (e.g., `"default-auto-workflow"`, `"fix_bug"`, `"full_sdd"`). Apply only length validation (`validate_input_length`), not UUID format validation.

### 7.6 Path Segment Safety

All UUID values used in URL paths are first validated as UUIDs (which inherently contain only hex digits and hyphens), then percent-encoded via `url_encode_path` for defense-in-depth.

## 8. Convenience Actions

### 8.1 solve_coding_problem

Composes multiple API calls into a single high-level action:

1. Validate `project_id` (UUID), validate `description` is non-empty after trimming (return error `"description must not be empty"` if zero-length), and validate `description` length
2. Derive title: truncate `description` to the first word boundary at or before 100 characters, append `"..."` if truncated. If description is <= 100 chars, use it as-is. Uses `char_indices` to avoid panics on multi-byte UTF-8 boundaries.
   ```rust
   fn derive_title(description: &str) -> String {
       let max_len = 100;
       if description.chars().count() <= max_len {
           return description.to_string();
       }
       let byte_limit = description
           .char_indices()
           .nth(max_len)
           .map(|(idx, _)| idx)
           .unwrap_or(description.len());
       let truncated = &description[..byte_limit];
       match truncated.rfind(' ') {
           Some(pos) if pos > 10 => format!("{}...", &truncated[..pos]),
           _ => format!("{}...", truncated),
       }
   }
   ```
3. `POST /api/v1/projects/{project_id}/tasks` with:
   - `title`: derived title (see above)
   - `description`: full description
   - `workflow_id`: user-specified or `"default-auto-workflow"`
   - `start`: `true`
4. Parse response to extract `task_id`. The exact JSON field path must be confirmed during API discovery (Section 6.1) — it may be `data.id`, `data.task_id`, or another path within the `{"success", "data", "message"}` envelope. Default assumption: `data.id`.
5. Return JSON with `task_id` and status message

### 8.2 check_solution_status

Composes task status and plan into a summary:

1. Validate `project_id` and `task_id` (UUIDs)
2. `GET /api/v1/projects/{project_id}/tasks/{task_id}` to get task details
3. `GET /api/v1/projects/{project_id}/tasks/{task_id}/plan` to get plan steps. If the plan GET returns a non-2xx response (e.g., 404 for a newly created task with no plan yet), treat it as an empty plan: set `plan_steps` to `[]` and `progress` to `"0 of 0 steps completed"` rather than propagating the error.
4. Extract `branch` from the task response JSON. The exact field name must be confirmed during API discovery (Section 6.1). If the field name cannot be determined, default to `branch`. If the field is absent or null, `branch` is omitted from the summary via `#[serde(skip_serializing_if = "Option::is_none")]`.
5. Assemble summary JSON:
   ```json
   {
     "task_status": "inprogress",
     "plan_steps": [
       {"name": "...", "status": "Completed"},
       {"name": "...", "status": "InProgress"}
     ],
     "progress": "2 of 4 steps completed",
     "branch": "task/da1d251c"
   }
   ```
   The `branch` field is nullable — omitted when the task has no associated branch.

## 9. Capabilities Configuration

File: `zencoder-tool.capabilities.json`

```json
{
  "version": "0.2.1",
  "wit_version": "0.3.0",
  "description": "Manage Zencoder/Zenflow projects, tasks, plans, workflows, and automations. Delegate complex coding problems to Zenflow's AI agents and track their progress.",
  "capabilities": {
    "http": {
      "allowlist": [
        {
          "host": "api.zencoder.ai",
          "path_prefix": "/api/v1/",
          "methods": ["GET", "POST", "PATCH"]
        }
      ],
      "credentials": {
        "zencoder_api_key": {
          "secret_name": "zencoder_api_key",
          "location": {
            "type": "bearer"
          },
          "host_patterns": ["api.zencoder.ai"]
        }
      },
      "rate_limit": {
        "requests_per_minute": 60,
        "requests_per_hour": 1000
      }
    },
    "secrets": {
      "allowed_names": ["zencoder_api_key"]
    }
  },
  "auth": {
    "secret_name": "zencoder_api_key",
    "display_name": "Zencoder",
    "instructions": "Get your Zencoder API key from the Zencoder dashboard (Settings > API Keys), then paste it here.",
    "setup_url": "https://app.zencoder.ai/settings",
    "token_hint": "Starts with 'zen_'",
    "env_var": "ZENCODER_API_KEY"
  },
  "setup": {
    "required_secrets": [
      {
        "name": "zencoder_api_key",
        "prompt": "Zencoder API Key (get one from app.zencoder.ai/settings)"
      }
    ]
  }
}
```

### 9.1 Security Properties

- **Endpoint allowlist**: Only `api.zencoder.ai` with path prefix `/api/v1/` and methods GET/POST/PATCH
- **No DELETE or PUT**: DELETE and PUT methods are intentionally omitted — no planned endpoint uses them, and omitting them reduces attack surface
- **Credential injection**: `zencoder_api_key` secret is injected as Bearer token only for `api.zencoder.ai`
- **Secret isolation**: WASM code only checks `secret_exists("zencoder_api_key")`, never reads the value
- **Rate limiting**: 60 req/min, 1000 req/hour to prevent abuse

## 10. JSON Schema (Tool Parameter Schema)

The `schema()` function returns a JSON Schema describing all actions using `oneOf` (same pattern as GitHub tool). Each action variant is a separate schema object within the `oneOf` array, with `action` as the discriminator field.

The schema covers all 17 actions defined in Section 4, with:
- Required fields clearly marked
- Optional fields with defaults documented
- Enum constraints for status values
- String type for UUIDs with description noting format requirement
- Array types for `steps` and `schedule_days_of_week`

## 11. Error Handling

### 11.1 Error Categories

| Category | Handling |
|---|---|
| Invalid params JSON | Return error from serde deserialization |
| Missing API key | Return setup instructions (same as GitHub tool) |
| Validation failure | Return field-specific error message |
| HTTP 4xx (not 429) | Return status code + response body |
| HTTP 429 | 3 total attempts (1 initial + up to 2 retries), then return rate-limit error |
| HTTP 5xx | 3 total attempts (1 initial + up to 2 retries), then return server error |
| Network error | 3 total attempts (1 initial + up to 2 retries), then return connection error |
| Invalid response body | Return UTF-8 decode error |

### 11.2 Error Message Safety

- Error messages never include the API key or token values
- Error messages include the HTTP status code and response body for debugging
- Internal URLs are shown (api.zencoder.ai is not sensitive) to aid troubleshooting

## 12. Logging

Uses `near::agent::host::log()` at appropriate levels:

| Level | When |
|---|---|
| `Warn` | Rate limit running low (remaining < 10) |
| `Warn` | Retrying after transient error |
| `Error` | Final failure after max retries |
| `Info` | Convenience action progress (e.g., "Task created, checking status...") |

## 13. Build Process

```bash
# Prerequisites
rustup target add wasm32-wasip1

# Build
cd zencoder-tool
cargo build --target wasm32-wasip1 --release

# Output: target/wasm32-wasip1/release/zencoder_tool.wasm
```

### 13.1 Installation

```bash
# From source
ironclaw tool install ./zencoder-tool/

# From registry (after publishing)
ironclaw registry install tools/zencoder
```

## 14. Verification Approach

### 14.1 Unit Tests

Located in `src/lib.rs` under `#[cfg(test)]`:

- **UUID validation**: valid/invalid formats, edge cases (wrong length, wrong segment count, non-hex chars)
- **Input length validation**: boundary conditions (exactly at limit, one over)
- **Status validation**: valid/invalid values for task and step statuses
- **URL encoding**: special characters, spaces, unicode
- **Schedule validation**: valid/invalid time formats (`"25:00"`, `"12:60"`, `"1:30"`, `"ab:cd"`), day values (0-6 valid, 7+ invalid)
- **Title derivation**: `derive_title` with short input (<= 100), long input with word boundary, long input with no spaces
- **Action deserialization**: verify serde correctly parses each action variant from JSON

### 14.2 Build Verification

```bash
# Lint
cargo clippy --target wasm32-wasip1 --all --all-features

# Format check
cargo fmt --check

# Run unit tests (native target)
cargo test

# Build WASM
cargo build --target wasm32-wasip1 --release
```

### 14.3 Integration Testing

Manual testing with a live Zencoder API key:

1. Install tool: `ironclaw tool install ./zencoder-tool/`
2. Set secret: `ironclaw secret set zencoder_api_key <key>`
3. Run IronClaw and invoke each action
4. Verify correct API calls and response parsing

## 15. Security Checklist

- [ ] No secrets handled in WASM code (only `secret_exists` check)
- [ ] All UUIDs validated before URL interpolation
- [ ] All string inputs length-bounded
- [ ] URL path segments percent-encoded
- [ ] HTTP allowlist restricted to `api.zencoder.ai` only
- [ ] No DELETE or PUT methods in allowlist
- [ ] Rate limits configured in capabilities
- [ ] Error messages contain no credential data
- [ ] Status enum values validated before use
- [ ] Schedule values range-checked
- [ ] No `unsafe` code
- [ ] No panic paths in production code (all errors returned as `Result`)

# Product Requirements Document: Zencoder WASM Tool for IronClaw

## 1. Overview

Build a WASM-based IronClaw extension ("zencoder-tool") that connects IronClaw's agent to Zencoder/Zenflow's full task management and AI coding capabilities via Zencoder's HTTP API. The tool runs inside IronClaw's secure WASM sandbox, inheriting credential injection, endpoint allowlisting, and leak detection.

## 2. Problem Statement

IronClaw users currently have no way to leverage Zencoder/Zenflow's AI-driven coding intelligence (spec-driven development, multi-agent orchestration, task planning) from within their IronClaw assistant. This extension bridges that gap, enabling IronClaw to delegate complex coding problems to Zenflow and manage the full task lifecycle.

## 3. Target Users

- IronClaw users who also use Zencoder/Zenflow for coding workflows
- Teams wanting to combine IronClaw's always-on personal assistant with Zenflow's structured development workflows

## 4. Functional Requirements

### 4.1 Project Management

- **List projects**: Retrieve all Zenflow projects accessible to the authenticated user
- **Get project details**: Fetch project configuration, repository info, and settings

### 4.2 Task Management

- **Create task**: Create a new Zenflow task with title, description, workflow selection, and optional auto-start
- **List tasks**: List tasks in a project with optional status filtering (todo, inprogress, inreview, done, cancelled) and pagination (limit)
- **Update task**: Modify task title, description, or status
- **Get task status**: Check current state and progress of a specific task

### 4.3 Workflow Management

- **List workflows**: Retrieve available workflows for a project (Quick Change, Fix Bug, Spec and Build, Full SDD, custom workflows)

### 4.4 Plan Management

- **Get plan**: Retrieve the structured implementation plan for a task (steps, statuses, descriptions)
- **Create plan**: Define a multi-step implementation plan for a task
- **Update plan step**: Change step status (Pending, InProgress, Completed, Skipped), name, or description
- **Add plan steps**: Append or insert additional steps into an existing plan

### 4.5 Automation Management

- **List automations**: Retrieve scheduled automations with optional enabled/disabled filtering
- **Create automation**: Set up recurring task-creation automations with schedule configuration
- **Toggle automation**: Enable or disable an automation

### 4.6 Coding Problem Solving (Primary Use Case)

- **Solve coding problem**: A high-level convenience action that:
  1. Creates a new Zenflow task with the problem description
  2. Selects an appropriate workflow (auto or user-specified)
  3. Starts the task immediately
  4. Returns the task ID for subsequent status polling
- **Check solution status**: Poll a task for completion and retrieve results, which include:
  - Current task status (todo, inprogress, inreview, done, cancelled)
  - Plan step summaries with their statuses (Pending, InProgress, Completed, Skipped)
  - The task's linked branch name (if any)
  - Human-readable progress summary (e.g., "3 of 5 steps completed")

## 5. Authentication

### 5.1 API Key Authentication

- User stores a Zencoder API key as an IronClaw secret: `ironclaw secret set zencoder_api_key <key>`
- The host injects the API key as a Bearer token on outbound HTTP requests to the Zencoder API
- The WASM tool never sees or handles the raw key
- Credential injection is configured via the `capabilities.json` file using the same pattern as [IronClaw's GitHub tool](https://github.com/nearai/ironclaw/blob/main/tools-src/github/github-tool.capabilities.json):
  - `capabilities.http.credentials.zencoder_api_key` maps the secret to a `bearer` token
  - `capabilities.http.credentials.zencoder_api_key.host_patterns` restricts injection to Zencoder API domains only
  - `capabilities.secrets.allowed_names` explicitly lists `zencoder_api_key`
  - `auth` block provides `secret_name`, `display_name`, `instructions`, and `setup_url` for the setup flow
  - `setup.required_secrets` lists the API key with a user-facing prompt

### 5.2 OAuth Authentication (Future)

- Support OAuth2 flow via `ironclaw tool auth zencoder-tool`
- Token stored securely in IronClaw's secret store
- Token refresh handled transparently by the host credential injection layer

### 5.3 Initial Release

- API key authentication is required for v1
- OAuth support is a future enhancement (documented but not blocking release)

## 6. Security Requirements

### 6.1 Endpoint Allowlisting

- HTTP requests restricted exclusively to Zencoder's API domain(s)
- No wildcard domains; only specific Zencoder API base URL(s) permitted

### 6.2 Credential Protection

- API key/token never exposed to WASM code; injected at host boundary
- All outbound requests and inbound responses scanned for secret leakage

### 6.3 Input Validation

- All user-provided strings (task titles, descriptions, project IDs) validated for length (max 65536 chars)
- Path segments validated to prevent injection (no `..`, `/`, `?`, `#` in IDs)
- UUID format validation for project_id, task_id, step_id, automation_id parameters

### 6.4 Rate Limiting

- Per-tool rate limits configured in capabilities (requests per minute/hour)
- Respect and surface Zencoder API rate limit headers to users

### 6.5 Error Handling

- No sensitive information (tokens, internal URLs) in error messages
- Structured error responses with actionable user-facing messages
- Retry logic for transient errors (429, 5xx) with bounded attempts

## 7. Installability

### 7.1 Registry Distribution

- Published to the IronClaw extension registry as `tools/zencoder`
- Installable via: `ironclaw registry install tools/zencoder`

### 7.2 Manual Installation

- Installable from source: `ironclaw tool install ./zencoder-tool/`
- Pre-built `.wasm` binary available for download

### 7.3 Setup Flow

- After installation: `ironclaw tool setup zencoder-tool` prompts for API key
- `ironclaw tool auth zencoder-tool` initiates OAuth flow (when implemented)

## 8. Non-Functional Requirements

### 8.1 Performance

- WASM binary size target: under 500KB if feasible (release-optimized, LTO, stripped); this is a best-effort goal — actual size depends on HTTP client, JSON, and UUID dependencies compiled into the WASM binary
- API response timeout: 30 seconds default, configurable via host
- Simple retry logic (up to 3 attempts) for transient failures

### 8.2 Compatibility

- Targets `wasm32-wasip1` (IronClaw's WASM runtime)
- Uses WIT interface `near:agent@0.3.0` (tool.wit)
- Compatible with IronClaw v0.23.0+

### 8.3 Observability

- Structured logging via host `log()` function at appropriate levels
- Rate limit warnings when remaining calls are low
- Error logging for all failed API calls

## 9. Scope Boundaries (v1)

### 9.1 Out of Scope

- Streaming/SSE support for real-time task execution monitoring
- Direct file transfer between IronClaw workspace and Zenflow tasks
- Webhook handling for Zencoder events
- OAuth2 authentication (documented for v2)
- Task flow (task automation) create/update/toggle (write operations deferred to v2)
- Browser/subagent capabilities

### 9.2 Partial Inclusions

- **List task flows** (read-only): Retrieve task flows attached to a specific task, supporting the polling use case in Section 4.6. Write operations (create/update/toggle) are deferred to v2.

## 10. Assumptions

- Zencoder exposes an HTTP REST API with endpoints for projects, tasks, plans, workflows, and automations; the API base URL must be configurable (defaulting to `https://api.zencoder.ai`), with support for local development endpoints determined in the Technical Specification
- API responses follow the `{"success": bool, "data": ..., "error_data": ..., "message": ...}` envelope format
- API key authentication follows Bearer token convention
- The Zencoder API accepts and returns JSON
- IronClaw's WASM host runtime supports the `near:agent@0.3.0` WIT interface
- The tool's `capabilities.json` follows the same structure as [IronClaw's GitHub tool](https://github.com/nearai/ironclaw/blob/main/tools-src/github/github-tool.capabilities.json) (http allowlist, credentials, secrets, auth, setup blocks)

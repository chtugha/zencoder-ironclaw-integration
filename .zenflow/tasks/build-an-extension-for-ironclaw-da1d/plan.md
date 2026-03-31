# Full SDD workflow

## Configuration
- **Artifacts Path**: `.zenflow/tasks/build-an-extension-for-ironclaw-da1d`

---

## Agent Instructions

---

## Workflow Steps

### [x] Step: Requirements
<!-- chat-id: b4edebdd-65e0-4cb9-bf21-bec15f26550c -->

Create a Product Requirements Document (PRD) based on the feature description.

1. Review existing codebase to understand current architecture and patterns
2. Analyze the feature definition and identify unclear aspects
3. Ask the user for clarifications on aspects that significantly impact scope or user experience
4. Make reasonable decisions for minor details based on context and conventions
5. If user can't clarify, make a decision, state the assumption, and continue

Focus on **what** the feature should do and **why**, not **how** it should be built. Do not include technical implementation details, technology choices, or code-level decisions — those belong in the Technical Specification.

Save the PRD to `.zenflow/tasks/build-an-extension-for-ironclaw-da1d/requirements.md`.

### [x] Step: Technical Specification
<!-- chat-id: 28a46188-2ed1-4d29-963a-8629422e07d9 -->

Create a technical specification based on the PRD in `.zenflow/tasks/build-an-extension-for-ironclaw-da1d/requirements.md`.

1. Review existing codebase architecture and identify reusable components
2. Define the implementation approach

Do not include implementation steps, phases, or task breakdowns — those belong in the Planning step.

Save to `.zenflow/tasks/build-an-extension-for-ironclaw-da1d/spec.md` with:
- Technical context (language, dependencies)
- Implementation approach referencing existing code patterns
- Source code structure changes
- Data model / API / interface changes
- Verification approach using project lint/test commands

### [x] Step: Planning
<!-- chat-id: 45521e9d-1909-401e-be1d-92c8c4879569 -->

Create a detailed implementation plan based on `.zenflow/tasks/build-an-extension-for-ironclaw-da1d/spec.md`.

Implementation tasks are below, replacing the generic "Implementation" step.

### [x] Step: Project Scaffolding & API Discovery
<!-- chat-id: 7bf6760e-7327-4aa1-87ab-c80ca0d17411 -->

Set up the Rust WASM project structure and discover/confirm the Zencoder HTTP API endpoints.

- [ ] Create `zencoder-tool/Cargo.toml` matching the GitHub tool pattern: `serde` 1.0 (derive), `serde_json` 1.0, `wit-bindgen` 0.41.0, `crate-type = ["cdylib"]`, release profile with `opt-level = "s"`, `lto = true`, `strip = true`, `codegen-units = 1`
- [ ] Create `zencoder-tool/zencoder-tool.capabilities.json` per spec Section 9: HTTP allowlist for `api.zencoder.ai` (GET/POST/PATCH only — no PUT or DELETE), path prefix `/api/v1/`, bearer credential injection for `zencoder_api_key`, rate limits (60/min, 1000/hr), auth/setup blocks
- [ ] Set up WIT file: create a local copy or symlink of `wit/tool.wit` (the `near:agent@0.3.0` sandboxed-tool world) so the project builds standalone
- [ ] Create `zencoder-tool/src/lib.rs` with initial `wit_bindgen::generate!` macro and empty struct
- [ ] Attempt Zencoder API discovery: try `GET https://api.zencoder.ai/openapi.json`, `GET https://api.zencoder.ai/api/v1/openapi.json`, `GET https://api.zencoder.ai/swagger.json`. If no spec is found, use the Zencoder CLI `zencoder` command with verbose/debug flags to observe actual HTTP request paths. If neither yields results, fall back to the provisional endpoint table from spec Section 6.2 and note that endpoints are best-guess
- [ ] Update `.zenflow/tasks/build-an-extension-for-ironclaw-da1d/spec.md` Section 6.2 if any endpoint paths are discovered to differ from the provisional table
- [ ] During API discovery, also confirm: (a) the exact JSON field name for a task's branch (is it `branch`, `branch_name`, or something else?), (b) whether `list_workflows` uses `/api/v1/workflows` vs `/api/v1/projects/{id}/workflows`, (c) the HTTP method for `toggle_automation` (POST vs PATCH), (d) the JSON field path for `task_id` in a task creation response (e.g., is it `data.id`, `data.task_id`, or another path within the `{"success", "data", "message"}` envelope?). Document findings in spec Sections 6.2 and 8.1
- [ ] Ensure `.gitignore` includes `target/`, `*.wasm`, and other Rust build artifacts
- [ ] Verify: `cargo check --target wasm32-wasip1` succeeds with the empty scaffolding

### [ ] Step: Core Infrastructure & Validation

Implement the foundational code in `zencoder-tool/src/lib.rs`: constants, all validation helpers, URL encoding, the API client function, the `ZencoderAction` enum definition (all 17 variants), supporting types, and the `Tool` trait implementation (execute dispatcher, schema placeholder, description). Include unit tests for all validation and encoding functions.

Reference: spec Sections 3–5, 7.

- [ ] Constants: `MAX_TEXT_LENGTH` (65536), `API_BASE_URL` ("https://api.zencoder.ai")
- [ ] `validate_input_length(s, field_name) -> Result<(), String>` per spec Section 7.2
- [ ] `validate_uuid(s, field_name) -> Result<(), String>` per spec Section 7.1 — check length 36, 5 dash-separated segments, hex-only chars, segment lengths 8-4-4-4-12
- [ ] `validate_task_status(s) -> Result<(), String>` per spec Section 7.3 — allow "todo", "inprogress", "inreview", "done", "cancelled"
- [ ] `validate_step_status(s) -> Result<(), String>` per spec Section 7.3 — allow "Pending", "InProgress", "Completed", "Skipped"
- [ ] `validate_schedule_time(s) -> Result<(), String>` per spec Section 7.4 — HH:MM format, ASCII, hour 0-23, minute 0-59
- [ ] `validate_days_of_week(days) -> Result<(), String>` per spec Section 7.4 — each value 0-6
- [ ] `url_encode_path(s) -> String` — percent-encode all bytes except `A-Z`, `a-z`, `0-9`, `-`, `_`, `.`; encode each other byte as `%XX` using uppercase hex. Implemented from scratch (no external crates). `url_encode_query(s) -> String` — identical logic (same as the GitHub tool's implementation)
- [ ] `ZencoderAction` enum with all 17 variants (spec Section 4) using `#[serde(tag = "action")]`
- [ ] Supporting types: `PlanStep`, `PlanStepSummary`, `SolutionStatus` (spec Section 4.1)
- [ ] `api_request(method, path, body) -> Result<String, String>` per spec Section 5.2: construct URL, set headers (Content-Type, User-Agent), retry loop (3 total attempts: 1 initial + up to 2 retries for 429/5xx, no delay — WASM has no sleep), rate-limit header monitoring (warn if remaining < 10), return body on 2xx or error
- [ ] `ensure_auth_configured() -> Result<(), String>` — call `secret_exists("zencoder_api_key")`, return setup instructions if missing. Returns unit `()`, never a token value. `api_request()` must NOT set an Authorization header — the IronClaw host injects the Bearer token automatically via credential injection
- [ ] Tool trait impl: `execute()` calls `ensure_auth_configured()` first (return early with setup instructions on Err), then dispatches to stub handlers; `schema()` returns placeholder; `description()` returns tool description string
- [ ] Unit tests: UUID validation (valid, wrong length, wrong segments, non-hex, wrong segment lengths), input length (at limit, over limit), status validation (valid/invalid for both task and step), schedule validation (valid/invalid times, valid/invalid days), URL encoding (special chars, spaces, unicode), action deserialization (at least 3 representative variants)
- [ ] Verify: `cargo test` passes, `cargo check --target wasm32-wasip1` succeeds

### [ ] Step: Action Handlers — Projects, Tasks & Workflows

Implement the handler functions for project, task, and workflow management actions (7 actions). Each handler validates inputs, constructs the API path, calls `api_request()`, and returns the result.

Reference: spec Section 6.2 endpoint table.

- [ ] `list_projects()` — `GET /api/v1/projects`
- [ ] `get_project(project_id)` — validate UUID, `GET /api/v1/projects/{project_id}`
- [ ] `create_task(project_id, title, description, workflow_id, start)` — validate UUID, reject empty title (after trim), validate lengths, build JSON body, `POST /api/v1/projects/{project_id}/tasks`
- [ ] `list_tasks(project_id, status, limit)` — validate UUID, validate status if present, `GET /api/v1/projects/{project_id}/tasks?status={s}&limit={n}`
- [ ] `get_task(project_id, task_id)` — validate UUIDs, `GET /api/v1/projects/{project_id}/tasks/{task_id}`
- [ ] `update_task(project_id, task_id, title, description, status)` — validate UUIDs, reject if all of title/description/status are None ("requires at least one of: title, description, status"), validate status if present, validate lengths, `PATCH /api/v1/projects/{project_id}/tasks/{task_id}`
- [ ] `list_workflows(project_id)` — if `project_id` is provided: validate UUID, `GET /api/v1/projects/{project_id}/workflows`; if omitted: `GET /api/v1/workflows`. Both paths must be tried during API discovery (Step 1) and the correct mapping confirmed before implementation
- [ ] Wire all handlers into the `execute()` match arms (replacing stubs)
- [ ] Verify: `cargo test` passes, `cargo check --target wasm32-wasip1` succeeds

### [ ] Step: Action Handlers — Plans & Automations

Implement the handler functions for plan management (4 actions), automation management (3 actions), and task automation (1 action).

Reference: spec Sections 6.2, 7.

- [ ] `get_plan(project_id, task_id)` — validate UUIDs, `GET /api/v1/projects/{project_id}/tasks/{task_id}/plan`
- [ ] `create_plan(project_id, task_id, steps)` — validate UUIDs, reject empty `steps` array with a clear error ("at least one step is required"), validate step names/descriptions length, `POST /api/v1/projects/{project_id}/tasks/{task_id}/plan`
- [ ] `update_plan_step(project_id, task_id, step_id, status, name, description)` — validate UUIDs, reject if all of status/name/description are None ("requires at least one of: status, name, description"), validate step status if present, validate lengths, `PATCH /api/v1/projects/{project_id}/tasks/{task_id}/plan/steps/{step_id}`
- [ ] `add_plan_steps(project_id, task_id, steps, after_step_id)` — validate UUIDs, reject empty `steps` array, validate step names/descriptions, `POST /api/v1/projects/{project_id}/tasks/{task_id}/plan/steps`
- [ ] `list_automations(enabled)` — `GET /api/v1/automations?enabled={bool}`
- [ ] `create_automation(name, target_project_id, task_name, task_description, task_workflow, schedule_time, schedule_days_of_week)` — reject empty name (after trim), validate lengths, validate schedule_time/days if present, validate UUID for target_project_id if present, `POST /api/v1/automations`
- [ ] `toggle_automation(automation_id, enabled)` — validate UUID, `POST /api/v1/automations/{automation_id}/toggle`
- [ ] `list_task_automations(project_id, task_id)` — validate UUIDs, `GET /api/v1/projects/{project_id}/tasks/{task_id}/automations`
- [ ] Wire all handlers into the `execute()` match arms
- [ ] Verify: `cargo test` passes, `cargo check --target wasm32-wasip1` succeeds

### [ ] Step: Convenience Actions, JSON Schema & Comprehensive Tests

Implement the two high-level convenience actions, the complete JSON schema, and comprehensive unit tests for all action deserialization and the `derive_title` helper.

Reference: spec Sections 8, 10, 14.1.

- [ ] `derive_title(description) -> String` per spec Section 8.1: truncate at first word boundary at or before 100 characters (by char count, not bytes), append "..." if truncated, handle multi-byte UTF-8 safely with `char_indices`
- [ ] `solve_coding_problem(project_id, description, workflow_id)` per spec Section 8.1: validate UUID, reject empty description (after trim), validate description length, derive title, POST create_task with `start: true`, extract `task_id` from response, return JSON with `task_id` and status message
- [ ] `check_solution_status(project_id, task_id)` per spec Section 8.2: validate UUIDs, GET task details, GET plan (treat non-2xx/404 as empty plan with `[]` steps and `"0 of 0 steps completed"`), extract branch field name (determined during API discovery in Step 1 — default to `branch` if unknown), count completed steps, assemble `SolutionStatus` JSON
- [ ] Wire convenience action handlers into `execute()` match arms
- [ ] Complete `SCHEMA` const string: JSON Schema with `oneOf` array covering all 17 actions, required fields, enum constraints for status values, descriptions for UUID fields
- [ ] Unit tests for `derive_title`: short input (<= 100 chars), long input with word boundary, long input with no spaces near boundary, multi-byte UTF-8 input
- [ ] Unit tests for action deserialization: every `ZencoderAction` variant parses correctly from JSON
- [ ] Verify: `cargo test` passes, `cargo check --target wasm32-wasip1` succeeds

### [ ] Step: Build Verification & Security Review

Final build, lint, format check, WASM compilation, and security audit against the spec's checklist.

- [ ] `cargo fmt --check` — fix any formatting issues
- [ ] `cargo clippy --target wasm32-wasip1 --all --all-features` — fix any warnings
- [ ] `cargo test` — all unit tests pass
- [ ] `cargo build --target wasm32-wasip1 --release` — WASM binary compiles successfully
- [ ] Check WASM binary size (`target/wasm32-wasip1/release/zencoder_tool.wasm`) — report size, note if over 500KB target
- [ ] Security checklist (spec Section 15):
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
- [ ] Record all verification results in this plan

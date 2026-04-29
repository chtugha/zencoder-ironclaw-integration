wit_bindgen::generate!({
    world: "sandboxed-tool",
    path: "../wit/tool.wit",
});

use serde::Deserialize;

const MAX_TEXT_LENGTH: usize = 65536;
const API_BASE_URL: &str = "https://api.zencoder.ai";

const MAX_HTTP_ATTEMPTS: u32 = 3;
const HTTP_TIMEOUT_MS: u32 = 30_000;
const RATE_LIMIT_WARN_THRESHOLD: u32 = 10;
const ERROR_BODY_PREVIEW_CHARS: usize = 500;

const UUID_LEN: usize = 36;
const UUID_SEGMENT_LENS: [usize; 5] = [8, 4, 4, 4, 12];

const SCHEDULE_TIME_LEN: usize = 5;
const HOUR_MAX: u32 = 23;
const MINUTE_MAX: u32 = 59;
const DAY_OF_WEEK_MAX: u8 = 6;

const TITLE_MAX_CHARS: usize = 100;
const TITLE_MIN_WORD_BREAK: usize = 10;

const DEFAULT_WORKFLOW_ID: &str = "default-auto-workflow";

fn validate_input_length(s: &str, field_name: &str) -> Result<(), String> {
    if s.chars().count() > MAX_TEXT_LENGTH {
        return Err(format!(
            "Input '{}' exceeds maximum length of {} characters",
            field_name, MAX_TEXT_LENGTH
        ));
    }
    Ok(())
}

fn validate_uuid(s: &str, field_name: &str) -> Result<(), String> {
    if s.len() != UUID_LEN {
        return Err(format!(
            "Invalid {}: expected {} characters, got {}",
            field_name,
            UUID_LEN,
            s.len()
        ));
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != UUID_SEGMENT_LENS.len() {
        return Err(format!(
            "Invalid {}: expected {} dash-separated segments, got {}",
            field_name,
            UUID_SEGMENT_LENS.len(),
            parts.len()
        ));
    }
    for (part, &len) in parts.iter().zip(&UUID_SEGMENT_LENS) {
        if part.len() != len {
            return Err(format!(
                "Invalid {}: segment '{}' has length {}, expected {}",
                field_name,
                part,
                part.len(),
                len
            ));
        }
        if !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!(
                "Invalid {}: segment '{}' contains non-hex characters",
                field_name, part
            ));
        }
    }
    Ok(())
}

fn validate_task_status(s: &str) -> Result<(), String> {
    match s {
        "todo" | "inprogress" | "inreview" | "done" | "cancelled" => Ok(()),
        _ => Err(format!(
            "Invalid status: '{}'. Must be one of: todo, inprogress, inreview, done, cancelled",
            s
        )),
    }
}

fn validate_step_status(s: &str) -> Result<(), String> {
    match s {
        "Pending" | "InProgress" | "Completed" | "Skipped" => Ok(()),
        _ => Err(format!(
            "Invalid step status: '{}'. Must be one of: Pending, InProgress, Completed, Skipped",
            s
        )),
    }
}

fn validate_schedule_time(s: &str) -> Result<(), String> {
    if !s.is_ascii() || s.len() != SCHEDULE_TIME_LEN {
        return Err(format!(
            "Invalid schedule_time '{}': must be HH:MM format (ASCII)",
            s
        ));
    }
    let bytes = s.as_bytes();
    if bytes[2] != b':' {
        return Err(format!(
            "Invalid schedule_time '{}': must be HH:MM format",
            s
        ));
    }
    let hour: u32 = s[..2]
        .parse()
        .map_err(|_| format!("Invalid hour in '{}'", s))?;
    let minute: u32 = s[3..5]
        .parse()
        .map_err(|_| format!("Invalid minute in '{}'", s))?;
    if hour > HOUR_MAX {
        return Err(format!("Invalid hour {}: must be 0-{}", hour, HOUR_MAX));
    }
    if minute > MINUTE_MAX {
        return Err(format!(
            "Invalid minute {}: must be 0-{}",
            minute, MINUTE_MAX
        ));
    }
    Ok(())
}

fn validate_days_of_week(days: &[u8]) -> Result<(), String> {
    if days.is_empty() {
        return Err("schedule_days_of_week must contain at least one day".into());
    }
    for &d in days {
        if d > DAY_OF_WEEK_MAX {
            return Err(format!(
                "Invalid day_of_week {}: must be 0 (Sun) - {} (Sat)",
                d, DAY_OF_WEEK_MAX
            ));
        }
    }
    Ok(())
}

fn url_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(char::from(b"0123456789ABCDEF"[(b >> 4) as usize]));
                out.push(char::from(b"0123456789ABCDEF"[(b & 0xf) as usize]));
            }
        }
    }
    out
}

// Intentionally identical to url_encode_path: query params use percent-encoding (not +)
// for Zencoder API compatibility. Kept as separate function for call-site clarity.
fn url_encode_query(s: &str) -> String {
    url_encode_path(s)
}

#[derive(Deserialize)]
#[serde(tag = "action")]
enum ZencoderAction {
    #[serde(rename = "list_projects")]
    ListProjects,

    #[serde(rename = "get_project")]
    GetProject { project_id: String },

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
    GetTask { project_id: String, task_id: String },

    #[serde(rename = "update_task")]
    UpdateTask {
        project_id: String,
        task_id: String,
        title: Option<String>,
        description: Option<String>,
        status: Option<String>,
    },

    #[serde(rename = "list_workflows")]
    ListWorkflows { project_id: Option<String> },

    #[serde(rename = "get_plan")]
    GetPlan { project_id: String, task_id: String },

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

    #[serde(rename = "list_automations")]
    ListAutomations { enabled: Option<bool> },

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

    #[serde(rename = "list_task_automations")]
    ListTaskAutomations { project_id: String, task_id: String },

    #[serde(rename = "solve_coding_problem")]
    SolveCodingProblem {
        project_id: String,
        description: String,
        workflow_id: Option<String>,
    },

    #[serde(rename = "check_solution_status")]
    CheckSolutionStatus { project_id: String, task_id: String },
}

#[derive(Deserialize, serde::Serialize)]
struct PlanStep {
    name: String,
    description: String,
}

#[derive(serde::Serialize)]
struct PlanStepSummary {
    name: String,
    status: String,
}

#[derive(serde::Serialize)]
struct SolutionStatus {
    task_status: String,
    plan_steps: Vec<PlanStepSummary>,
    progress: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
}

struct ZencoderTool;

impl exports::near::agent::tool::Guest for ZencoderTool {
    fn execute(req: exports::near::agent::tool::Request) -> exports::near::agent::tool::Response {
        match execute_inner(&req.params) {
            Ok(result) => exports::near::agent::tool::Response {
                output: Some(result),
                error: None,
            },
            Err(e) => exports::near::agent::tool::Response {
                output: None,
                error: Some(e),
            },
        }
    }

    fn schema() -> String {
        SCHEMA.to_string()
    }

    fn description() -> String {
        "Zencoder/Zenflow integration for managing projects, tasks, plans, workflows, and \
         automations. Delegate complex coding problems to Zenflow's AI agents and track their \
         progress. Authentication uses a Zencoder JWT access token — run the bundled helper \
         (`scripts/zencoder-auth.sh` / `.ps1`) to exchange your Client ID and Client Secret \
         for a JWT, then paste it into IronClaw via \
         'ironclaw tool auth zencoder-tool'."
            .to_string()
    }
}

fn ensure_auth_configured() -> Result<(), String> {
    if near::agent::host::secret_exists("zencoder_access_token") {
        return Ok(());
    }

    Err(AUTH_NOT_CONFIGURED_ERROR.into())
}

const AUTH_NOT_CONFIGURED_ERROR: &str = "Zencoder access token not configured.

1. Generate a personal access token at https://auth.zencoder.ai
   (Administration > Settings > Personal Tokens) — copy the Client ID
   and Client Secret immediately (the secret is shown only once).

2. Run the bundled helper to exchange them for a JWT:

     scripts/zencoder-auth.sh           # bash / zsh / WSL
     scripts\\zencoder-auth.ps1          # PowerShell

   The helper prints the JWT to your terminal.

3. Paste the JWT into IronClaw:

     ironclaw tool auth zencoder-tool

   At the prompt, press 's' to skip the browser step (headless containers),
   then paste the token when asked 'Paste your token:'.";

fn api_request(method: &str, path: &str, body: Option<String>) -> Result<String, String> {
    let url = format!("{}{}", API_BASE_URL, path);

    let headers = if body.is_some() {
        serde_json::json!({
            "Content-Type": "application/json",
            "User-Agent": "IronClaw-Zencoder-Tool"
        })
    } else {
        serde_json::json!({
            "User-Agent": "IronClaw-Zencoder-Tool"
        })
    };

    let body_bytes = body.map(|b| b.into_bytes());

    // Only retry methods that the Zencoder API treats as idempotent. Retrying
    // POST/PATCH on a transport error or 5xx can produce duplicate writes
    // (e.g. two created tasks, two scheduled automations) because the server
    // may have already accepted the first request before the response was
    // lost. Without a sleep primitive in WASM we also cannot back off, which
    // makes the duplicate-write window microseconds wide.
    let is_idempotent = matches!(method, "GET" | "HEAD" | "OPTIONS");
    let max_attempts: u32 = if is_idempotent { MAX_HTTP_ATTEMPTS } else { 1 };
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;

        let response = near::agent::host::http_request(
            method,
            &url,
            &headers.to_string(),
            body_bytes.as_deref(),
            Some(HTTP_TIMEOUT_MS),
        );

        match response {
            Ok(resp) => {
                if let Ok(headers_json) =
                    serde_json::from_str::<serde_json::Value>(&resp.headers_json)
                {
                    if let Some(remaining) = headers_json
                        .get("x-ratelimit-remaining")
                        .or_else(|| headers_json.get("X-RateLimit-Remaining"))
                        .and_then(|v| v.as_str())
                    {
                        if let Ok(count) = remaining.parse::<u32>() {
                            if count < RATE_LIMIT_WARN_THRESHOLD {
                                near::agent::host::log(
                                    near::agent::host::LogLevel::Warn,
                                    &format!("Zencoder API rate limit low: {} remaining", count),
                                );
                            }
                        }
                    }
                }

                if resp.status >= 200 && resp.status < 300 {
                    return String::from_utf8(resp.body)
                        .map_err(|e| format!("Invalid UTF-8 in response: {}", e));
                } else if resp.status == 429 {
                    let mut err_msg =
                        "Zencoder API rate limited (429). Cannot retry without delay in WASM."
                            .to_string();
                    if let Ok(headers_json) =
                        serde_json::from_str::<serde_json::Value>(&resp.headers_json)
                    {
                        if let Some(retry_after) = headers_json
                            .get("retry-after")
                            .or_else(|| headers_json.get("Retry-After"))
                            .and_then(|v| v.as_str())
                        {
                            err_msg = format!("{} Retry after {}s.", err_msg, retry_after);
                        }
                    }
                    return Err(err_msg);
                } else if is_idempotent && attempt < max_attempts && resp.status >= 500 {
                    let msg = format!(
                        "Zencoder API error {} (attempt {}/{}). Retrying...",
                        resp.status, attempt, max_attempts
                    );
                    near::agent::host::log(near::agent::host::LogLevel::Warn, &msg);
                    continue;
                } else if resp.status == 401 {
                    return Err(
                        "Zencoder API returned 401 Unauthorized. Your access token may have \
                         expired.\n\
                         \n\
                         Re-run the bundled helper to get a fresh JWT:\n\
                         \n  scripts/zencoder-auth.sh        # bash / zsh / WSL\n\
                         \n  scripts\\zencoder-auth.ps1       # PowerShell\n\
                         \n\
                         Then paste it into IronClaw:\n\
                         \n  ironclaw tool auth zencoder-tool\n\
                         \n\
                         (Press 's' to skip the browser step on headless containers.)"
                            .into(),
                    );
                } else {
                    let body_preview: String = String::from_utf8_lossy(&resp.body)
                        .chars()
                        .take(ERROR_BODY_PREVIEW_CHARS)
                        .collect();
                    let err_msg = if body_preview.is_empty() {
                        format!(
                            "Zencoder API returned status {}. Check your credentials and parameters.",
                            resp.status
                        )
                    } else {
                        format!(
                            "Zencoder API returned status {}: {}",
                            resp.status, body_preview
                        )
                    };
                    return Err(err_msg);
                }
            }
            Err(e) => {
                if is_idempotent && attempt < max_attempts {
                    near::agent::host::log(
                        near::agent::host::LogLevel::Warn,
                        &format!(
                            "HTTP request failed: {} (attempt {}/{}). Retrying...",
                            e, attempt, max_attempts
                        ),
                    );
                    continue;
                }
                return Err(format!(
                    "HTTP request failed after {} attempt(s) [{}]: {}",
                    attempt, method, e
                ));
            }
        }
    }
}

fn execute_inner(params: &str) -> Result<String, String> {
    ensure_auth_configured()?;

    let action: ZencoderAction =
        serde_json::from_str(params).map_err(|e| format!("Invalid parameters: {}", e))?;

    match action {
        ZencoderAction::ListProjects => handle_list_projects(),
        ZencoderAction::GetProject { project_id } => handle_get_project(project_id),
        ZencoderAction::CreateTask {
            project_id,
            title,
            description,
            workflow_id,
            start,
        } => handle_create_task(project_id, title, description, workflow_id, start),
        ZencoderAction::ListTasks {
            project_id,
            status,
            limit,
        } => handle_list_tasks(project_id, status, limit),
        ZencoderAction::GetTask {
            project_id,
            task_id,
        } => handle_get_task(project_id, task_id),
        ZencoderAction::UpdateTask {
            project_id,
            task_id,
            title,
            description,
            status,
        } => handle_update_task(project_id, task_id, title, description, status),
        ZencoderAction::ListWorkflows { project_id } => handle_list_workflows(project_id),
        ZencoderAction::GetPlan {
            project_id,
            task_id,
        } => handle_get_plan(project_id, task_id),
        ZencoderAction::CreatePlan {
            project_id,
            task_id,
            steps,
        } => handle_create_plan(project_id, task_id, steps),
        ZencoderAction::UpdatePlanStep {
            project_id,
            task_id,
            step_id,
            status,
            name,
            description,
        } => handle_update_plan_step(project_id, task_id, step_id, status, name, description),
        ZencoderAction::AddPlanSteps {
            project_id,
            task_id,
            steps,
            after_step_id,
        } => handle_add_plan_steps(project_id, task_id, steps, after_step_id),
        ZencoderAction::ListAutomations { enabled } => handle_list_automations(enabled),
        ZencoderAction::CreateAutomation {
            name,
            target_project_id,
            task_name,
            task_description,
            task_workflow,
            schedule_time,
            schedule_days_of_week,
        } => handle_create_automation(
            name,
            target_project_id,
            task_name,
            task_description,
            task_workflow,
            schedule_time,
            schedule_days_of_week,
        ),
        ZencoderAction::ToggleAutomation {
            automation_id,
            enabled,
        } => handle_toggle_automation(automation_id, enabled),
        ZencoderAction::ListTaskAutomations {
            project_id,
            task_id,
        } => handle_list_task_automations(project_id, task_id),
        ZencoderAction::SolveCodingProblem {
            project_id,
            description,
            workflow_id,
        } => handle_solve_coding_problem(project_id, description, workflow_id),
        ZencoderAction::CheckSolutionStatus {
            project_id,
            task_id,
        } => handle_check_solution_status(project_id, task_id),
    }
}

fn handle_list_projects() -> Result<String, String> {
    api_request("GET", "/api/v1/projects", None)
}

fn handle_get_project(project_id: String) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    let path = format!("/api/v1/projects/{}", url_encode_path(&project_id));
    api_request("GET", &path, None)
}

fn handle_create_task(
    project_id: String,
    title: String,
    description: Option<String>,
    workflow_id: Option<String>,
    start: Option<bool>,
) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    let trimmed_title = title.trim();
    if trimmed_title.is_empty() {
        return Err("title must not be empty".into());
    }
    validate_input_length(trimmed_title, "title")?;
    if let Some(ref d) = description {
        validate_input_length(d, "description")?;
    }
    if let Some(ref w) = workflow_id {
        validate_input_length(w, "workflow_id")?;
    }

    let mut body = serde_json::json!({ "title": trimmed_title });
    if let Some(d) = description {
        body["description"] = serde_json::Value::String(d);
    }
    if let Some(w) = workflow_id {
        body["workflow_id"] = serde_json::Value::String(w);
    }
    if let Some(s) = start {
        body["start"] = serde_json::Value::Bool(s);
    }

    let path = format!("/api/v1/projects/{}/tasks", url_encode_path(&project_id));
    api_request("POST", &path, Some(body.to_string()))
}

fn handle_list_tasks(
    project_id: String,
    status: Option<String>,
    limit: Option<u32>,
) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    if let Some(ref s) = status {
        validate_task_status(s)?;
    }

    let mut path = format!("/api/v1/projects/{}/tasks", url_encode_path(&project_id));

    let mut params: Vec<String> = Vec::new();
    if let Some(ref s) = status {
        params.push(format!("status={}", url_encode_query(s)));
    }
    if let Some(n) = limit {
        params.push(format!("limit={}", n));
    }
    if !params.is_empty() {
        path.push('?');
        path.push_str(&params.join("&"));
    }

    api_request("GET", &path, None)
}

fn handle_get_task(project_id: String, task_id: String) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    validate_uuid(&task_id, "task_id")?;
    let path = format!(
        "/api/v1/projects/{}/tasks/{}",
        url_encode_path(&project_id),
        url_encode_path(&task_id)
    );
    api_request("GET", &path, None)
}

fn handle_update_task(
    project_id: String,
    task_id: String,
    title: Option<String>,
    description: Option<String>,
    status: Option<String>,
) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    validate_uuid(&task_id, "task_id")?;

    if title.is_none() && description.is_none() && status.is_none() {
        return Err("update_task requires at least one of: title, description, status".into());
    }

    if let Some(ref s) = status {
        validate_task_status(s)?;
    }

    let mut body = serde_json::Map::new();
    if let Some(t) = title {
        let trimmed = t.trim().to_string();
        if trimmed.is_empty() {
            return Err("title must not be empty".into());
        }
        validate_input_length(&trimmed, "title")?;
        body.insert("title".into(), serde_json::Value::String(trimmed));
    }
    if let Some(d) = description {
        validate_input_length(&d, "description")?;
        body.insert("description".into(), serde_json::Value::String(d));
    }
    if let Some(s) = status {
        body.insert("status".into(), serde_json::Value::String(s));
    }

    let path = format!(
        "/api/v1/projects/{}/tasks/{}",
        url_encode_path(&project_id),
        url_encode_path(&task_id)
    );
    api_request(
        "PATCH",
        &path,
        Some(serde_json::Value::Object(body).to_string()),
    )
}

fn handle_list_workflows(project_id: Option<String>) -> Result<String, String> {
    let path = match project_id {
        Some(pid) => {
            validate_uuid(&pid, "project_id")?;
            format!("/api/v1/projects/{}/workflows", url_encode_path(&pid))
        }
        None => "/api/v1/workflows".to_string(),
    };
    api_request("GET", &path, None)
}

fn handle_get_plan(project_id: String, task_id: String) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    validate_uuid(&task_id, "task_id")?;
    let path = format!(
        "/api/v1/projects/{}/tasks/{}/plan",
        url_encode_path(&project_id),
        url_encode_path(&task_id)
    );
    api_request("GET", &path, None)
}

fn handle_create_plan(
    project_id: String,
    task_id: String,
    steps: Vec<PlanStep>,
) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    validate_uuid(&task_id, "task_id")?;
    if steps.is_empty() {
        return Err("at least one step is required".into());
    }
    let steps: Vec<PlanStep> = steps
        .into_iter()
        .map(|s| PlanStep {
            name: s.name.trim().to_string(),
            description: s.description,
        })
        .collect();
    for step in &steps {
        if step.name.is_empty() {
            return Err("step name must not be empty".into());
        }
        validate_input_length(&step.name, "step name")?;
        validate_input_length(&step.description, "step description")?;
    }
    let body = serde_json::json!({ "steps": steps });
    let path = format!(
        "/api/v1/projects/{}/tasks/{}/plan",
        url_encode_path(&project_id),
        url_encode_path(&task_id)
    );
    api_request("POST", &path, Some(body.to_string()))
}

fn handle_update_plan_step(
    project_id: String,
    task_id: String,
    step_id: String,
    status: Option<String>,
    name: Option<String>,
    description: Option<String>,
) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    validate_uuid(&task_id, "task_id")?;
    validate_uuid(&step_id, "step_id")?;

    if status.is_none() && name.is_none() && description.is_none() {
        return Err("update_plan_step requires at least one of: status, name, description".into());
    }

    if let Some(ref s) = status {
        validate_step_status(s)?;
    }

    let mut body = serde_json::Map::new();
    if let Some(s) = status {
        body.insert("status".into(), serde_json::Value::String(s));
    }
    if let Some(n) = name {
        let n = n.trim().to_string();
        if n.is_empty() {
            return Err("name must not be empty".into());
        }
        validate_input_length(&n, "name")?;
        body.insert("name".into(), serde_json::Value::String(n));
    }
    if let Some(d) = description {
        validate_input_length(&d, "description")?;
        body.insert("description".into(), serde_json::Value::String(d));
    }

    let path = format!(
        "/api/v1/projects/{}/tasks/{}/plan/steps/{}",
        url_encode_path(&project_id),
        url_encode_path(&task_id),
        url_encode_path(&step_id)
    );
    api_request(
        "PATCH",
        &path,
        Some(serde_json::Value::Object(body).to_string()),
    )
}

fn handle_add_plan_steps(
    project_id: String,
    task_id: String,
    steps: Vec<PlanStep>,
    after_step_id: Option<String>,
) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    validate_uuid(&task_id, "task_id")?;
    if steps.is_empty() {
        return Err("at least one step is required".into());
    }
    let steps: Vec<PlanStep> = steps
        .into_iter()
        .map(|s| PlanStep {
            name: s.name.trim().to_string(),
            description: s.description,
        })
        .collect();
    for step in &steps {
        if step.name.is_empty() {
            return Err("step name must not be empty".into());
        }
        validate_input_length(&step.name, "step name")?;
        validate_input_length(&step.description, "step description")?;
    }
    if let Some(ref id) = after_step_id {
        validate_uuid(id, "after_step_id")?;
    }

    let mut body = serde_json::json!({ "steps": steps });
    if let Some(id) = after_step_id {
        body["after_step_id"] = serde_json::Value::String(id);
    }

    let path = format!(
        "/api/v1/projects/{}/tasks/{}/plan/steps",
        url_encode_path(&project_id),
        url_encode_path(&task_id)
    );
    api_request("POST", &path, Some(body.to_string()))
}

fn handle_list_automations(enabled: Option<bool>) -> Result<String, String> {
    let path = match enabled {
        Some(e) => format!("/api/v1/automations?enabled={}", e),
        None => "/api/v1/automations".to_string(),
    };
    api_request("GET", &path, None)
}

fn handle_create_automation(
    name: String,
    target_project_id: Option<String>,
    task_name: Option<String>,
    task_description: Option<String>,
    task_workflow: Option<String>,
    schedule_time: Option<String>,
    schedule_days_of_week: Option<Vec<u8>>,
) -> Result<String, String> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err("name must not be empty".into());
    }
    validate_input_length(trimmed_name, "name")?;
    if let Some(ref pid) = target_project_id {
        validate_uuid(pid, "target_project_id")?;
    }
    let task_name = task_name.map(|tn| tn.trim().to_string());
    if let Some(ref tn) = task_name {
        if tn.is_empty() {
            return Err("task_name must not be empty".into());
        }
        validate_input_length(tn, "task_name")?;
    }
    if let Some(ref td) = task_description {
        validate_input_length(td, "task_description")?;
    }
    if let Some(ref tw) = task_workflow {
        validate_input_length(tw, "task_workflow")?;
    }
    if let Some(ref st) = schedule_time {
        validate_schedule_time(st)?;
    }
    if let Some(ref days) = schedule_days_of_week {
        validate_days_of_week(days)?;
    }

    let mut body = serde_json::json!({ "name": trimmed_name });
    if let Some(pid) = target_project_id {
        body["target_project_id"] = serde_json::Value::String(pid);
    }
    if let Some(tn) = task_name {
        body["task_name"] = serde_json::Value::String(tn);
    }
    if let Some(td) = task_description {
        body["task_description"] = serde_json::Value::String(td);
    }
    if let Some(tw) = task_workflow {
        body["task_workflow"] = serde_json::Value::String(tw);
    }
    if let Some(st) = schedule_time {
        body["schedule_time"] = serde_json::Value::String(st);
    }
    if let Some(days) = schedule_days_of_week {
        body["schedule_days_of_week"] =
            serde_json::Value::Array(days.into_iter().map(|d| serde_json::json!(d)).collect());
    }

    api_request("POST", "/api/v1/automations", Some(body.to_string()))
}

fn handle_toggle_automation(automation_id: String, enabled: bool) -> Result<String, String> {
    validate_uuid(&automation_id, "automation_id")?;
    let body = serde_json::json!({ "enabled": enabled });
    let path = format!(
        "/api/v1/automations/{}/toggle",
        url_encode_path(&automation_id)
    );
    api_request("POST", &path, Some(body.to_string()))
}

fn handle_list_task_automations(project_id: String, task_id: String) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    validate_uuid(&task_id, "task_id")?;
    let path = format!(
        "/api/v1/projects/{}/tasks/{}/automations",
        url_encode_path(&project_id),
        url_encode_path(&task_id)
    );
    api_request("GET", &path, None)
}

fn derive_title(description: &str) -> String {
    if description.chars().count() <= TITLE_MAX_CHARS {
        return description.to_string();
    }
    let byte_limit = description
        .char_indices()
        .nth(TITLE_MAX_CHARS)
        .map(|(idx, _)| idx)
        .unwrap_or(description.len());
    let truncated = &description[..byte_limit];
    match truncated.rfind(' ') {
        // Only break at word boundary if it keeps a reasonable title length.
        Some(pos) if pos > TITLE_MIN_WORD_BREAK => format!("{}...", &truncated[..pos]),
        _ => format!("{}...", truncated),
    }
}

fn handle_solve_coding_problem(
    project_id: String,
    description: String,
    workflow_id: Option<String>,
) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return Err("description must not be empty".into());
    }
    validate_input_length(trimmed, "description")?;
    if let Some(ref w) = workflow_id {
        validate_input_length(w, "workflow_id")?;
    }

    let title = derive_title(trimmed);
    let wf = workflow_id.unwrap_or_else(|| DEFAULT_WORKFLOW_ID.to_string());

    let body = serde_json::json!({
        "title": title,
        "description": trimmed,
        "workflow_id": wf,
        "start": true
    });

    let path = format!("/api/v1/projects/{}/tasks", url_encode_path(&project_id));
    let resp = api_request("POST", &path, Some(body.to_string()))?;

    let parsed: serde_json::Value =
        serde_json::from_str(&resp).map_err(|e| format!("Failed to parse response: {}", e))?;

    let task_id = parsed
        .get("data")
        .and_then(|d| d.get("id"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            parsed
                .get("data")
                .and_then(|d| d.get("task_id"))
                .and_then(|v| v.as_str())
        })
        .or_else(|| parsed.get("id").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            "Failed to extract task_id from API response. Response format may have changed."
                .to_string()
        })?;

    let result = serde_json::json!({
        "task_id": task_id,
        "message": format!("Task created and started. Track progress with check_solution_status using task_id: {}", task_id)
    });
    Ok(result.to_string())
}

fn handle_check_solution_status(project_id: String, task_id: String) -> Result<String, String> {
    validate_uuid(&project_id, "project_id")?;
    validate_uuid(&task_id, "task_id")?;

    let task_path = format!(
        "/api/v1/projects/{}/tasks/{}",
        url_encode_path(&project_id),
        url_encode_path(&task_id)
    );
    let task_resp = api_request("GET", &task_path, None)?;
    let task_json: serde_json::Value = serde_json::from_str(&task_resp)
        .map_err(|e| format!("Failed to parse task response: {}", e))?;

    let task_data = task_json.get("data").unwrap_or(&task_json);
    let task_status = match task_data.get("status").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => {
            near::agent::host::log(
                near::agent::host::LogLevel::Warn,
                "Task response missing 'status' field — API response shape may have changed",
            );
            "unparseable".to_string()
        }
    };

    let branch = task_data
        .get("branch")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let plan_path = format!(
        "/api/v1/projects/{}/tasks/{}/plan",
        url_encode_path(&project_id),
        url_encode_path(&task_id)
    );
    let (plan_steps, progress) = match api_request("GET", &plan_path, None) {
        Ok(plan_resp) => {
            let plan_json: serde_json::Value = match serde_json::from_str(&plan_resp) {
                Ok(v) => v,
                Err(e) => {
                    near::agent::host::log(
                        near::agent::host::LogLevel::Warn,
                        &format!(
                            "Failed to parse plan response JSON (treating as empty): {}",
                            e
                        ),
                    );
                    serde_json::json!({})
                }
            };
            let plan_data = plan_json.get("data").unwrap_or(&plan_json);
            let steps_arr = plan_data
                .get("steps")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let summaries: Vec<PlanStepSummary> = steps_arr
                .iter()
                .map(|s| PlanStepSummary {
                    name: s
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    status: s
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Pending")
                        .to_string(),
                })
                .collect();

            let total = summaries.len();
            let completed = summaries.iter().filter(|s| s.status == "Completed").count();
            let progress = format!("{} of {} steps completed", completed, total);
            (summaries, progress)
        }
        Err(e) => {
            near::agent::host::log(
                near::agent::host::LogLevel::Warn,
                &format!("Failed to fetch plan (returning unavailable status): {}", e),
            );
            (Vec::new(), format!("plan unavailable: {}", e))
        }
    };

    let status = SolutionStatus {
        task_status,
        plan_steps,
        progress,
        branch,
    };
    serde_json::to_string(&status).map_err(|e| format!("Failed to serialize status: {}", e))
}

const SCHEMA: &str = r#"{
  "type": "object",
  "required": ["action"],
  "oneOf": [
    {
      "properties": {
        "action": { "const": "list_projects" }
      },
      "required": ["action"],
      "additionalProperties": false,
      "description": "List all available projects"
    },
    {
      "properties": {
        "action": { "const": "get_project" },
        "project_id": { "type": "string", "description": "Project UUID" }
      },
      "required": ["action", "project_id"],
      "additionalProperties": false,
      "description": "Get details of a specific project"
    },
    {
      "properties": {
        "action": { "const": "create_task" },
        "project_id": { "type": "string", "description": "Project UUID" },
        "title": { "type": "string", "description": "Task title" },
        "description": { "type": "string", "description": "Task description" },
        "workflow_id": { "type": "string", "description": "Workflow ID (UUID) or slug (e.g. 'default-auto-workflow'). Both formats accepted." },
        "start": { "type": "boolean", "description": "Start execution immediately" }
      },
      "required": ["action", "project_id", "title"],
      "additionalProperties": false,
      "description": "Create a new task in a project"
    },
    {
      "properties": {
        "action": { "const": "list_tasks" },
        "project_id": { "type": "string", "description": "Project UUID" },
        "status": { "type": "string", "enum": ["todo", "inprogress", "inreview", "done", "cancelled"], "description": "Filter by status" },
        "limit": { "type": "integer", "description": "Max tasks to return" }
      },
      "required": ["action", "project_id"],
      "additionalProperties": false,
      "description": "List tasks in a project"
    },
    {
      "properties": {
        "action": { "const": "get_task" },
        "project_id": { "type": "string", "description": "Project UUID" },
        "task_id": { "type": "string", "description": "Task UUID" }
      },
      "required": ["action", "project_id", "task_id"],
      "additionalProperties": false,
      "description": "Get details of a specific task"
    },
    {
      "properties": {
        "action": { "const": "update_task" },
        "project_id": { "type": "string", "description": "Project UUID" },
        "task_id": { "type": "string", "description": "Task UUID" },
        "title": { "type": "string", "description": "New title" },
        "description": { "type": "string", "description": "New description" },
        "status": { "type": "string", "enum": ["todo", "inprogress", "inreview", "done", "cancelled"], "description": "New status" }
      },
      "required": ["action", "project_id", "task_id"],
      "additionalProperties": false,
      "description": "Update a task (at least one of title, description, status required)"
    },
    {
      "properties": {
        "action": { "const": "list_workflows" },
        "project_id": { "type": "string", "description": "Project UUID (omit for global workflows)" }
      },
      "required": ["action"],
      "additionalProperties": false,
      "description": "List available workflows"
    },
    {
      "properties": {
        "action": { "const": "get_plan" },
        "project_id": { "type": "string", "description": "Project UUID" },
        "task_id": { "type": "string", "description": "Task UUID" }
      },
      "required": ["action", "project_id", "task_id"],
      "additionalProperties": false,
      "description": "Get the structured plan for a task"
    },
    {
      "properties": {
        "action": { "const": "create_plan" },
        "project_id": { "type": "string", "description": "Project UUID" },
        "task_id": { "type": "string", "description": "Task UUID" },
        "steps": { "type": "array", "items": { "type": "object", "properties": { "name": { "type": "string" }, "description": { "type": "string" } }, "required": ["name", "description"] }, "minItems": 1, "description": "Plan steps" }
      },
      "required": ["action", "project_id", "task_id", "steps"],
      "additionalProperties": false,
      "description": "Create a structured plan for a task"
    },
    {
      "properties": {
        "action": { "const": "update_plan_step" },
        "project_id": { "type": "string", "description": "Project UUID" },
        "task_id": { "type": "string", "description": "Task UUID" },
        "step_id": { "type": "string", "description": "Step UUID" },
        "status": { "type": "string", "enum": ["Pending", "InProgress", "Completed", "Skipped"], "description": "New step status" },
        "name": { "type": "string", "description": "New step name" },
        "description": { "type": "string", "description": "New step description" }
      },
      "required": ["action", "project_id", "task_id", "step_id"],
      "additionalProperties": false,
      "description": "Update a plan step (at least one of status, name, description required)"
    },
    {
      "properties": {
        "action": { "const": "add_plan_steps" },
        "project_id": { "type": "string", "description": "Project UUID" },
        "task_id": { "type": "string", "description": "Task UUID" },
        "steps": { "type": "array", "items": { "type": "object", "properties": { "name": { "type": "string" }, "description": { "type": "string" } }, "required": ["name", "description"] }, "minItems": 1, "description": "Steps to add" },
        "after_step_id": { "type": "string", "description": "Insert after this step UUID" }
      },
      "required": ["action", "project_id", "task_id", "steps"],
      "additionalProperties": false,
      "description": "Add steps to an existing plan"
    },
    {
      "properties": {
        "action": { "const": "list_automations" },
        "enabled": { "type": "boolean", "description": "Filter by enabled state" }
      },
      "required": ["action"],
      "additionalProperties": false,
      "description": "List scheduled automations"
    },
    {
      "properties": {
        "action": { "const": "create_automation" },
        "name": { "type": "string", "description": "Automation name" },
        "target_project_id": { "type": "string", "description": "Target project UUID" },
        "task_name": { "type": "string", "description": "Task name template" },
        "task_description": { "type": "string", "description": "Task description template" },
        "task_workflow": { "type": "string", "description": "Workflow ID (UUID) or slug (e.g. 'default-auto-workflow'). Both formats accepted." },
        "schedule_time": { "type": "string", "description": "Time in HH:MM format (24-hour)" },
        "schedule_days_of_week": { "type": "array", "items": { "type": "integer", "minimum": 0, "maximum": 6 }, "description": "Days: 0=Sun..6=Sat" }
      },
      "required": ["action", "name"],
      "additionalProperties": false,
      "description": "Create a scheduled automation"
    },
    {
      "properties": {
        "action": { "const": "toggle_automation" },
        "automation_id": { "type": "string", "description": "Automation UUID" },
        "enabled": { "type": "boolean", "description": "Enable or disable" }
      },
      "required": ["action", "automation_id", "enabled"],
      "additionalProperties": false,
      "description": "Enable or disable an automation"
    },
    {
      "properties": {
        "action": { "const": "list_task_automations" },
        "project_id": { "type": "string", "description": "Project UUID" },
        "task_id": { "type": "string", "description": "Task UUID" }
      },
      "required": ["action", "project_id", "task_id"],
      "additionalProperties": false,
      "description": "List task automations (flows) for a task"
    },
    {
      "properties": {
        "action": { "const": "solve_coding_problem" },
        "project_id": { "type": "string", "description": "Project UUID" },
        "description": { "type": "string", "description": "Problem description to solve" },
        "workflow_id": { "type": "string", "description": "Workflow ID (UUID) or slug (default: 'default-auto-workflow'). Both formats accepted." }
      },
      "required": ["action", "project_id", "description"],
      "additionalProperties": false,
      "description": "Delegate a coding problem to Zenflow AI agents. Creates and starts a task automatically."
    },
    {
      "properties": {
        "action": { "const": "check_solution_status" },
        "project_id": { "type": "string", "description": "Project UUID" },
        "task_id": { "type": "string", "description": "Task UUID returned by solve_coding_problem" }
      },
      "required": ["action", "project_id", "task_id"],
      "additionalProperties": false,
      "description": "Check the progress of a coding problem solution"
    }
  ]
}"#;

export!(ZencoderTool);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_uuid_valid() {
        assert!(validate_uuid("550e8400-e29b-41d4-a716-446655440000", "test").is_ok());
        assert!(validate_uuid("da1d251c-0cea-4fe6-a744-ec2986035c35", "test").is_ok());
    }

    #[test]
    fn test_validate_uuid_wrong_length() {
        assert!(validate_uuid("too-short", "test").is_err());
        assert!(validate_uuid("", "test").is_err());
    }

    #[test]
    fn test_validate_uuid_wrong_segments() {
        assert!(validate_uuid("aaaaaaaaa-aaaaaaaaa-aaaaaaaaa-aaaaaa", "test").is_err());
    }

    #[test]
    fn test_validate_uuid_non_hex() {
        assert!(validate_uuid("550e8400-e29b-41d4-a716-44665544gggg", "test").is_err());
    }

    #[test]
    fn test_validate_uuid_wrong_segment_lengths() {
        assert!(validate_uuid("550e840-0e29b-41d4-a716-446655440000", "test").is_err());
    }

    #[test]
    fn test_validate_input_length_ok() {
        let at_limit = "a".repeat(MAX_TEXT_LENGTH);
        assert!(validate_input_length(&at_limit, "test").is_ok());
    }

    #[test]
    fn test_validate_input_length_over() {
        let over = "a".repeat(MAX_TEXT_LENGTH + 1);
        assert!(validate_input_length(&over, "test").is_err());
    }

    #[test]
    fn test_validate_task_status_valid() {
        for s in &["todo", "inprogress", "inreview", "done", "cancelled"] {
            assert!(validate_task_status(s).is_ok());
        }
    }

    #[test]
    fn test_validate_task_status_invalid() {
        assert!(validate_task_status("invalid").is_err());
        assert!(validate_task_status("DONE").is_err());
    }

    #[test]
    fn test_validate_step_status_valid() {
        for s in &["Pending", "InProgress", "Completed", "Skipped"] {
            assert!(validate_step_status(s).is_ok());
        }
    }

    #[test]
    fn test_validate_step_status_invalid() {
        assert!(validate_step_status("pending").is_err());
        assert!(validate_step_status("Done").is_err());
    }

    #[test]
    fn test_validate_schedule_time_valid() {
        assert!(validate_schedule_time("00:00").is_ok());
        assert!(validate_schedule_time("23:59").is_ok());
        assert!(validate_schedule_time("09:30").is_ok());
    }

    #[test]
    fn test_validate_schedule_time_invalid() {
        assert!(validate_schedule_time("25:00").is_err());
        assert!(validate_schedule_time("12:60").is_err());
        assert!(validate_schedule_time("1:30").is_err());
        assert!(validate_schedule_time("ab:cd").is_err());
    }

    #[test]
    fn test_validate_days_of_week_valid() {
        assert!(validate_days_of_week(&[0, 1, 2, 3, 4, 5, 6]).is_ok());
        assert!(validate_days_of_week(&[]).is_err());
    }

    #[test]
    fn test_validate_days_of_week_invalid() {
        assert!(validate_days_of_week(&[7]).is_err());
        assert!(validate_days_of_week(&[0, 255]).is_err());
    }

    #[test]
    fn test_url_encode_path_safe_chars() {
        assert_eq!(url_encode_path("foo-bar_123.baz"), "foo-bar_123.baz");
    }

    #[test]
    fn test_url_encode_path_special_chars() {
        assert_eq!(url_encode_path("foo bar"), "foo%20bar");
        assert_eq!(url_encode_path("foo/bar"), "foo%2Fbar");
        assert_eq!(url_encode_path("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn test_url_encode_path_unicode() {
        let encoded = url_encode_path("caf\u{00e9}");
        assert_eq!(encoded, "caf%C3%A9");
    }

    #[test]
    fn test_action_deserialize_list_projects() {
        let json = r#"{"action":"list_projects"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::ListProjects));
    }

    #[test]
    fn test_action_deserialize_create_task() {
        let json = r#"{"action":"create_task","project_id":"550e8400-e29b-41d4-a716-446655440000","title":"Fix bug"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::CreateTask { .. }));
    }

    #[test]
    fn test_action_deserialize_solve_coding_problem() {
        let json = r#"{"action":"solve_coding_problem","project_id":"550e8400-e29b-41d4-a716-446655440000","description":"Fix the login bug"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::SolveCodingProblem { .. }));
    }

    #[test]
    fn test_handle_get_project_invalid_uuid() {
        let err = handle_get_project("not-a-uuid".into()).unwrap_err();
        assert!(err.contains("project_id"));
    }

    #[test]
    fn test_handle_create_task_empty_title() {
        let err = handle_create_task(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "   ".into(),
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("title must not be empty"));
    }

    #[test]
    fn test_handle_create_task_invalid_uuid() {
        let err = handle_create_task("bad".into(), "Title".into(), None, None, None).unwrap_err();
        assert!(err.contains("project_id"));
    }

    #[test]
    fn test_handle_list_tasks_invalid_status() {
        let err = handle_list_tasks(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            Some("bogus".into()),
            None,
        )
        .unwrap_err();
        assert!(err.contains("Invalid status"));
    }

    #[test]
    fn test_handle_list_tasks_invalid_uuid() {
        let err = handle_list_tasks("bad".into(), None, None).unwrap_err();
        assert!(err.contains("project_id"));
    }

    #[test]
    fn test_handle_get_task_invalid_uuids() {
        let err = handle_get_task("bad".into(), "bad".into()).unwrap_err();
        assert!(err.contains("project_id"));

        let err = handle_get_task("550e8400-e29b-41d4-a716-446655440000".into(), "bad".into())
            .unwrap_err();
        assert!(err.contains("task_id"));
    }

    #[test]
    fn test_handle_update_task_all_none() {
        let err = handle_update_task(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("requires at least one of"));
    }

    #[test]
    fn test_handle_update_task_invalid_status() {
        let err = handle_update_task(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            None,
            None,
            Some("INVALID".into()),
        )
        .unwrap_err();
        assert!(err.contains("Invalid status"));
    }

    #[test]
    fn test_handle_update_task_empty_title() {
        let err = handle_update_task(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            Some("   ".into()),
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("title must not be empty"));
    }

    #[test]
    fn test_handle_list_workflows_invalid_uuid() {
        let err = handle_list_workflows(Some("not-valid".into())).unwrap_err();
        assert!(err.contains("project_id"));
    }

    #[test]
    fn test_handle_get_plan_invalid_uuids() {
        let err = handle_get_plan("bad".into(), "bad".into()).unwrap_err();
        assert!(err.contains("project_id"));

        let err = handle_get_plan("550e8400-e29b-41d4-a716-446655440000".into(), "bad".into())
            .unwrap_err();
        assert!(err.contains("task_id"));
    }

    #[test]
    fn test_handle_create_plan_empty_step_name() {
        let err = handle_create_plan(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            vec![PlanStep {
                name: "   ".into(),
                description: "d".into(),
            }],
        )
        .unwrap_err();
        assert!(err.contains("step name must not be empty"));
    }

    #[test]
    fn test_handle_add_plan_steps_empty_step_name() {
        let err = handle_add_plan_steps(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            vec![PlanStep {
                name: "".into(),
                description: "d".into(),
            }],
            None,
        )
        .unwrap_err();
        assert!(err.contains("step name must not be empty"));
    }

    #[test]
    fn test_handle_create_automation_empty_task_name() {
        let err = handle_create_automation(
            "My Auto".into(),
            None,
            Some("   ".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("task_name must not be empty"));
    }

    #[test]
    fn test_handle_create_automation_empty_days() {
        let err =
            handle_create_automation("My Auto".into(), None, None, None, None, None, Some(vec![]))
                .unwrap_err();
        assert!(err.contains("schedule_days_of_week must contain at least one day"));
    }

    #[test]
    fn test_handle_create_plan_empty_steps() {
        let err = handle_create_plan(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            vec![],
        )
        .unwrap_err();
        assert!(err.contains("at least one step is required"));
    }

    #[test]
    fn test_handle_create_plan_invalid_uuid() {
        let err = handle_create_plan(
            "bad".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            vec![PlanStep {
                name: "s".into(),
                description: "d".into(),
            }],
        )
        .unwrap_err();
        assert!(err.contains("project_id"));
    }

    #[test]
    fn test_handle_update_plan_step_all_none() {
        let err = handle_update_plan_step(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("requires at least one of"));
    }

    #[test]
    fn test_handle_update_plan_step_invalid_status() {
        let err = handle_update_plan_step(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            Some("bad_status".into()),
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("Invalid step status"));
    }

    #[test]
    fn test_handle_update_plan_step_empty_name() {
        let err = handle_update_plan_step(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            None,
            Some("   ".into()),
            None,
        )
        .unwrap_err();
        assert!(err.contains("name must not be empty"));
    }

    #[test]
    fn test_handle_update_plan_step_invalid_step_id() {
        let err = handle_update_plan_step(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "bad".into(),
            Some("Completed".into()),
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("step_id"));
    }

    #[test]
    fn test_handle_add_plan_steps_empty() {
        let err = handle_add_plan_steps(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            vec![],
            None,
        )
        .unwrap_err();
        assert!(err.contains("at least one step is required"));
    }

    #[test]
    fn test_handle_add_plan_steps_invalid_after_step_id() {
        let err = handle_add_plan_steps(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "550e8400-e29b-41d4-a716-446655440000".into(),
            vec![PlanStep {
                name: "s".into(),
                description: "d".into(),
            }],
            Some("bad".into()),
        )
        .unwrap_err();
        assert!(err.contains("after_step_id"));
    }

    #[test]
    fn test_handle_create_automation_empty_name() {
        let err =
            handle_create_automation("   ".into(), None, None, None, None, None, None).unwrap_err();
        assert!(err.contains("name must not be empty"));
    }

    #[test]
    fn test_handle_create_automation_invalid_project_uuid() {
        let err = handle_create_automation(
            "My Auto".into(),
            Some("bad".into()),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("target_project_id"));
    }

    #[test]
    fn test_handle_create_automation_invalid_schedule() {
        let err = handle_create_automation(
            "My Auto".into(),
            None,
            None,
            None,
            None,
            Some("99:99".into()),
            None,
        )
        .unwrap_err();
        assert!(err.contains("Invalid"));

        let err = handle_create_automation(
            "My Auto".into(),
            None,
            None,
            None,
            None,
            None,
            Some(vec![7]),
        )
        .unwrap_err();
        assert!(err.contains("Invalid day_of_week"));
    }

    #[test]
    fn test_handle_toggle_automation_invalid_uuid() {
        let err = handle_toggle_automation("bad".into(), true).unwrap_err();
        assert!(err.contains("automation_id"));
    }

    #[test]
    fn test_handle_list_task_automations_invalid_uuids() {
        let err = handle_list_task_automations("bad".into(), "bad".into()).unwrap_err();
        assert!(err.contains("project_id"));

        let err = handle_list_task_automations(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "bad".into(),
        )
        .unwrap_err();
        assert!(err.contains("task_id"));
    }

    #[test]
    fn test_action_deserialize_get_plan() {
        let json = r#"{"action":"get_plan","project_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"550e8400-e29b-41d4-a716-446655440000"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::GetPlan { .. }));
    }

    #[test]
    fn test_action_deserialize_create_plan() {
        let json = r#"{"action":"create_plan","project_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"550e8400-e29b-41d4-a716-446655440000","steps":[{"name":"Step 1","description":"Do something"}]}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::CreatePlan { .. }));
    }

    #[test]
    fn test_action_deserialize_list_automations() {
        let json = r#"{"action":"list_automations"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::ListAutomations { .. }));
    }

    #[test]
    fn test_action_deserialize_toggle_automation() {
        let json = r#"{"action":"toggle_automation","automation_id":"550e8400-e29b-41d4-a716-446655440000","enabled":false}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::ToggleAutomation { .. }));
    }

    #[test]
    fn test_action_deserialize_create_automation() {
        let json = r#"{"action":"create_automation","name":"Daily build","schedule_time":"09:00","schedule_days_of_week":[1,2,3,4,5]}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::CreateAutomation { .. }));
    }

    #[test]
    fn test_action_deserialize_list_task_automations() {
        let json = r#"{"action":"list_task_automations","project_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"550e8400-e29b-41d4-a716-446655440000"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::ListTaskAutomations { .. }));
    }

    #[test]
    fn test_derive_title_short() {
        let input = "Fix the login bug";
        assert_eq!(derive_title(input), "Fix the login bug");
    }

    #[test]
    fn test_derive_title_exactly_100_chars() {
        let input = "a".repeat(100);
        assert_eq!(derive_title(&input), input);
    }

    #[test]
    fn test_derive_title_long_with_word_boundary() {
        let input = format!("{} {}", "word".repeat(20), "tail".repeat(10));
        let result = derive_title(&input);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= 104);
    }

    #[test]
    fn test_derive_title_long_no_spaces_near_boundary() {
        let input = "a".repeat(200);
        let result = derive_title(&input);
        assert!(result.ends_with("..."));
        assert_eq!(result.chars().count(), 103);
    }

    #[test]
    fn test_derive_title_multibyte_utf8() {
        let input = "\u{00e9}".repeat(150);
        let result = derive_title(&input);
        assert!(result.ends_with("..."));
        assert_eq!(result.chars().count(), 103);
    }

    #[test]
    fn test_derive_title_early_word_boundary() {
        let input = format!("a b {}", "c".repeat(200));
        let result = derive_title(&input);
        assert!(result.ends_with("..."));
        assert_eq!(result.chars().count(), 103);
    }

    #[test]
    fn test_handle_solve_coding_problem_empty_description() {
        let err = handle_solve_coding_problem(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "   ".into(),
            None,
        )
        .unwrap_err();
        assert!(err.contains("description must not be empty"));
    }

    #[test]
    fn test_handle_solve_coding_problem_invalid_uuid() {
        let err =
            handle_solve_coding_problem("bad".into(), "Fix the bug".into(), None).unwrap_err();
        assert!(err.contains("project_id"));
    }

    #[test]
    fn test_handle_check_solution_status_invalid_uuids() {
        let err = handle_check_solution_status("bad".into(), "bad".into()).unwrap_err();
        assert!(err.contains("project_id"));

        let err = handle_check_solution_status(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "bad".into(),
        )
        .unwrap_err();
        assert!(err.contains("task_id"));
    }

    #[test]
    fn test_action_deserialize_get_project() {
        let json =
            r#"{"action":"get_project","project_id":"550e8400-e29b-41d4-a716-446655440000"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::GetProject { .. }));
    }

    #[test]
    fn test_action_deserialize_list_tasks() {
        let json = r#"{"action":"list_tasks","project_id":"550e8400-e29b-41d4-a716-446655440000","status":"todo","limit":10}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::ListTasks { .. }));
    }

    #[test]
    fn test_action_deserialize_get_task() {
        let json = r#"{"action":"get_task","project_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"550e8400-e29b-41d4-a716-446655440000"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::GetTask { .. }));
    }

    #[test]
    fn test_action_deserialize_update_task() {
        let json = r#"{"action":"update_task","project_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"550e8400-e29b-41d4-a716-446655440000","status":"done"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::UpdateTask { .. }));
    }

    #[test]
    fn test_action_deserialize_list_workflows() {
        let json = r#"{"action":"list_workflows"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::ListWorkflows { .. }));
    }

    #[test]
    fn test_action_deserialize_update_plan_step() {
        let json = r#"{"action":"update_plan_step","project_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"550e8400-e29b-41d4-a716-446655440000","step_id":"550e8400-e29b-41d4-a716-446655440000","status":"Completed"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::UpdatePlanStep { .. }));
    }

    #[test]
    fn test_action_deserialize_add_plan_steps() {
        let json = r#"{"action":"add_plan_steps","project_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"550e8400-e29b-41d4-a716-446655440000","steps":[{"name":"New step","description":"Details"}]}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::AddPlanSteps { .. }));
    }

    #[test]
    fn test_action_deserialize_check_solution_status() {
        let json = r#"{"action":"check_solution_status","project_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"550e8400-e29b-41d4-a716-446655440000"}"#;
        let action: ZencoderAction = serde_json::from_str(json).unwrap();
        assert!(matches!(action, ZencoderAction::CheckSolutionStatus { .. }));
    }

    #[test]
    fn test_schema_is_valid_json() {
        let parsed: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
        let one_of = parsed.get("oneOf").unwrap().as_array().unwrap();
        assert_eq!(one_of.len(), 17);
    }

    #[test]
    fn test_auth_not_configured_error_has_no_blank_continuation_lines() {
        let msg = AUTH_NOT_CONFIGURED_ERROR;

        let lines: Vec<&str> = msg.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim_end().ends_with('\\') {
                let next = lines.get(i + 1).copied().unwrap_or("");
                assert!(
                    !next.trim().is_empty(),
                    "blank line follows shell line-continuation at line {}: \
                     copy-pasting this curl command would break in bash/zsh.\n\
                     Full message:\n{}",
                    i,
                    msg
                );
            }
        }
    }

    #[test]
    fn test_auth_not_configured_error_mentions_required_commands() {
        let msg = AUTH_NOT_CONFIGURED_ERROR;
        for needle in [
            "https://auth.zencoder.ai",
            "scripts/zencoder-auth.sh",
            "ironclaw tool auth zencoder-tool",
        ] {
            assert!(
                msg.contains(needle),
                "expected error message to mention `{}`; got:\n{}",
                needle,
                msg
            );
        }
    }

    #[test]
    fn test_url_encode_path_preserves_tilde() {
        assert_eq!(url_encode_path("a~b"), "a~b");
    }

    #[test]
    fn test_handle_solve_coding_problem_workflow_id_length_validated() {
        let huge = "x".repeat(MAX_TEXT_LENGTH + 1);
        let err = handle_solve_coding_problem(
            "550e8400-e29b-41d4-a716-446655440000".into(),
            "do something".into(),
            Some(huge),
        )
        .unwrap_err();
        assert!(
            err.contains("workflow_id"),
            "expected workflow_id length validation; got: {}",
            err
        );
    }

    #[test]
    fn test_auth_not_configured_error_paste_prompt_mentioned() {
        let msg = AUTH_NOT_CONFIGURED_ERROR;
        assert!(
            msg.contains("Paste your token:"),
            "error message must tell the user what IronClaw will prompt for; got:\n{}",
            msg
        );
    }
}

wit_bindgen::generate!({
    world: "sandboxed-tool",
    path: "../wit/tool.wit",
});

use serde::Deserialize;

const MAX_TEXT_LENGTH: usize = 65536;
const API_BASE_URL: &str = "https://api.zencoder.ai";

fn validate_input_length(s: &str, field_name: &str) -> Result<(), String> {
    if s.len() > MAX_TEXT_LENGTH && s.chars().count() > MAX_TEXT_LENGTH {
        return Err(format!(
            "Input '{}' exceeds maximum length of {} characters",
            field_name, MAX_TEXT_LENGTH
        ));
    }
    Ok(())
}

fn validate_uuid(s: &str, field_name: &str) -> Result<(), String> {
    if s.len() != 36 {
        return Err(format!(
            "Invalid {}: expected 36 characters, got {}",
            field_name,
            s.len()
        ));
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return Err(format!(
            "Invalid {}: expected 5 dash-separated segments, got {}",
            field_name,
            parts.len()
        ));
    }
    let expected = [8, 4, 4, 4, 12];
    for (part, &len) in parts.iter().zip(&expected) {
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
    if !s.is_ascii() || s.len() != 5 {
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
            return Err(format!(
                "Invalid day_of_week {}: must be 0 (Sun) - 6 (Sat)",
                d
            ));
        }
    }
    Ok(())
}

fn url_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
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

    #[serde(rename = "list_workflows")]
    ListWorkflows { project_id: Option<String> },

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
    ListTaskAutomations {
        project_id: String,
        task_id: String,
    },

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

#[derive(Deserialize, serde::Serialize)]
struct PlanStep {
    name: String,
    description: String,
}

#[derive(Deserialize, serde::Serialize)]
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
         progress. Authentication is handled via the 'zencoder_api_key' secret injected by the host."
            .to_string()
    }
}

fn ensure_auth_configured() -> Result<(), String> {
    if near::agent::host::secret_exists("zencoder_api_key") {
        return Ok(());
    }
    Err(
        "Zencoder API key not found. Set it with: ironclaw secret set zencoder_api_key <key>\n\
         Get your API key from https://app.zencoder.ai/settings"
            .into(),
    )
}

fn api_request(method: &str, path: &str, body: Option<String>) -> Result<String, String> {
    let url = format!("{}{}", API_BASE_URL, path);

    let headers = serde_json::json!({
        "Content-Type": "application/json",
        "User-Agent": "IronClaw-Zencoder-Tool"
    });

    let body_bytes = body.map(|b| b.into_bytes());

    let max_attempts = 3;
    let mut attempt = 0;

    loop {
        attempt += 1;

        let response = near::agent::host::http_request(
            method,
            &url,
            &headers.to_string(),
            body_bytes.as_deref(),
            Some(30_000u32),
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
                            if count < 10 {
                                near::agent::host::log(
                                    near::agent::host::LogLevel::Warn,
                                    &format!(
                                        "Zencoder API rate limit low: {} remaining",
                                        count
                                    ),
                                );
                            }
                        }
                    }
                }

                if resp.status >= 200 && resp.status < 300 {
                    return String::from_utf8(resp.body)
                        .map_err(|e| format!("Invalid UTF-8 in response: {}", e));
                } else if attempt < max_attempts && (resp.status == 429 || resp.status >= 500) {
                    let mut msg = format!(
                        "Zencoder API error {} (attempt {}/{}). Retrying...",
                        resp.status, attempt, max_attempts
                    );
                    if resp.status == 429 {
                        if let Ok(headers_json) =
                            serde_json::from_str::<serde_json::Value>(&resp.headers_json)
                        {
                            if let Some(retry_after) = headers_json
                                .get("retry-after")
                                .or_else(|| headers_json.get("Retry-After"))
                                .and_then(|v| v.as_str())
                            {
                                msg = format!("{} (Retry-After: {}s)", msg, retry_after);
                            }
                        }
                    }
                    near::agent::host::log(near::agent::host::LogLevel::Warn, &msg);
                    continue;
                } else {
                    let body_str = String::from_utf8_lossy(&resp.body);
                    near::agent::host::log(
                        near::agent::host::LogLevel::Warn,
                        &format!("Zencoder API {} body: {}", resp.status, body_str),
                    );
                    return Err(format!(
                        "Zencoder API returned status {}. Check your API key and parameters.",
                        resp.status
                    ));
                }
            }
            Err(e) => {
                if attempt < max_attempts {
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
                    "HTTP request failed after {} attempts: {}",
                    max_attempts, e
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
        ZencoderAction::ListProjects => stub("list_projects"),
        ZencoderAction::GetProject { .. } => stub("get_project"),
        ZencoderAction::CreateTask { .. } => stub("create_task"),
        ZencoderAction::ListTasks { .. } => stub("list_tasks"),
        ZencoderAction::GetTask { .. } => stub("get_task"),
        ZencoderAction::UpdateTask { .. } => stub("update_task"),
        ZencoderAction::ListWorkflows { .. } => stub("list_workflows"),
        ZencoderAction::GetPlan { .. } => stub("get_plan"),
        ZencoderAction::CreatePlan { .. } => stub("create_plan"),
        ZencoderAction::UpdatePlanStep { .. } => stub("update_plan_step"),
        ZencoderAction::AddPlanSteps { .. } => stub("add_plan_steps"),
        ZencoderAction::ListAutomations { .. } => stub("list_automations"),
        ZencoderAction::CreateAutomation { .. } => stub("create_automation"),
        ZencoderAction::ToggleAutomation { .. } => stub("toggle_automation"),
        ZencoderAction::ListTaskAutomations { .. } => stub("list_task_automations"),
        ZencoderAction::SolveCodingProblem { .. } => stub("solve_coding_problem"),
        ZencoderAction::CheckSolutionStatus { .. } => stub("check_solution_status"),
    }
}

fn stub(action: &str) -> Result<String, String> {
    Err(format!("Action '{}' is not yet implemented", action))
}

const SCHEMA: &str = r#"{"type":"object","required":["action"],"oneOf":[]}"#;

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
        assert!(validate_uuid("550e8400e29b-41d4-a716-446655440000", "test").is_err());
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
        assert!(validate_days_of_week(&[]).is_ok());
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
}

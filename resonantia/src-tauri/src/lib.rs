use hex::encode as hex_encode;
use regex::Regex;
use reqwest::Client;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use tauri::Manager;

const DEFAULT_GATEWAY_BASE_URL: &str = "http://127.0.0.1:8080";
const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_OLLAMA_MODEL: &str = "gemma3";
const APP_CONFIG_FILE_NAME: &str = "resonantia-config.json";

struct AppState {
    http: Client,
    gateway_base_url: RwLock<String>,
    ollama_base_url: RwLock<String>,
    ollama_model: RwLock<String>,
    layout_overrides: RwLock<LayoutOverrides>,
    config_path: RwLock<Option<PathBuf>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            http: Client::new(),
            gateway_base_url: RwLock::new(DEFAULT_GATEWAY_BASE_URL.to_string()),
            ollama_base_url: RwLock::new(DEFAULT_OLLAMA_BASE_URL.to_string()),
            ollama_model: RwLock::new(DEFAULT_OLLAMA_MODEL.to_string()),
            layout_overrides: RwLock::new(LayoutOverrides::default()),
            config_path: RwLock::new(None),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct LayoutPoint {
    x: f32,
    y: f32,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct LayoutOverrides {
    #[serde(default)]
    session_overrides: HashMap<String, LayoutPoint>,
    #[serde(default)]
    node_overrides: HashMap<String, LayoutPoint>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    gateway_base_url: String,
    ollama_base_url: String,
    ollama_model: String,
    #[serde(default)]
    layout_overrides: LayoutOverrides,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: String,
    transport: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoreContextRequest {
    node: String,
    session_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoreContextResponse {
    node_id: String,
    psi: f32,
    valid: bool,
    validation_error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalibrateSessionRequest {
    session_id: String,
    stability: f32,
    friction: f32,
    logic: f32,
    autonomy: f32,
    trigger: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AvecState {
    stability: f32,
    friction: f32,
    logic: f32,
    autonomy: f32,
    psi: f32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalibrateSessionResponse {
    previous_avec: AvecState,
    delta: f32,
    drift_classification: String,
    trigger: String,
    trigger_history: Vec<String>,
    is_first_calibration: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListNodesResponse {
    nodes: Vec<NodeDto>,
    retrieved: i32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphResponse {
    sessions: Vec<GraphSessionDto>,
    nodes: Vec<GraphNodeDto>,
    edges: Vec<GraphEdgeDto>,
    retrieved: i32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphSessionDto {
    id: String,
    label: String,
    node_count: i32,
    avg_psi: f32,
    last_modified: String,
    size: i32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphNodeDto {
    id: String,
    session_id: String,
    label: String,
    tier: String,
    timestamp: String,
    psi: f32,
    parent_node_id: Option<String>,
    size: i32,
    #[serde(default)]
    synthetic_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphEdgeDto {
    id: String,
    source: String,
    target: String,
    kind: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeDto {
    raw: String,
    session_id: String,
    tier: String,
    timestamp: String,
    compression_depth: i32,
    parent_node_id: Option<String>,
    user_avec: AvecState,
    model_avec: AvecState,
    compression_avec: Option<AvecState>,
    rho: f32,
    kappa: f32,
    psi: f32,
    #[serde(default)]
    synthetic_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnwindResult {
    status_icon: String,
    status_label: String,
    status_class: String,
    summary: String,
    interpretation: String,
    next_action: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiSummary {
    topic: String,
    what_happened: String,
    where_we_left_off: String,
    vibe: String,
    pick_back_up_with: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OllamaChatResponse {
    message: Option<OllamaMessage>,
}

fn with_query(base_url: &str, path: &str, limit: i32, session_id: Option<String>) -> Result<String, String> {
    let mut url = Url::parse(&format!("{base_url}{path}"))
        .map_err(|err| map_err("failed to build request url", err))?;

    {
        let mut query = url.query_pairs_mut();
        query.append_pair("limit", &limit.to_string());
        if let Some(session_id) = session_id {
            let trimmed = session_id.trim();
            if !trimmed.is_empty() {
                query.append_pair("sessionId", trimmed);
            }
        }
    }

    Ok(url.to_string())
}

fn map_err(prefix: &str, err: impl std::fmt::Display) -> String {
    format!("{prefix}: {err}")
}

fn node_fingerprint(session_id: &str, timestamp: &str, tier: &str, parent_node_id: Option<&str>, psi: f32) -> String {
    let canonical = format!(
        "{}|{}|{}|{}|{:.6}",
        session_id.trim(),
        timestamp.trim(),
        tier.trim(),
        parent_node_id.unwrap_or("").trim(),
        psi
    );
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex_encode(hasher.finalize())
}

fn stamp_graph_node_ids(response: &mut GraphResponse) {
    for node in &mut response.nodes {
        node.synthetic_id = node_fingerprint(
            &node.session_id,
            &node.timestamp,
            &node.tier,
            node.parent_node_id.as_deref(),
            node.psi,
        );
    }
}

fn stamp_list_node_ids(response: &mut ListNodesResponse) {
    for node in &mut response.nodes {
        node.synthetic_id = node_fingerprint(
            &node.session_id,
            &node.timestamp,
            &node.tier,
            node.parent_node_id.as_deref(),
            node.psi,
        );
    }
}

fn read_current_config(state: &AppState) -> Result<AppConfig, String> {
    let gateway_base_url = state
        .gateway_base_url
        .read()
        .map_err(|err| map_err("failed to read gateway url", err))?
        .clone();

    let ollama_base_url = state
        .ollama_base_url
        .read()
        .map_err(|err| map_err("failed to read ollama url", err))?
        .clone();

    let ollama_model = state
        .ollama_model
        .read()
        .map_err(|err| map_err("failed to read ollama model", err))?
        .clone();

    let layout_overrides = state
        .layout_overrides
        .read()
        .map_err(|err| map_err("failed to read layout overrides", err))?
        .clone();

    Ok(AppConfig {
        gateway_base_url,
        ollama_base_url,
        ollama_model,
        layout_overrides,
    })
}

fn persist_current_config(state: &AppState) -> Result<(), String> {
    let config = read_current_config(state)?;
    let config_path = state
        .config_path
        .read()
        .map_err(|err| map_err("failed to read config path", err))?
        .clone();

    let Some(path) = config_path else {
        return Ok(());
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| map_err("failed to create config directory", err))?;
    }

    let payload = serde_json::to_string_pretty(&config)
        .map_err(|err| map_err("failed to serialize app config", err))?;

    fs::write(path, payload).map_err(|err| map_err("failed to write app config", err))
}

fn load_persisted_config(state: &AppState) -> Result<(), String> {
    let config_path = state
        .config_path
        .read()
        .map_err(|err| map_err("failed to read config path", err))?
        .clone();

    let Some(path) = config_path else {
        return Ok(());
    };

    if !path.exists() {
        return Ok(());
    }

    let payload = fs::read_to_string(path).map_err(|err| map_err("failed to read app config", err))?;
    let config: AppConfig = serde_json::from_str(&payload)
        .map_err(|err| map_err("failed to parse app config", err))?;

    {
        let mut guard = state
            .gateway_base_url
            .write()
            .map_err(|err| map_err("failed to restore gateway url", err))?;
        *guard = config.gateway_base_url;
    }

    {
        let mut guard = state
            .ollama_base_url
            .write()
            .map_err(|err| map_err("failed to restore ollama url", err))?;
        *guard = config.ollama_base_url;
    }

    {
        let mut guard = state
            .ollama_model
            .write()
            .map_err(|err| map_err("failed to restore ollama model", err))?;
        *guard = config.ollama_model;
    }

    {
        let mut guard = state
            .layout_overrides
            .write()
            .map_err(|err| map_err("failed to restore layout overrides", err))?;
        *guard = config.layout_overrides;
    }

    Ok(())
}

fn parse_ai_response(text: &str) -> Option<AiSummary> {
    let thinking_re = Regex::new(r"(?is)Thinking\.\.\..*?\.\.\.done thinking\.").ok()?;
    let cleaned = thinking_re.replace(text, "").trim().to_string();

    let headings = r"Topic|What happened|Where we left off|Vibe|Pick back up with";

    let extract = |label: &str| {
        let pattern = format!(
            r"(?is)(?:^|\n){}:\s*(.+?)(?=\n(?:{}):|$)",
            regex::escape(label),
            headings
        );
        let re = match Regex::new(&pattern) {
            Ok(value) => value,
            Err(_) => return String::new(),
        };
        re.captures(&cleaned)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default()
    };

    let topic = extract("Topic");
    let what_happened = extract("What happened");
    let where_we_left_off = extract("Where we left off");
    let vibe = extract("Vibe");
    let pick_back_up_with = extract("Pick back up with");

    if topic.trim().is_empty() && what_happened.trim().is_empty() {
        return None;
    }

    Some(AiSummary {
        topic,
        what_happened,
        where_we_left_off,
        vibe,
        pick_back_up_with,
    })
}

fn to_sentence(raw: &str) -> String {
    let date_re = match Regex::new(r"[-_]\d{4}[-_]\d{2}[-_]\d{2}$") {
        Ok(re) => re,
        Err(_) => return "a session.".to_string(),
    };

    let cleaned = date_re.replace(raw, "").to_string();
    let cleaned = cleaned.replace('-', " ").replace('_', " ").trim().to_string();

    if cleaned.is_empty() {
        return "a session.".to_string();
    }

    format!("You worked on {}.", cleaned.to_lowercase())
}

fn extract_summary(node: &NodeDto) -> String {
    if !node.raw.trim().is_empty() {
        // Try quoted form first: context_summary(...):"text"
        let quoted_re = match Regex::new(r#"context_summary\([^)]*\):\s*"([^"]+)""#) {
            Ok(value) => value,
            Err(_) => return to_sentence(&node.session_id),
        };
        if let Some(caps) = quoted_re.captures(&node.raw) {
            if let Some(value) = caps.get(1) {
                return to_sentence(value.as_str());
            }
        }

        // Fall back to unquoted form: context_summary(...): text until newline or comma
        let unquoted_re = match Regex::new(r#"context_summary\([^)]*\):\s*([^,\n]+)"#) {
            Ok(value) => value,
            Err(_) => return to_sentence(&node.session_id),
        };
        if let Some(caps) = unquoted_re.captures(&node.raw) {
            if let Some(value) = caps.get(1) {
                let text = value.as_str().trim();
                if !text.is_empty() {
                    return to_sentence(text);
                }
            }
        }
    }

    to_sentence(&node.session_id)
}

fn interpret(friction: f32, logic: f32) -> String {
    let high_f = friction > 0.5;
    let high_l = logic > 0.85;
    let med_l = logic >= 0.6;

    match (high_f, high_l, med_l) {
        (false, true, _) => "You understood things clearly and it felt smooth.".to_string(),
        (false, false, true) => "Things went smoothly, but some parts are still forming.".to_string(),
        (true, true, _) => "You figured it out, but it took effort.".to_string(),
        _ => "This was confusing and frustrating.".to_string(),
    }
}

fn next_action(friction: f32, logic: f32, _autonomy: f32) -> String {
    if logic > 0.85 && friction < 0.2 {
        return "Keep going - you're ready to build or expand this.".to_string();
    }
    if logic >= 0.6 && friction < 0.5 {
        return "Keep practicing - try a small improvement or variation.".to_string();
    }
    if friction > 0.5 {
        return "Slow down - break this into smaller steps or ask for help.".to_string();
    }
    "Focus on understanding - revisit the basics or simplify.".to_string()
}

fn unwind(node: NodeDto) -> UnwindResult {
    let avec = &node.user_avec;
    let score = (avec.logic + avec.stability + avec.autonomy) / 3.0 - avec.friction;

    let (icon, label, class_name) = if score >= 0.75 {
        ("OK", "Great progress", "status-great")
    } else if score >= 0.5 {
        ("GOOD", "Good progress", "status-good")
    } else if score >= 0.25 {
        ("FRICTION", "Some friction", "status-friction")
    } else {
        ("STUCK", "You got stuck", "status-stuck")
    };

    UnwindResult {
        status_icon: icon.to_string(),
        status_label: label.to_string(),
        status_class: class_name.to_string(),
        summary: extract_summary(&node),
        interpretation: interpret(avec.friction, avec.logic),
        next_action: next_action(avec.friction, avec.logic, avec.autonomy),
    }
}

#[tauri::command]
fn get_config(state: tauri::State<'_, AppState>) -> Result<AppConfig, String> {
    read_current_config(&state)
}

#[tauri::command]
fn get_layout_overrides(state: tauri::State<'_, AppState>) -> Result<LayoutOverrides, String> {
    state
        .layout_overrides
        .read()
        .map_err(|err| map_err("failed to read layout overrides", err))
        .map(|guard| guard.clone())
}

#[tauri::command]
fn save_layout_overrides(
    state: tauri::State<'_, AppState>,
    layout_overrides: LayoutOverrides,
) -> Result<(), String> {
    {
        let mut guard = state
            .layout_overrides
            .write()
            .map_err(|err| map_err("failed to update layout overrides", err))?;
        *guard = layout_overrides;
    }

    persist_current_config(&state)?;
    Ok(())
}

#[tauri::command]
fn reset_layout_overrides(state: tauri::State<'_, AppState>) -> Result<(), String> {
    {
        let mut guard = state
            .layout_overrides
            .write()
            .map_err(|err| map_err("failed to reset layout overrides", err))?;
        *guard = LayoutOverrides::default();
    }

    persist_current_config(&state)?;
    Ok(())
}

#[tauri::command]
fn set_gateway_base_url(state: tauri::State<'_, AppState>, base_url: String) -> Result<(), String> {
    {
        let mut guard = state
            .gateway_base_url
            .write()
            .map_err(|err| map_err("failed to update gateway url", err))?;
        *guard = base_url;
    }

    persist_current_config(&state)?;
    Ok(())
}

#[tauri::command]
fn set_ollama_config(
    state: tauri::State<'_, AppState>,
    base_url: Option<String>,
    model: Option<String>,
) -> Result<(), String> {
    if let Some(base_url) = base_url {
        let mut guard = state
            .ollama_base_url
            .write()
            .map_err(|err| map_err("failed to update ollama url", err))?;
        *guard = base_url;
    }

    if let Some(model) = model {
        let mut guard = state
            .ollama_model
            .write()
            .map_err(|err| map_err("failed to update ollama model", err))?;
        *guard = model;
    }

    persist_current_config(&state)?;

    Ok(())
}

#[tauri::command]
async fn get_health(state: tauri::State<'_, AppState>) -> Result<HealthResponse, String> {
    let base_url = state
        .gateway_base_url
        .read()
        .map_err(|err| map_err("failed to read gateway url", err))?
        .clone();

    let url = format!("{base_url}/health");
    state
        .http
        .get(url)
        .send()
        .await
        .map_err(|err| map_err("health request failed", err))?
        .error_for_status()
        .map_err(|err| map_err("health response status failed", err))?
        .json::<HealthResponse>()
        .await
        .map_err(|err| map_err("health response parse failed", err))
}

#[tauri::command]
async fn list_nodes(
    state: tauri::State<'_, AppState>,
    limit: i32,
    session_id: Option<String>,
) -> Result<ListNodesResponse, String> {
    let base_url = state
        .gateway_base_url
        .read()
        .map_err(|err| map_err("failed to read gateway url", err))?
        .clone();

    let url = with_query(&base_url, "/api/v1/nodes", limit, session_id)?;

    let mut response = state
        .http
        .get(url)
        .send()
        .await
        .map_err(|err| map_err("list nodes request failed", err))?
        .error_for_status()
        .map_err(|err| map_err("list nodes response status failed", err))?
        .json::<ListNodesResponse>()
        .await
        .map_err(|err| map_err("list nodes response parse failed", err))?;

    stamp_list_node_ids(&mut response);
    Ok(response)
}

#[tauri::command]
async fn get_graph(
    state: tauri::State<'_, AppState>,
    limit: i32,
    session_id: Option<String>,
) -> Result<GraphResponse, String> {
    let base_url = state
        .gateway_base_url
        .read()
        .map_err(|err| map_err("failed to read gateway url", err))?
        .clone();

    let url = with_query(&base_url, "/api/v1/graph", limit, session_id)?;

    let mut response = state
        .http
        .get(url)
        .send()
        .await
        .map_err(|err| map_err("graph request failed", err))?
        .error_for_status()
        .map_err(|err| map_err("graph response status failed", err))?
        .json::<GraphResponse>()
        .await
        .map_err(|err| map_err("graph response parse failed", err))?;

    stamp_graph_node_ids(&mut response);
    Ok(response)
}

#[tauri::command]
async fn store_context(
    state: tauri::State<'_, AppState>,
    request: StoreContextRequest,
) -> Result<StoreContextResponse, String> {
    let base_url = state
        .gateway_base_url
        .read()
        .map_err(|err| map_err("failed to read gateway url", err))?
        .clone();

    let url = format!("{base_url}/api/v1/store");

    state
        .http
        .post(url)
        .json(&request)
        .send()
        .await
        .map_err(|err| map_err("store request failed", err))?
        .error_for_status()
        .map_err(|err| map_err("store response status failed", err))?
        .json::<StoreContextResponse>()
        .await
        .map_err(|err| map_err("store response parse failed", err))
}

#[tauri::command]
async fn calibrate_session(
    state: tauri::State<'_, AppState>,
    request: CalibrateSessionRequest,
) -> Result<CalibrateSessionResponse, String> {
    let base_url = state
        .gateway_base_url
        .read()
        .map_err(|err| map_err("failed to read gateway url", err))?
        .clone();

    let url = format!("{base_url}/api/v1/calibrate");

    state
        .http
        .post(url)
        .json(&request)
        .send()
        .await
        .map_err(|err| map_err("calibrate request failed", err))?
        .error_for_status()
        .map_err(|err| map_err("calibrate response status failed", err))?
        .json::<CalibrateSessionResponse>()
        .await
        .map_err(|err| map_err("calibrate response parse failed", err))
}

#[tauri::command]
async fn summarize_node(
    state: tauri::State<'_, AppState>,
    raw_node: String,
) -> Result<Option<AiSummary>, String> {
    let ollama_base_url = state
        .ollama_base_url
        .read()
        .map_err(|err| map_err("failed to read ollama url", err))?
        .clone();

    let model = state
        .ollama_model
        .read()
        .map_err(|err| map_err("failed to read ollama model", err))?
        .clone();

    let url = format!("{ollama_base_url}/api/chat");
    let payload = OllamaChatRequest {
        model,
        messages: vec![OllamaMessage {
            role: "user".to_string(),
            content: raw_node,
        }],
        stream: false,
    };

    let response = state
        .http
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|err| map_err("ollama request failed", err))?
        .error_for_status()
        .map_err(|err| map_err("ollama response status failed", err))?
        .json::<OllamaChatResponse>()
        .await
        .map_err(|err| map_err("ollama response parse failed", err))?;

    let text = match response.message {
        Some(message) => message.content,
        None => return Ok(None),
    };

    Ok(parse_ai_response(&text))
}

#[tauri::command]
fn unwind_node(node: NodeDto) -> UnwindResult {
    unwind(node)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .setup(|app| {
            let state = app.state::<AppState>();
            let config_dir = app
                .path()
                .app_config_dir()
                .map_err(|err| map_err("failed to resolve app config dir", err))?;

            fs::create_dir_all(&config_dir)
                .map_err(|err| map_err("failed to create app config dir", err))?;

            let config_path = config_dir.join(APP_CONFIG_FILE_NAME);
            {
                let mut guard = state
                    .config_path
                    .write()
                    .map_err(|err| map_err("failed to set config path", err))?;
                *guard = Some(config_path);
            }

            if let Err(err) = load_persisted_config(&state) {
                eprintln!("config restore warning: {err}");
            }

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_layout_overrides,
            save_layout_overrides,
            reset_layout_overrides,
            set_gateway_base_url,
            set_ollama_config,
            get_health,
            list_nodes,
            get_graph,
            store_context,
            calibrate_session,
            summarize_node,
            unwind_node,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

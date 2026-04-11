use async_trait::async_trait;
use hex::encode as hex_encode;
use regex::Regex;
use reqwest::Client;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use sttp_core_rs::storage::{QueryParams, SurrealDbClient, SurrealDbNodeStore};
use sttp_core_rs::{
    CalibrationService, ContextQueryService, InMemoryNodeStore, NodeQuery, NodeStore,
    NodeStoreInitializer, StoreContextService, SttpNodeParser, TreeSitterValidator,
};
use surrealdb::engine::any::{connect as surreal_connect, Any as SurrealAny};
use surrealdb::Surreal;
use tauri::Manager;

const DEFAULT_GATEWAY_BASE_URL: &str = "http://127.0.0.1:8080";
const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_OLLAMA_MODEL: &str = "gemma3";
const APP_CONFIG_FILE_NAME: &str = "resonantia-config.json";
const LOCAL_STTP_DB_FILE_NAME: &str = "sttp-local.db";
const STORE_DEDUPE_SCAN_LIMIT: usize = 5000;

struct SttpRuntime {
    store: Arc<dyn NodeStore>,
    context_query: ContextQueryService,
    store_context: StoreContextService,
    calibration: CalibrationService,
}

impl SttpRuntime {
    fn new(store: Arc<dyn NodeStore>) -> Self {
        let validator = Arc::new(TreeSitterValidator::new());
        Self {
            store: store.clone(),
            context_query: ContextQueryService::new(store.clone()),
            store_context: StoreContextService::new(store.clone(), validator),
            calibration: CalibrationService::new(store),
        }
    }
}

struct SurrealSdkClient {
    db: Surreal<SurrealAny>,
}

impl SurrealSdkClient {
    async fn connect(endpoint: &str, namespace: &str, database: &str) -> Result<Self, String> {
        let db = surreal_connect(endpoint)
            .await
            .map_err(|err| map_err("failed to connect local SurrealDB", err))?;

        db.use_ns(namespace)
            .use_db(database)
            .await
            .map_err(|err| map_err("failed to initialize SurrealDB namespace/database", err))?;

        Ok(Self { db })
    }
}

#[async_trait]
impl SurrealDbClient for SurrealSdkClient {
    async fn raw_query(&self, query: &str, parameters: QueryParams) -> anyhow::Result<Vec<Value>> {
        let mut request = self.db.query(query);
        for (key, value) in parameters {
            request = request.bind((key, value));
        }

        let mut response = request.await?.check()?;

        match response.take::<Vec<Value>>(0) {
            Ok(rows) => Ok(rows),
            Err(_) => Ok(Vec::new()),
        }
    }
}

struct AppState {
    http: Client,
    gateway_base_url: RwLock<String>,
    ollama_base_url: RwLock<String>,
    ollama_model: RwLock<String>,
    layout_overrides: RwLock<LayoutOverrides>,
    config_path: RwLock<Option<PathBuf>>,
    sttp_runtime: RwLock<Arc<SttpRuntime>>,
    sttp_runtime_label: RwLock<String>,
}

impl Default for AppState {
    fn default() -> Self {
        let (sttp_runtime, sttp_runtime_label) = build_in_memory_runtime();

        Self {
            http: Client::new(),
            gateway_base_url: RwLock::new(DEFAULT_GATEWAY_BASE_URL.to_string()),
            ollama_base_url: RwLock::new(DEFAULT_OLLAMA_BASE_URL.to_string()),
            ollama_model: RwLock::new(DEFAULT_OLLAMA_MODEL.to_string()),
            layout_overrides: RwLock::new(LayoutOverrides::default()),
            config_path: RwLock::new(None),
            sttp_runtime: RwLock::new(sttp_runtime),
            sttp_runtime_label: RwLock::new(sttp_runtime_label),
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
    #[serde(default)]
    duplicate_skipped: bool,
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
    sync_key: String,
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

fn join_url(base_url: &str, path: &str) -> Result<String, String> {
    let normalized_base = if base_url.ends_with('/') {
        base_url.to_string()
    } else {
        format!("{base_url}/")
    };

    Url::parse(&normalized_base)
        .and_then(|base| base.join(path.trim_start_matches('/')))
        .map(|url| url.to_string())
        .map_err(|err| map_err("failed to build request url", err))
}

fn map_err(prefix: &str, err: impl std::fmt::Display) -> String {
    format!("{prefix}: {err}")
}

fn build_in_memory_runtime() -> (Arc<SttpRuntime>, String) {
    let store = Arc::new(InMemoryNodeStore::new());
    let initializer: Arc<dyn NodeStoreInitializer> = store.clone();

    if let Err(err) = tauri::async_runtime::block_on(async { initializer.initialize_async().await }) {
        eprintln!("in-memory STTP store initialize warning: {err}");
    }

    let store_trait: Arc<dyn NodeStore> = store;
    (
        Arc::new(SttpRuntime::new(store_trait)),
        "sttp-core-rs (in-memory fallback)".to_string(),
    )
}

fn build_surreal_runtime(config_dir: &Path) -> Result<(Arc<SttpRuntime>, String), String> {
    let db_path = config_dir.join(LOCAL_STTP_DB_FILE_NAME);
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| map_err("failed to create local db directory", err))?;
    }

    let endpoint = format!("surrealkv://{}", db_path.to_string_lossy());
    let namespace = "resonantia";
    let database = "sttp-local";

    let client = tauri::async_runtime::block_on(async {
        SurrealSdkClient::connect(&endpoint, namespace, database).await
    })?;

    let store = Arc::new(SurrealDbNodeStore::new(Arc::new(client)));
    let initializer: Arc<dyn NodeStoreInitializer> = store.clone();
    tauri::async_runtime::block_on(async { initializer.initialize_async().await })
        .map_err(|err| map_err("failed to initialize local STTP schema", err))?;

    let store_trait: Arc<dyn NodeStore> = store;

    Ok((
        Arc::new(SttpRuntime::new(store_trait)),
        format!("sttp-core-rs (surrealdb local: {endpoint})"),
    ))
}

fn ensure_sttp_runtime_initialized(state: &AppState, config_dir: &Path) {
    match build_surreal_runtime(config_dir) {
        Ok((runtime, label)) => {
            if let Ok(mut guard) = state.sttp_runtime.write() {
                *guard = runtime;
            }
            if let Ok(mut guard) = state.sttp_runtime_label.write() {
                *guard = label;
            }
        }
        Err(err) => {
            eprintln!("local surreal runtime init warning: {err}");
            let (fallback_runtime, fallback_label) = build_in_memory_runtime();
            if let Ok(mut guard) = state.sttp_runtime.write() {
                *guard = fallback_runtime;
            }
            if let Ok(mut guard) = state.sttp_runtime_label.write() {
                *guard = fallback_label;
            }
        }
    }
}

fn sttp_runtime_handle(state: &AppState) -> Result<Arc<SttpRuntime>, String> {
    state
        .sttp_runtime
        .read()
        .map_err(|err| map_err("failed to read STTP runtime", err))
        .map(|guard| guard.clone())
}

fn sttp_transport_label(state: &AppState) -> String {
    state
        .sttp_runtime_label
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| "sttp-core-rs (runtime unavailable)".to_string())
}

fn to_ui_avec(value: sttp_core_rs::AvecState) -> AvecState {
    AvecState {
        stability: value.stability,
        friction: value.friction,
        logic: value.logic,
        autonomy: value.autonomy,
        psi: value.psi(),
    }
}

fn to_ui_node(node: sttp_core_rs::SttpNode) -> NodeDto {
    let timestamp = node.timestamp.to_rfc3339();
    let sync_key = node_sync_key_from_sttp(&node);
    let synthetic_id = node_fingerprint(
        &node.session_id,
        &timestamp,
        &node.tier,
        node.parent_node_id.as_deref(),
        node.psi,
    );

    NodeDto {
        raw: node.raw,
        session_id: node.session_id,
        tier: node.tier,
        timestamp,
        compression_depth: node.compression_depth,
        parent_node_id: node.parent_node_id,
        user_avec: to_ui_avec(node.user_avec),
        model_avec: to_ui_avec(node.model_avec),
        compression_avec: node.compression_avec.map(to_ui_avec),
        rho: node.rho,
        kappa: node.kappa,
        psi: node.psi,
        sync_key,
        synthetic_id,
    }
}

fn session_graph_id(session_id: &str) -> String {
    if session_id.starts_with("s:") {
        session_id.to_string()
    } else {
        format!("s:{session_id}")
    }
}

fn node_label(node: &NodeDto) -> String {
    let date = node.timestamp.chars().take(10).collect::<String>();
    format!("{} · {}", node.tier, date)
}

fn build_graph_response(nodes_response: &ListNodesResponse) -> GraphResponse {
    let mut sessions: HashMap<String, GraphSessionDto> = HashMap::new();
    let mut nodes = Vec::with_capacity(nodes_response.nodes.len());

    for node in &nodes_response.nodes {
        let graph_session_id = session_graph_id(&node.session_id);
        let entry = sessions
            .entry(graph_session_id.clone())
            .or_insert_with(|| GraphSessionDto {
                id: graph_session_id.clone(),
                label: node.session_id.clone(),
                node_count: 0,
                avg_psi: 0.0,
                last_modified: node.timestamp.clone(),
                size: 0,
            });

        entry.node_count += 1;
        entry.avg_psi += node.psi;
        if node.timestamp > entry.last_modified {
            entry.last_modified = node.timestamp.clone();
        }

        nodes.push(GraphNodeDto {
            id: node.synthetic_id.clone(),
            session_id: node.session_id.clone(),
            label: node_label(node),
            tier: node.tier.clone(),
            timestamp: node.timestamp.clone(),
            psi: node.psi,
            parent_node_id: node.parent_node_id.clone(),
            size: ((node.psi * 6.0).round() as i32).clamp(4, 24),
            synthetic_id: node.synthetic_id.clone(),
        });
    }

    let mut sessions = sessions
        .into_values()
        .map(|mut session| {
            if session.node_count > 0 {
                session.avg_psi /= session.node_count as f32;
            }
            session.size = (session.node_count * 2).clamp(8, 42);
            session
        })
        .collect::<Vec<_>>();

    sessions.sort_by(|left, right| right.last_modified.cmp(&left.last_modified));

    let mut edges = Vec::new();

    for index in 0..sessions.len() {
        if index + 1 < sessions.len() {
            let source = sessions[index].id.clone();
            let target = sessions[index + 1].id.clone();
            edges.push(GraphEdgeDto {
                id: format!("temporal:{source}->{target}"),
                source,
                target,
                kind: "temporal".to_string(),
            });
        }
    }

    for left in 0..sessions.len() {
        for right in (left + 1)..sessions.len() {
            if edges.len() >= 120 {
                break;
            }

            let diff = (sessions[left].avg_psi - sessions[right].avg_psi).abs();
            if diff <= 0.45 {
                let source = sessions[left].id.clone();
                let target = sessions[right].id.clone();
                edges.push(GraphEdgeDto {
                    id: format!("resonance:{source}->{target}"),
                    source,
                    target,
                    kind: "resonance".to_string(),
                });
            }
        }
    }

    GraphResponse {
        retrieved: nodes_response.retrieved,
        sessions,
        nodes,
        edges,
    }
}

fn drift_label(drift: sttp_core_rs::DriftClassification) -> String {
    match drift {
        sttp_core_rs::DriftClassification::Intentional => "Intentional".to_string(),
        sttp_core_rs::DriftClassification::Uncontrolled => "Uncontrolled".to_string(),
    }
}

fn quantize(value: f32) -> String {
    format!("{value:.6}")
}

fn node_sync_key_from_sttp(node: &sttp_core_rs::SttpNode) -> String {
    let compression = node.compression_avec.unwrap_or(node.model_avec);
    let canonical = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        node.session_id.trim(),
        node.tier.trim(),
        node.timestamp.to_rfc3339(),
        node.compression_depth,
        node.parent_node_id.as_deref().unwrap_or("").trim(),
        quantize(node.psi),
        quantize(node.rho),
        quantize(node.kappa),
        quantize(node.user_avec.stability),
        quantize(node.user_avec.friction),
        quantize(node.user_avec.logic),
        quantize(node.user_avec.autonomy),
        quantize(node.model_avec.stability),
        quantize(node.model_avec.friction),
        quantize(node.model_avec.logic),
        quantize(node.model_avec.autonomy),
        quantize(compression.stability),
        quantize(compression.friction),
        quantize(compression.logic),
        quantize(compression.autonomy),
        quantize(node.user_avec.psi()),
        quantize(node.model_avec.psi()),
        quantize(compression.psi()),
        quantize(compression.psi() - node.model_avec.psi()),
    );

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex_encode(hasher.finalize())
}

async fn find_duplicate_node_by_sync_key(
    runtime: &SttpRuntime,
    raw_node: &str,
    requested_session_id: &str,
) -> Result<Option<sttp_core_rs::SttpNode>, String> {
    let parser = SttpNodeParser::new();
    let parse = parser.try_parse(raw_node, requested_session_id);
    if !parse.success {
        return Ok(None);
    }

    let Some(candidate) = parse.node else {
        return Ok(None);
    };

    let candidate_key = node_sync_key_from_sttp(&candidate);
    let existing_nodes = runtime
        .store
        .query_nodes_async(NodeQuery {
            limit: STORE_DEDUPE_SCAN_LIMIT,
            session_id: Some(candidate.session_id.clone()),
            from_utc: None,
            to_utc: None,
        })
        .await
        .map_err(|err| map_err("duplicate scan query failed", err))?;

    for existing in existing_nodes {
        if node_sync_key_from_sttp(&existing) == candidate_key {
            return Ok(Some(existing));
        }
    }

    Ok(None)
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
    let mut cleaned = thinking_re.replace(text, "").replace("\r\n", "\n").trim().to_string();

    for label in [
        "Topic",
        "What happened",
        "Where we left off",
        "Vibe",
        "Pick back up with",
    ] {
        let markdown_pattern = format!(r"(?im)^\s*(?:[-*]\s*)?\*\*?{}\*\*?\s*:", regex::escape(label));
        let markdown_re = match Regex::new(&markdown_pattern) {
            Ok(value) => value,
            Err(_) => continue,
        };
        cleaned = markdown_re
            .replace_all(&cleaned, format!("{label}:"))
            .to_string();
    }

    let labels = [
        "Topic",
        "What happened",
        "Where we left off",
        "Vibe",
        "Pick back up with",
    ];

    let lower = cleaned.to_lowercase();
    let mut positions: Vec<(usize, &'static str)> = labels
        .iter()
        .filter_map(|label| {
            let needle = format!("{}:", label.to_lowercase());
            lower.find(&needle).map(|index| (index, *label))
        })
        .collect();

    positions.sort_by_key(|(index, _)| *index);

    let extract_section = |label: &str| -> String {
        let Some((start, _)) = positions.iter().find(|(_, current)| *current == label) else {
            return String::new();
        };

        let header_len = label.len() + 1;
        let content_start = start + header_len;
        let content_end = positions
            .iter()
            .filter(|(index, _)| *index > *start)
            .map(|(index, _)| *index)
            .min()
            .unwrap_or(cleaned.len());

        cleaned[content_start..content_end]
            .trim()
            .trim_matches('*')
            .trim()
            .to_string()
    };

    let topic = extract_section("Topic");
    let what_happened = extract_section("What happened");
    let where_we_left_off = extract_section("Where we left off");
    let vibe = extract_section("Vibe");
    let pick_back_up_with = extract_section("Pick back up with");

    if topic.trim().is_empty() && what_happened.trim().is_empty() {
        let fallback = cleaned.trim();
        if fallback.is_empty() {
            return None;
        }

        let fallback_topic = fallback
            .lines()
            .next()
            .unwrap_or("transmutation")
            .trim()
            .trim_matches('*')
            .trim_end_matches(':')
            .to_string();

        return Some(AiSummary {
            topic: if fallback_topic.is_empty() { "transmutation".to_string() } else { fallback_topic },
            what_happened: fallback.to_string(),
            where_we_left_off: String::new(),
            vibe: String::new(),
            pick_back_up_with: String::new(),
        });
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
    let runtime = sttp_runtime_handle(&state)?;

    runtime
        .context_query
        .list_nodes_async(1, None)
        .await
        .map_err(|err| map_err("local STTP health check failed", err))?;

    Ok(HealthResponse {
        status: "ok".to_string(),
        transport: sttp_transport_label(&state),
    })
}

#[tauri::command]
async fn list_nodes(
    state: tauri::State<'_, AppState>,
    limit: i32,
    session_id: Option<String>,
) -> Result<ListNodesResponse, String> {
    let runtime = sttp_runtime_handle(&state)?;
    let capped_limit = limit.clamp(1, 400) as usize;
    let session_filter = session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let listed = runtime
        .context_query
        .list_nodes_async(capped_limit, session_filter)
        .await
        .map_err(|err| map_err("local list nodes failed", err))?;

    Ok(ListNodesResponse {
        nodes: listed.nodes.into_iter().map(to_ui_node).collect(),
        retrieved: listed.retrieved as i32,
    })
}

#[tauri::command]
async fn get_graph(
    state: tauri::State<'_, AppState>,
    limit: i32,
    session_id: Option<String>,
) -> Result<GraphResponse, String> {
    let listed = list_nodes(state, limit, session_id).await?;
    Ok(build_graph_response(&listed))
}

#[tauri::command]
async fn store_context(
    state: tauri::State<'_, AppState>,
    request: StoreContextRequest,
) -> Result<StoreContextResponse, String> {
    let runtime = sttp_runtime_handle(&state)?;
    let session_id = request.session_id.trim();
    let effective_session_id = if session_id.is_empty() {
        "resonantia-local"
    } else {
        session_id
    };

    match find_duplicate_node_by_sync_key(&runtime, &request.node, effective_session_id).await {
        Ok(Some(existing)) => {
            let existing_timestamp = existing.timestamp.to_rfc3339();
            let existing_synthetic_id = node_fingerprint(
                &existing.session_id,
                &existing_timestamp,
                &existing.tier,
                existing.parent_node_id.as_deref(),
                existing.psi,
            );

            return Ok(StoreContextResponse {
                node_id: format!("dup:{existing_synthetic_id}"),
                psi: existing.psi,
                valid: true,
                validation_error: None,
                duplicate_skipped: true,
            });
        }
        Ok(None) => {}
        Err(err) => eprintln!("duplicate check warning: {err}"),
    }

    let result = runtime
        .store_context
        .store_async(&request.node, effective_session_id)
        .await;

    Ok(StoreContextResponse {
        node_id: result.node_id,
        psi: result.psi,
        valid: result.valid,
        validation_error: result.validation_error,
        duplicate_skipped: false,
    })
}

#[tauri::command]
async fn calibrate_session(
    state: tauri::State<'_, AppState>,
    request: CalibrateSessionRequest,
) -> Result<CalibrateSessionResponse, String> {
    let runtime = sttp_runtime_handle(&state)?;
    let session_id = request.session_id.trim();
    let effective_session_id = if session_id.is_empty() {
        "resonantia-local"
    } else {
        session_id
    };

    let result = runtime
        .calibration
        .calibrate_async(
            effective_session_id,
            request.stability,
            request.friction,
            request.logic,
            request.autonomy,
            &request.trigger,
        )
        .await
        .map_err(|err| map_err("local calibration failed", err))?;

    Ok(CalibrateSessionResponse {
        previous_avec: to_ui_avec(result.previous_avec),
        delta: result.delta,
        drift_classification: drift_label(result.drift_classification),
        trigger: result.trigger,
        trigger_history: result.trigger_history,
        is_first_calibration: result.is_first_calibration,
    })
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

    let url = join_url(&ollama_base_url, "/api/chat")?;
    let payload = OllamaChatRequest {
        model,
        messages: vec![OllamaMessage {
            role: "user".to_string(),
            content: raw_node,
        }],
        stream: false,
    };

    eprintln!(
        "AI summary requested · model={} url={} nodeLength={}",
        payload.model,
        url,
        payload.messages.first().map(|message| message.content.len()).unwrap_or(0)
    );

    let response = state
        .http
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|err| {
            eprintln!("AI summary HTTP request failed · model={} error={}", payload.model, err);
            map_err("ollama request failed", err)
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        eprintln!(
            "AI summary non-success response · model={} status={} body={}",
            payload.model,
            status,
            body
        );
        return Err(format!("ollama response status failed: {} {}", status, body));
    }

    let response = response
        .json::<OllamaChatResponse>()
        .await
        .map_err(|err| {
            eprintln!("AI summary response parse failed · model={} error={}", payload.model, err);
            map_err("ollama response parse failed", err)
        })?;

    let text = match response.message {
        Some(message) => message.content,
        None => {
            eprintln!("AI summary returned no message content · model={}", payload.model);
            return Ok(None);
        }
    };

    eprintln!(
        "AI summary raw response received · model={} responseLength={}",
        payload.model,
        text.len()
    );

    let parsed = parse_ai_response(&text);
    if parsed.is_none() {
        eprintln!("AI summary parse returned no recognizable sections · model={}", payload.model);
    }
    Ok(parsed)
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

            ensure_sttp_runtime_initialized(&state, &config_dir);
            eprintln!("sttp runtime ready: {}", sttp_transport_label(&state));

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

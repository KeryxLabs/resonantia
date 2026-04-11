use async_trait::async_trait;
use chrono::{DateTime, Utc};
use hex::encode as hex_encode;
use regex::Regex;
use reqwest::Client;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use sttp_core_rs::storage::{QueryParams, SurrealDbClient, SurrealDbNodeStore};
use sttp_core_rs::{
    CalibrationService, ChangeQueryResult, ConnectorMetadata, ContextQueryService,
    InMemoryNodeStore, NodeStore, NodeStoreInitializer, NodeUpsertStatus, NodeValidator,
    SttpNode, SttpNodeParser, SyncChangeSource, SyncCheckpoint, SyncCoordinatorPolicy,
    SyncCoordinatorService, SyncCursor, SyncPullRequest, SyncPullResult, TreeSitterValidator,
};
use surrealdb::engine::any::{connect as surreal_connect, Any as SurrealAny};
use surrealdb::Surreal;
use tauri::Manager;

const DEFAULT_GATEWAY_BASE_URL: &str = "";
const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_OLLAMA_MODEL: &str = "gemma3";
const APP_CONFIG_FILE_NAME: &str = "resonantia-config.json";
const LOCAL_STTP_DB_FILE_NAME: &str = "sttp-local.db";
const GATEWAY_LIST_NODES_PATH: &str = "/api/v1/nodes";
const GATEWAY_STORE_CONTEXT_PATH: &str = "/api/v1/store";
const GATEWAY_PAGE_OVERSCAN: usize = 3;

struct SttpRuntime {
    store: Arc<dyn NodeStore>,
    validator: Arc<dyn NodeValidator>,
    context_query: ContextQueryService,
    calibration: CalibrationService,
}

impl SttpRuntime {
    fn new(store: Arc<dyn NodeStore>) -> Self {
        let validator: Arc<dyn NodeValidator> = Arc::new(TreeSitterValidator::new());
        Self {
            store: store.clone(),
            validator: validator.clone(),
            context_query: ContextQueryService::new(store.clone()),
            calibration: CalibrationService::new(store),
        }
    }
}

struct LocalStoreChangeSource {
    store: Arc<dyn NodeStore>,
}

impl LocalStoreChangeSource {
    fn new(store: Arc<dyn NodeStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl SyncChangeSource for LocalStoreChangeSource {
    async fn read_changes_async(
        &self,
        session_id: &str,
        _connector_id: &str,
        cursor: Option<SyncCursor>,
        limit: usize,
    ) -> anyhow::Result<ChangeQueryResult> {
        self.store
            .query_changes_since_async(session_id, cursor, limit)
            .await
    }
}

struct GatewayChangeSource {
    http: Client,
    base_url: String,
}

impl GatewayChangeSource {
    fn new(http: Client, base_url: String) -> Self {
        Self { http, base_url }
    }

    async fn query_via_list_nodes(
        &self,
        session_filter: Option<&str>,
        cursor: Option<SyncCursor>,
        limit: usize,
    ) -> anyhow::Result<ChangeQueryResult> {
        let overscan = limit
            .saturating_mul(GATEWAY_PAGE_OVERSCAN)
            .saturating_add(1)
            .clamp(1, 5000);

        let base = join_url(&self.base_url, GATEWAY_LIST_NODES_PATH).map_err(anyhow::Error::msg)?;
        let mut url = Url::parse(&base)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("limit", &overscan.to_string());
            if let Some(session_id) = session_filter.map(str::trim).filter(|value| !value.is_empty()) {
                pairs.append_pair("sessionId", session_id);
            }
        }

        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "gateway list nodes failed: {} {}",
                status,
                body
            ));
        }

        let payload = response.json::<GatewayListNodesResponse>().await?;
        gateway_list_nodes_to_change_result(payload, cursor, limit)
    }
}

#[async_trait]
impl SyncChangeSource for GatewayChangeSource {
    async fn read_changes_async(
        &self,
        session_id: &str,
        _connector_id: &str,
        cursor: Option<SyncCursor>,
        limit: usize,
    ) -> anyhow::Result<ChangeQueryResult> {
        self.query_via_list_nodes(Some(session_id), cursor, limit)
            .await
    }
}

struct ResonantiaSyncPolicy {
    min_psi: Option<f32>,
    blocked_tiers: Vec<String>,
}

impl ResonantiaSyncPolicy {
    fn new(min_psi: Option<f32>, blocked_tiers: Option<Vec<String>>) -> Self {
        let blocked_tiers = blocked_tiers
            .unwrap_or_default()
            .into_iter()
            .map(|tier| tier.trim().to_ascii_lowercase())
            .filter(|tier| !tier.is_empty())
            .collect();

        Self {
            min_psi,
            blocked_tiers,
        }
    }

    fn is_blocked_tier(&self, tier: &str) -> bool {
        let normalized = tier.trim().to_ascii_lowercase();
        self.blocked_tiers.iter().any(|blocked| blocked == &normalized)
    }
}

impl SyncCoordinatorPolicy for ResonantiaSyncPolicy {
    fn should_accept_node(&self, node: &SttpNode) -> bool {
        if self.is_blocked_tier(&node.tier) {
            return false;
        }

        match self.min_psi {
            Some(min_psi) => node.psi >= min_psi,
            None => true,
        }
    }

    fn checkpoint_metadata(
        &self,
        _session_id: &str,
        connector_id: &str,
        previous: Option<&SyncCheckpoint>,
        last_applied_node: Option<&SttpNode>,
        next_cursor: Option<&SyncCursor>,
    ) -> Option<ConnectorMetadata> {
        let observed_at_utc = next_cursor
            .map(|cursor| cursor.updated_at)
            .or_else(|| last_applied_node.map(|node| node.updated_at))
            .or_else(|| previous.map(|checkpoint| checkpoint.updated_at))?;

        let upstream_id = last_applied_node
            .map(sync_key_for_node)
            .or_else(|| next_cursor.map(|cursor| cursor.sync_key.clone()))
            .or_else(|| {
                previous.and_then(|checkpoint| {
                    checkpoint
                        .metadata
                        .as_ref()
                        .map(|metadata| metadata.upstream_id.clone())
                })
            })
            .unwrap_or_else(|| connector_id.to_string());

        Some(ConnectorMetadata {
            connector_id: connector_id.to_string(),
            source_kind: "resonantia-sync-policy".to_string(),
            upstream_id,
            revision: next_cursor.map(|cursor| cursor.sync_key.clone()),
            observed_at_utc,
            extra: Some(json!({
                "blockedTiers": &self.blocked_tiers,
                "minPsi": self.min_psi,
                "checkpointSource": "resonantia-policy",
            })),
        })
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

    async fn ensure_source_metadata_schema(&self) -> Result<(), String> {
        // sttp-core-rs serializes ConnectorMetadata in camelCase; define nested
        // fields so SCHEMAFULL temporal_node accepts source_metadata payloads.
        let query = r#"
            DEFINE FIELD OVERWRITE source_metadata.connectorId   ON temporal_node TYPE option<string>;
            DEFINE FIELD OVERWRITE source_metadata.sourceKind    ON temporal_node TYPE option<string>;
            DEFINE FIELD OVERWRITE source_metadata.upstreamId    ON temporal_node TYPE option<string>;
            DEFINE FIELD OVERWRITE source_metadata.revision      ON temporal_node TYPE option<string>;
            DEFINE FIELD OVERWRITE source_metadata.observedAtUtc ON temporal_node TYPE option<string>;
            DEFINE FIELD OVERWRITE source_metadata.extra         ON temporal_node TYPE option<object>;
        "#;

        self.raw_query(query, QueryParams::new())
            .await
            .map(|_| ())
            .map_err(|err| map_err("failed to ensure source metadata schema", err))
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
    #[serde(default)]
    upsert_status: Option<String>,
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
struct SyncPullCommandRequest {
    session_id: String,
    connector_id: String,
    source: Option<String>,
    gateway_base_url: Option<String>,
    page_size: Option<i32>,
    max_batches: Option<i32>,
    min_psi: Option<f32>,
    blocked_tiers: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncNowRequest {
    session_id: Option<String>,
    gateway_base_url: Option<String>,
    page_size: Option<i32>,
    max_batches: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GatewayAvecDto {
    stability: f32,
    friction: f32,
    logic: f32,
    autonomy: f32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GatewayNodeDto {
    raw: String,
    session_id: String,
    tier: String,
    timestamp: String,
    compression_depth: i32,
    parent_node_id: Option<String>,
    user_avec: GatewayAvecDto,
    model_avec: GatewayAvecDto,
    compression_avec: Option<GatewayAvecDto>,
    rho: f32,
    kappa: f32,
    psi: f32,
    #[serde(default)]
    sync_key: String,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    source_metadata: Option<ConnectorMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GatewayListNodesResponse {
    #[serde(default)]
    nodes: Vec<GatewayNodeDto>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncCursorDto {
    updated_at: String,
    sync_key: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncCheckpointDto {
    session_id: String,
    connector_id: String,
    cursor: Option<SyncCursorDto>,
    updated_at: String,
    metadata: Option<ConnectorMetadata>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncPullCommandResponse {
    source: String,
    fetched: i32,
    created: i32,
    updated: i32,
    duplicate: i32,
    skipped: i32,
    filtered: i32,
    batches: i32,
    has_more: bool,
    last_cursor: Option<SyncCursorDto>,
    checkpoint: Option<SyncCheckpointDto>,
}

#[derive(Debug, Default)]
struct SyncUploadSummary {
    uploaded: i32,
    duplicate: i32,
    rejected: i32,
    batches: i32,
    has_more: bool,
}

#[derive(Debug, Default)]
struct SyncDownloadSummary {
    fetched: i32,
    created: i32,
    updated: i32,
    duplicate: i32,
    skipped: i32,
    filtered: i32,
    batches: i32,
    has_more: bool,
}

#[derive(Debug, Default)]
struct GatewayStoreOutcome {
    valid: bool,
    duplicate: bool,
    validation_error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncNowResponse {
    session_id: String,
    remote_base_url: String,
    upload: SyncUploadStats,
    download: SyncDownloadStats,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncUploadStats {
    uploaded: i32,
    duplicate: i32,
    rejected: i32,
    batches: i32,
    has_more: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncDownloadStats {
    fetched: i32,
    created: i32,
    updated: i32,
    duplicate: i32,
    skipped: i32,
    filtered: i32,
    batches: i32,
    has_more: bool,
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

    tauri::async_runtime::block_on(async { client.ensure_source_metadata_schema().await })?;

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
    let sync_key = sync_key_for_node(&node);
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

fn sync_key_for_node(node: &SttpNode) -> String {
    let existing = node.sync_key.trim();
    if !existing.is_empty() {
        return existing.to_string();
    }

    node.canonical_sync_key()
}

fn node_upsert_status_label(status: NodeUpsertStatus) -> String {
    match status {
        NodeUpsertStatus::Created => "created".to_string(),
        NodeUpsertStatus::Updated => "updated".to_string(),
        NodeUpsertStatus::Duplicate => "duplicate".to_string(),
        NodeUpsertStatus::Skipped => "skipped".to_string(),
    }
}

fn is_duplicate_upsert_status(status: NodeUpsertStatus) -> bool {
    matches!(status, NodeUpsertStatus::Duplicate | NodeUpsertStatus::Skipped)
}

fn to_sync_cursor_dto(cursor: SyncCursor) -> SyncCursorDto {
    SyncCursorDto {
        updated_at: cursor.updated_at.to_rfc3339(),
        sync_key: cursor.sync_key,
    }
}

fn to_sync_checkpoint_dto(checkpoint: SyncCheckpoint) -> SyncCheckpointDto {
    SyncCheckpointDto {
        session_id: checkpoint.session_id,
        connector_id: checkpoint.connector_id,
        cursor: checkpoint.cursor.map(to_sync_cursor_dto),
        updated_at: checkpoint.updated_at.to_rfc3339(),
        metadata: checkpoint.metadata,
    }
}

fn parse_rfc3339_utc(value: &str, field_name: &str) -> anyhow::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|err| anyhow::anyhow!("invalid {} datetime '{}': {}", field_name, value, err))
}

fn gateway_avec_to_core(dto: GatewayAvecDto) -> sttp_core_rs::AvecState {
    sttp_core_rs::AvecState {
        stability: dto.stability,
        friction: dto.friction,
        logic: dto.logic,
        autonomy: dto.autonomy,
    }
}

fn gateway_node_to_sttp(dto: GatewayNodeDto) -> anyhow::Result<SttpNode> {
    let timestamp = parse_rfc3339_utc(&dto.timestamp, "timestamp")?;
    let updated_at = match dto.updated_at {
        Some(value) if !value.trim().is_empty() => parse_rfc3339_utc(value.trim(), "updatedAt")?,
        _ => timestamp,
    };

    let mut node = SttpNode {
        raw: dto.raw,
        session_id: dto.session_id,
        tier: dto.tier,
        timestamp,
        compression_depth: dto.compression_depth,
        parent_node_id: dto.parent_node_id,
        sync_key: dto.sync_key.trim().to_string(),
        updated_at,
        source_metadata: dto.source_metadata,
        user_avec: gateway_avec_to_core(dto.user_avec),
        model_avec: gateway_avec_to_core(dto.model_avec),
        compression_avec: dto.compression_avec.map(gateway_avec_to_core),
        rho: dto.rho,
        kappa: dto.kappa,
        psi: dto.psi,
    };

    if node.sync_key.trim().is_empty() {
        node.sync_key = node.canonical_sync_key();
    }

    if let Some(metadata) = node.source_metadata.as_mut() {
        // Normalize metadata for SCHEMAFULL local storage compatibility.
        if metadata.connector_id.trim().is_empty() {
            metadata.connector_id = "gateway:download".to_string();
        }
        if metadata.source_kind.trim().is_empty() {
            metadata.source_kind = "resonantia-gateway".to_string();
        }
        if metadata.upstream_id.trim().is_empty() {
            metadata.upstream_id = node.sync_key.clone();
        }
        if metadata
            .revision
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            metadata.revision = Some(node.sync_key.clone());
        }

        // Keep nested payload deterministic and non-null.
        metadata.extra = Some(json!({}));
    }

    // Local Surreal schema expects `none | object` for source metadata.
    // Gateway payloads can include explicit null, so synthesize metadata when absent.
    if node.source_metadata.is_none() {
        node.source_metadata = Some(ConnectorMetadata {
            connector_id: "gateway:download".to_string(),
            source_kind: "resonantia-gateway".to_string(),
            upstream_id: node.sync_key.clone(),
            revision: Some(node.sync_key.clone()),
            observed_at_utc: updated_at,
            extra: Some(json!({})),
        });
    }

    Ok(node)
}

fn compare_cursor_markers(
    left_updated_at: DateTime<Utc>,
    left_sync_key: &str,
    right_updated_at: DateTime<Utc>,
    right_sync_key: &str,
) -> std::cmp::Ordering {
    match left_updated_at.cmp(&right_updated_at) {
        std::cmp::Ordering::Equal => left_sync_key.cmp(right_sync_key),
        value => value,
    }
}

fn cursor_from_node(node: &SttpNode) -> SyncCursor {
    SyncCursor {
        updated_at: node.updated_at,
        sync_key: sync_key_for_node(node),
    }
}

fn node_is_after_cursor(node: &SttpNode, cursor: &SyncCursor) -> bool {
    compare_cursor_markers(
        node.updated_at,
        &sync_key_for_node(node),
        cursor.updated_at,
        &cursor.sync_key,
    ) == std::cmp::Ordering::Greater
}

fn sort_nodes_by_cursor(nodes: &mut [SttpNode]) {
    nodes.sort_by(|left, right| {
        compare_cursor_markers(
            left.updated_at,
            &sync_key_for_node(left),
            right.updated_at,
            &sync_key_for_node(right),
        )
    });
}

fn gateway_list_nodes_to_change_result(
    payload: GatewayListNodesResponse,
    cursor: Option<SyncCursor>,
    limit: usize,
) -> anyhow::Result<ChangeQueryResult> {
    let mut nodes = payload
        .nodes
        .into_iter()
        .map(gateway_node_to_sttp)
        .collect::<anyhow::Result<Vec<_>>>()?;
    sort_nodes_by_cursor(&mut nodes);

    let mut filtered = match cursor {
        Some(cursor_value) => nodes
            .into_iter()
            .filter(|node| node_is_after_cursor(node, &cursor_value))
            .collect::<Vec<_>>(),
        None => nodes,
    };

    let has_more = filtered.len() > limit;
    if filtered.len() > limit {
        filtered.truncate(limit);
    }

    let next_cursor = filtered.last().map(cursor_from_node);

    Ok(ChangeQueryResult {
        nodes: filtered,
        next_cursor,
        has_more,
    })
}

enum SyncSourceKind {
    Local,
    Gateway,
}

fn resolve_sync_source(source: Option<&str>, connector_id: &str) -> SyncSourceKind {
    let normalized_source = source
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());

    if let Some(value) = normalized_source {
        if value == "gateway" || value == "cloud" || value == "remote" {
            return SyncSourceKind::Gateway;
        }
        return SyncSourceKind::Local;
    }

    let normalized_connector = connector_id.trim().to_ascii_lowercase();
    if normalized_connector.starts_with("gateway:")
        || normalized_connector.starts_with("cloud:")
        || normalized_connector.starts_with("remote:")
    {
        return SyncSourceKind::Gateway;
    }

    SyncSourceKind::Local
}

fn effective_gateway_base_url(state: &AppState, override_base_url: Option<&str>) -> Result<String, String> {
    if let Some(value) = override_base_url {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    state
        .gateway_base_url
        .read()
        .map_err(|err| map_err("failed to read gateway url", err))
        .map(|guard| guard.clone())
}

fn json_value_at_alias<'a>(root: &'a Value, aliases: &[&str]) -> Option<&'a Value> {
    aliases.iter().find_map(|key| root.get(*key))
}

fn parse_gateway_store_outcome(payload: &Value) -> GatewayStoreOutcome {
    let root = payload.get("result").unwrap_or(payload);
    let upsert_status = json_value_at_alias(root, &["upsertStatus", "upsert_status"])
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_lowercase());
    let duplicate_from_status = upsert_status
        .as_deref()
        .map(|value| value == "duplicate" || value == "skipped")
        .unwrap_or(false);

    GatewayStoreOutcome {
        valid: json_value_at_alias(root, &["valid", "isValid"])
            .and_then(Value::as_bool)
            .unwrap_or(true),
        duplicate: json_value_at_alias(root, &["duplicateSkipped", "duplicate_skipped"])
            .and_then(Value::as_bool)
            .unwrap_or(duplicate_from_status),
        validation_error: json_value_at_alias(root, &["validationError", "validation_error", "error"])
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
    }
}

fn node_originates_from_gateway(node: &SttpNode) -> bool {
    let Some(metadata) = node.source_metadata.as_ref() else {
        return false;
    };

    let connector = metadata.connector_id.trim().to_ascii_lowercase();
    let source_kind = metadata.source_kind.trim().to_ascii_lowercase();

    connector.contains("gateway")
        || connector.contains("cloud")
        || source_kind.contains("gateway")
        || source_kind.contains("cloud")
}

fn is_source_metadata_null_store_failure(message: &str) -> bool {
    let normalized = message.trim().to_ascii_lowercase();
    normalized.contains("source_metadata")
        && normalized.contains("none | object")
        && normalized.contains("null")
}

async fn store_node_to_gateway(
    http: &Client,
    gateway_base_url: &str,
    raw_node: &str,
    session_id: &str,
) -> Result<GatewayStoreOutcome, String> {
    let url = join_url(gateway_base_url, GATEWAY_STORE_CONTEXT_PATH)?;
    let payload = json!({ "node": raw_node, "sessionId": session_id });

    let response = http
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|err| map_err("gateway store request failed", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("gateway store failed: {} {}", status, body));
    }

    let payload = response.json::<Value>().await.unwrap_or(Value::Null);
    Ok(parse_gateway_store_outcome(&payload))
}

async fn push_local_changes_to_gateway(
    runtime: &SttpRuntime,
    http: &Client,
    gateway_base_url: &str,
    session_filter: Option<&str>,
    page_size: usize,
    max_batches: usize,
) -> Result<SyncUploadSummary, String> {
    let capped_batches = max_batches.max(1);
    let mut summary = SyncUploadSummary::default();
    let mut cursor: Option<SyncCursor> = None;
    let mut stop_upload_for_run = false;

    while (summary.batches as usize) < capped_batches {
        let page = local_outbound_changes_via_list_nodes(runtime, session_filter, cursor.clone(), page_size).await?;

        if page.nodes.is_empty() {
            summary.has_more = page.has_more;
            break;
        }

        summary.batches += 1;

        for node in page.nodes.iter() {
            if node_originates_from_gateway(node) {
                // Nodes pulled from gateway are already cloud-origin; skip re-upload.
                summary.duplicate += 1;
                continue;
            }

            let outcome = store_node_to_gateway(http, gateway_base_url, &node.raw, &node.session_id).await?;
            if !outcome.valid {
                summary.rejected += 1;
                if let Some(error) = outcome.validation_error {
                    if is_source_metadata_null_store_failure(&error) {
                        eprintln!(
                            "gateway rejected local upload due source_metadata NULL compatibility; skipping remaining uploads for this run"
                        );
                        stop_upload_for_run = true;
                        break;
                    }
                    eprintln!("gateway rejected node during upload: {error}");
                }
                continue;
            }

            summary.uploaded += 1;
            if outcome.duplicate {
                summary.duplicate += 1;
            }
        }

        if stop_upload_for_run {
            summary.has_more = true;
            break;
        }

        summary.has_more = page.has_more;
        cursor = page.next_cursor;

        if !summary.has_more || cursor.is_none() {
            break;
        }
    }

    Ok(summary)
}

async fn local_outbound_changes_via_list_nodes(
    runtime: &SttpRuntime,
    session_filter: Option<&str>,
    cursor: Option<SyncCursor>,
    limit: usize,
) -> Result<ChangeQueryResult, String> {
    let overscan = limit
        .saturating_mul(GATEWAY_PAGE_OVERSCAN)
        .saturating_add(1)
        .clamp(1, 5000);

    let mut nodes = runtime
        .store
        .list_nodes_async(overscan, session_filter)
        .await
        .map_err(|err| map_err("local outbound fallback query failed", err))?;

    sort_nodes_by_cursor(&mut nodes);

    let mut filtered = match cursor {
        Some(cursor_value) => nodes
            .into_iter()
            .filter(|node| node_is_after_cursor(node, &cursor_value))
            .collect::<Vec<_>>(),
        None => nodes,
    };

    let has_more = filtered.len() > limit;
    if filtered.len() > limit {
        filtered.truncate(limit);
    }

    let next_cursor = filtered.last().map(cursor_from_node);

    Ok(ChangeQueryResult {
        nodes: filtered,
        next_cursor,
        has_more,
    })
}

fn to_sync_upload_stats(summary: SyncUploadSummary) -> SyncUploadStats {
    SyncUploadStats {
        uploaded: summary.uploaded,
        duplicate: summary.duplicate,
        rejected: summary.rejected,
        batches: summary.batches,
        has_more: summary.has_more,
    }
}

fn to_sync_download_stats(summary: SyncDownloadSummary) -> SyncDownloadStats {
    SyncDownloadStats {
        fetched: summary.fetched,
        created: summary.created,
        updated: summary.updated,
        duplicate: summary.duplicate,
        skipped: summary.skipped,
        filtered: summary.filtered,
        batches: summary.batches,
        has_more: summary.has_more,
    }
}

async fn pull_gateway_changes_to_local(
    runtime: &SttpRuntime,
    http: &Client,
    gateway_base_url: &str,
    session_filter: Option<&str>,
    page_size: usize,
    max_batches: usize,
) -> Result<SyncDownloadSummary, String> {
    let capped_batches = max_batches.max(1);
    let source = GatewayChangeSource::new(http.clone(), gateway_base_url.to_string());
    let mut summary = SyncDownloadSummary::default();
    let mut cursor: Option<SyncCursor> = None;

    while (summary.batches as usize) < capped_batches {
        let page = source
            .query_via_list_nodes(session_filter, cursor.clone(), page_size)
            .await
            .map_err(|err| map_err("gateway full pull query failed", err))?;

        if page.nodes.is_empty() {
            summary.has_more = page.has_more;
            break;
        }

        summary.batches += 1;
        summary.fetched += page.nodes.len() as i32;

        for node in page.nodes.into_iter() {
            let upsert = runtime
                .store
                .upsert_node_async(node)
                .await
                .map_err(|err| map_err("local node upsert during download failed", err))?;

            match upsert.status {
                NodeUpsertStatus::Created => summary.created += 1,
                NodeUpsertStatus::Updated => summary.updated += 1,
                NodeUpsertStatus::Duplicate => summary.duplicate += 1,
                NodeUpsertStatus::Skipped => summary.skipped += 1,
            }
        }

        summary.has_more = page.has_more;
        cursor = page.next_cursor;

        if !summary.has_more || cursor.is_none() {
            break;
        }
    }

    Ok(summary)
}

async fn execute_sync_pull(
    state: &AppState,
    runtime: Arc<SttpRuntime>,
    request: SyncPullCommandRequest,
) -> Result<SyncPullCommandResponse, String> {
    let SyncPullCommandRequest {
        session_id,
        connector_id,
        source,
        gateway_base_url,
        page_size,
        max_batches,
        min_psi,
        blocked_tiers,
    } = request;

    let session_id = session_id.trim();
    let effective_session_id = if session_id.is_empty() {
        "resonantia-local"
    } else {
        session_id
    };

    let connector_id = connector_id.trim();
    let effective_connector_id = if connector_id.is_empty() {
        "resonantia-local"
    } else {
        connector_id
    };

    let source_kind = resolve_sync_source(source.as_deref(), effective_connector_id);
    let (source, source_label): (Arc<dyn SyncChangeSource>, &str) = match source_kind {
        SyncSourceKind::Local => (
            Arc::new(LocalStoreChangeSource::new(runtime.store.clone())),
            "local",
        ),
        SyncSourceKind::Gateway => {
            let base_url = effective_gateway_base_url(state, gateway_base_url.as_deref())?;
            (
                Arc::new(GatewayChangeSource::new(state.http.clone(), base_url)),
                "gateway",
            )
        }
    };

    let policy: Arc<dyn SyncCoordinatorPolicy> = Arc::new(ResonantiaSyncPolicy::new(
        min_psi,
        blocked_tiers,
    ));
    let coordinator = SyncCoordinatorService::with_policy(runtime.store.clone(), source, policy);

    let result = coordinator
        .pull_async(SyncPullRequest {
            session_id: effective_session_id.to_string(),
            connector_id: effective_connector_id.to_string(),
            page_size: page_size.unwrap_or(200).clamp(1, 500) as usize,
            max_batches: max_batches.map(|value| value.max(1) as usize),
        })
        .await
        .map_err(|err| map_err(&format!("{} sync pull failed", source_label), err))?;

    Ok(to_sync_pull_response(result, source_label))
}

fn to_sync_pull_response(result: SyncPullResult, source: &str) -> SyncPullCommandResponse {
    SyncPullCommandResponse {
        source: source.to_string(),
        fetched: result.fetched as i32,
        created: result.created as i32,
        updated: result.updated as i32,
        duplicate: result.duplicate as i32,
        skipped: result.skipped as i32,
        filtered: result.filtered as i32,
        batches: result.batches as i32,
        has_more: result.has_more,
        last_cursor: result.last_cursor.map(to_sync_cursor_dto),
        checkpoint: result.checkpoint.map(to_sync_checkpoint_dto),
    }
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

    let validation = runtime.validator.validate(&request.node);
    if !validation.is_valid {
        return Ok(StoreContextResponse {
            node_id: String::new(),
            psi: 0.0,
            valid: false,
            validation_error: Some(format!(
                "{}: {}",
                validation.reason,
                validation.error.unwrap_or_default()
            )),
            duplicate_skipped: false,
            upsert_status: None,
        });
    }

    let parser = SttpNodeParser::new();
    let parse_result = parser.try_parse(&request.node, effective_session_id);
    if !parse_result.success {
        return Ok(StoreContextResponse {
            node_id: String::new(),
            psi: 0.0,
            valid: false,
            validation_error: Some(format!(
                "ParseFailure: {}",
                parse_result.error.unwrap_or_default()
            )),
            duplicate_skipped: false,
            upsert_status: None,
        });
    }

    let Some(parsed) = parse_result.node else {
        return Ok(StoreContextResponse {
            node_id: String::new(),
            psi: 0.0,
            valid: false,
            validation_error: Some("ParseFailure: missing parsed node".to_string()),
            duplicate_skipped: false,
            upsert_status: None,
        });
    };

    let psi = parsed.psi;
    let upsert = runtime
        .store
        .upsert_node_async(parsed)
        .await
        .map_err(|err| map_err("local node upsert failed", err))?;

    Ok(StoreContextResponse {
        node_id: upsert.node_id,
        psi,
        valid: true,
        validation_error: None,
        duplicate_skipped: is_duplicate_upsert_status(upsert.status),
        upsert_status: Some(node_upsert_status_label(upsert.status)),
    })
}

#[tauri::command]
async fn sync_pull(
    state: tauri::State<'_, AppState>,
    request: SyncPullCommandRequest,
) -> Result<SyncPullCommandResponse, String> {
    let runtime = sttp_runtime_handle(&state)?;
    execute_sync_pull(&state, runtime, request).await
}

#[tauri::command]
async fn sync_now(
    state: tauri::State<'_, AppState>,
    request: SyncNowRequest,
) -> Result<SyncNowResponse, String> {
    let runtime = sttp_runtime_handle(&state)?;
    let session_filter = request
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let session_scope = session_filter.unwrap_or("all").to_string();

    let gateway_base_url = effective_gateway_base_url(&state, request.gateway_base_url.as_deref())?;
    if gateway_base_url.trim().is_empty() {
        return Err(
            "cloud sync path not set. open settings -> advanced sync once, then sync is one-click."
                .to_string(),
        );
    }

    let page_size = request.page_size.unwrap_or(200).clamp(1, 500) as usize;
    let max_batches = request
        .max_batches
        .map(|value| value.max(1) as usize)
        .unwrap_or(usize::MAX);

    let upload_summary = push_local_changes_to_gateway(
        &runtime,
        &state.http,
        &gateway_base_url,
        session_filter,
        page_size,
        max_batches,
    )
    .await?;

    let download_summary = pull_gateway_changes_to_local(
        &runtime,
        &state.http,
        &gateway_base_url,
        session_filter,
        page_size,
        max_batches,
    )
    .await?;

    Ok(SyncNowResponse {
        session_id: session_scope,
        remote_base_url: gateway_base_url,
        upload: to_sync_upload_stats(upload_summary),
        download: to_sync_download_stats(download_summary),
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
            sync_pull,
            sync_now,
            calibrate_session,
            summarize_node,
            unwind_node,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use resonantia_core as core;
use tauri::Manager;

#[tauri::command]
fn get_config(state: tauri::State<'_, core::AppState>) -> Result<core::AppConfig, String> {
    core::get_config(state.inner())
}

#[tauri::command]
fn get_layout_overrides(
    state: tauri::State<'_, core::AppState>,
) -> Result<core::LayoutOverrides, String> {
    core::get_layout_overrides(state.inner())
}

#[tauri::command]
fn save_layout_overrides(
    state: tauri::State<'_, core::AppState>,
    layout_overrides: core::LayoutOverrides,
) -> Result<(), String> {
    core::save_layout_overrides(state.inner(), layout_overrides)
}

#[tauri::command]
fn reset_layout_overrides(state: tauri::State<'_, core::AppState>) -> Result<(), String> {
    core::reset_layout_overrides(state.inner())
}

#[tauri::command]
fn set_gateway_base_url(
    state: tauri::State<'_, core::AppState>,
    base_url: String,
) -> Result<(), String> {
    core::set_gateway_base_url(state.inner(), base_url)
}

#[tauri::command]
fn set_ollama_config(
    state: tauri::State<'_, core::AppState>,
    base_url: Option<String>,
    model: Option<String>,
) -> Result<(), String> {
    core::set_ollama_config(state.inner(), base_url, model)
}

#[tauri::command]
async fn get_health(
    state: tauri::State<'_, core::AppState>,
) -> Result<core::HealthResponse, String> {
    core::get_health(state.inner()).await
}

#[tauri::command]
async fn list_nodes(
    state: tauri::State<'_, core::AppState>,
    limit: i32,
    session_id: Option<String>,
) -> Result<core::ListNodesResponse, String> {
    core::list_nodes(state.inner(), limit, session_id).await
}

#[tauri::command]
async fn get_graph(
    state: tauri::State<'_, core::AppState>,
    limit: i32,
    session_id: Option<String>,
) -> Result<core::GraphResponse, String> {
    core::get_graph(state.inner(), limit, session_id).await
}

#[tauri::command]
async fn store_context(
    state: tauri::State<'_, core::AppState>,
    request: core::StoreContextRequest,
) -> Result<core::StoreContextResponse, String> {
    core::store_context(state.inner(), request).await
}

#[tauri::command]
async fn sync_pull(
    state: tauri::State<'_, core::AppState>,
    request: core::SyncPullCommandRequest,
) -> Result<core::SyncPullCommandResponse, String> {
    core::sync_pull(state.inner(), request).await
}

#[tauri::command]
async fn sync_now(
    state: tauri::State<'_, core::AppState>,
    request: core::SyncNowRequest,
) -> Result<core::SyncNowResponse, String> {
    core::sync_now(state.inner(), request).await
}

#[tauri::command]
async fn calibrate_session(
    state: tauri::State<'_, core::AppState>,
    request: core::CalibrateSessionRequest,
) -> Result<core::CalibrateSessionResponse, String> {
    core::calibrate_session(state.inner(), request).await
}

#[tauri::command]
async fn summarize_node(
    state: tauri::State<'_, core::AppState>,
    raw_node: String,
) -> Result<Option<core::AiSummary>, String> {
    core::summarize_node(state.inner(), raw_node).await
}

#[tauri::command]
async fn chat_compose(
    state: tauri::State<'_, core::AppState>,
    request: core::ComposeChatRequest,
) -> Result<Option<String>, String> {
    core::chat_compose(state.inner(), request).await
}

#[tauri::command]
fn get_compose_encode_preamble() -> String {
    core::get_compose_encode_preamble()
}

#[tauri::command]
async fn encode_compose(
    state: tauri::State<'_, core::AppState>,
    request: core::EncodeComposeRequest,
) -> Result<String, String> {
    core::encode_compose(state.inner(), request).await
}

#[tauri::command]
fn unwind_node(node: core::NodeDto) -> core::UnwindResult {
    core::unwind_node(node)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(core::create_app_state())
        .setup(|app| {
            let state = app.state::<core::AppState>();
            let config_dir = app
                .path()
                .app_config_dir()
                .map_err(|err| format!("failed to resolve app config dir: {err}"))?;

            core::initialize_app_state(state.inner(), &config_dir)?;
            eprintln!("sttp runtime ready: {}", core::transport_label(state.inner()));
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
            chat_compose,
            get_compose_encode_preamble,
            encode_compose,
            unwind_node,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

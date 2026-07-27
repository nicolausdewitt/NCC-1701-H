use std::{fs, sync::Arc};

use ncc_core::{CrewId, CrewManifest, ModelAssignment, ProjectConnection, example_manifest};
use ncc_warpcore::{SaveIntent, ServerSaveCommand, WarpCore, WarpCoreStatus};
use tauri::{Manager, State};

const SYSTEM_COLLECTION: &str = "system";
const CREW_MANIFEST_ID: &str = "crew-manifest";
const PROJECT_CONNECTION_ID: &str = "active-project";

struct BridgeState {
    warp_core: Arc<WarpCore>,
}

/// Read requests also pass through the Warp Core. The webview never opens or
/// owns a database and cannot construct an alternative source of truth.
#[tauri::command]
async fn get_crew_manifest(state: State<'_, BridgeState>) -> Result<CrewManifest, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || load_crew_manifest(&warp_core))
        .await
        .map_err(|error| error.to_string())?
}

/// UI intent enters here, but the mutation occurs only in WarpCore::save.
/// That call commits the new crew projection and its server-save command in one
/// local transaction before this function reports success.
#[tauri::command]
async fn assign_leader_model(
    state: State<'_, BridgeState>,
    leader_id: String,
    model: ModelAssignment,
) -> Result<CrewManifest, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let stored = warp_core
            .document(SYSTEM_COLLECTION, CREW_MANIFEST_ID)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "crew manifest is not commissioned".to_string())?;
        let mut manifest: CrewManifest = stored
            .document
            .ok_or_else(|| "crew manifest has no document".to_string())
            .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))?;

        manifest
            .assign_model(&CrewId::new(leader_id), model)
            .map_err(|error| error.to_string())?;

        let mut intent = SaveIntent::upsert(
            SYSTEM_COLLECTION,
            CREW_MANIFEST_ID,
            serde_json::to_value(&manifest).map_err(|error| error.to_string())?,
        );
        intent.expected_local_version = Some(stored.local_version);
        warp_core.save(intent).map_err(|error| error.to_string())?;
        Ok(manifest)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn assign_command_model(
    state: State<'_, BridgeState>,
    model: ModelAssignment,
) -> Result<CrewManifest, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let stored = warp_core
            .document(SYSTEM_COLLECTION, CREW_MANIFEST_ID)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "crew manifest is not commissioned".to_string())?;
        let mut manifest: CrewManifest = stored
            .document
            .ok_or_else(|| "crew manifest has no document".to_string())
            .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))?;

        manifest
            .assign_command_model(model)
            .map_err(|error| error.to_string())?;

        let mut intent = SaveIntent::upsert(
            SYSTEM_COLLECTION,
            CREW_MANIFEST_ID,
            serde_json::to_value(&manifest).map_err(|error| error.to_string())?,
        );
        intent.expected_local_version = Some(stored.local_version);
        warp_core.save(intent).map_err(|error| error.to_string())?;
        Ok(manifest)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn get_warp_core_status(state: State<'_, BridgeState>) -> Result<WarpCoreStatus, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        warp_core.status().map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn get_project_connection(
    state: State<'_, BridgeState>,
) -> Result<Option<ProjectConnection>, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        warp_core
            .document(SYSTEM_COLLECTION, PROJECT_CONNECTION_ID)
            .map_err(|error| error.to_string())?
            .map(|stored| {
                stored
                    .document
                    .ok_or_else(|| "project connection has been deleted".to_string())
                    .and_then(|value| {
                        serde_json::from_value(value).map_err(|error| error.to_string())
                    })
            })
            .transpose()
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Commissions an arbitrary project through a named adapter. The connection is
/// deliberately metadata-only: provider tokens and Git credentials never enter
/// the Warp Core document or the webview.
#[tauri::command]
async fn connect_project(
    state: State<'_, BridgeState>,
    mut connection: ProjectConnection,
) -> Result<ProjectConnection, String> {
    connection.adapter = connection.adapter.trim().to_string();
    connection.display_name = connection.display_name.trim().to_string();
    connection.repository = connection.repository.trim().to_string();
    connection.workspace_path = connection
        .workspace_path
        .map(|path| path.trim().to_string());
    connection.default_branch = connection.default_branch.trim().to_string();
    connection.validate().map_err(|error| error.to_string())?;

    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let current = warp_core
            .document(SYSTEM_COLLECTION, PROJECT_CONNECTION_ID)
            .map_err(|error| error.to_string())?;
        let mut intent = SaveIntent::upsert(
            SYSTEM_COLLECTION,
            PROJECT_CONNECTION_ID,
            serde_json::to_value(&connection).map_err(|error| error.to_string())?,
        );
        intent.expected_local_version = current.map(|stored| stored.local_version);
        warp_core.save(intent).map_err(|error| error.to_string())?;
        Ok(connection)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn submit_captain_message(
    state: State<'_, BridgeState>,
    message_id: String,
    text: String,
) -> Result<ServerSaveCommand, String> {
    let message_id = message_id.trim().to_string();
    let text = text.trim().to_string();
    if message_id.is_empty() {
        return Err("message id must not be empty".into());
    }
    if text.is_empty() {
        return Err("message must not be empty".into());
    }
    if text.chars().count() > 16_000 {
        return Err("message exceeds the 16,000 character limit".into());
    }

    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        warp_core
            .save(SaveIntent::upsert(
                "captain_messages",
                message_id,
                serde_json::json!({
                    "author": "captain",
                    "text": text,
                    "status": "submitted"
                }),
            ))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn submit_staffing_brief(
    state: State<'_, BridgeState>,
    brief_id: String,
    prompt: String,
) -> Result<ServerSaveCommand, String> {
    let brief_id = brief_id.trim().to_string();
    let prompt = prompt.trim().to_string();
    if brief_id.is_empty() || prompt.is_empty() {
        return Err("staffing brief id and prompt must not be empty".into());
    }
    if prompt.chars().count() > 16_000 {
        return Err("staffing brief exceeds the 16,000 character limit".into());
    }

    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        warp_core
            .save(SaveIntent::upsert(
                "staffing_briefs",
                brief_id,
                serde_json::json!({
                    "intent": "recommend_crew_model_assignments",
                    "prompt": prompt,
                    "status": "queued_for_command_model"
                }),
            ))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn load_crew_manifest(warp_core: &WarpCore) -> Result<CrewManifest, String> {
    let stored = warp_core
        .document(SYSTEM_COLLECTION, CREW_MANIFEST_ID)
        .map_err(|error| error.to_string())?;

    if let Some(stored) = stored {
        return stored
            .document
            .ok_or_else(|| "crew manifest has been deleted".to_string())
            .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()));
    }

    let manifest = example_manifest();
    warp_core
        .save(SaveIntent::upsert(
            SYSTEM_COLLECTION,
            CREW_MANIFEST_ID,
            serde_json::to_value(&manifest).map_err(|error| error.to_string())?,
        ))
        .map_err(|error| error.to_string())?;
    Ok(manifest)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_directory = app.path().app_local_data_dir()?;
            fs::create_dir_all(&data_directory)?;
            let warp_core = WarpCore::open_or_create(data_directory.join("warp-core.db"))
                .map_err(|error| std::io::Error::other(error.to_string()))?;

            // Commissioning the crew itself is a Warp Core save, ensuring the
            // first local state has the same replication guarantees as later UI.
            load_crew_manifest(&warp_core).map_err(std::io::Error::other)?;
            app.manage(BridgeState {
                warp_core: Arc::new(warp_core),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_crew_manifest,
            assign_leader_model,
            assign_command_model,
            get_warp_core_status,
            get_project_connection,
            connect_project,
            submit_captain_message,
            submit_staffing_brief
        ])
        .run(tauri::generate_context!())
        .expect("failed to run NCC-1701-H");
}

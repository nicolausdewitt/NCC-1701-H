use std::{fs, sync::Arc};

use ncc_core::{CrewId, CrewManifest, ModelAssignment, example_manifest};
use ncc_warpcore::{SaveIntent, WarpCore, WarpCoreStatus};
use tauri::{Manager, State};

const SYSTEM_COLLECTION: &str = "system";
const CREW_MANIFEST_ID: &str = "crew-manifest";

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
async fn get_warp_core_status(state: State<'_, BridgeState>) -> Result<WarpCoreStatus, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        warp_core.status().map_err(|error| error.to_string())
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
            get_warp_core_status
        ])
        .run(tauri::generate_context!())
        .expect("failed to run NCC-1701-H");
}

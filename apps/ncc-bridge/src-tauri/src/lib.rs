use std::{fs, process::Command, sync::Arc};

use ncc_core::{
    CrewId, CrewManifest, ModelAssignment, ProjectAccess, ProjectConnection, example_manifest,
};
use ncc_warpcore::{SaveIntent, ServerSaveCommand, WarpCore, WarpCoreStatus};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

const SYSTEM_COLLECTION: &str = "system";
const CREW_MANIFEST_ID: &str = "crew-manifest";
const PROJECT_CONNECTION_ID: &str = "active-project";
const PROVIDER_COLLECTION: &str = "provider_connections";
const GITHUB_CONNECTION_ID: &str = "github";
const OPENAI_CONNECTION_ID: &str = "openai";

struct BridgeState {
    warp_core: Arc<WarpCore>,
}

#[derive(Serialize)]
struct GithubWriteAuthorization {
    connection: ProjectConnection,
    account: String,
    permission: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct GithubConnection {
    provider: String,
    adapter: String,
    account: String,
    credential_profile: String,
    status: String,
}

#[derive(Clone, Debug, Serialize)]
struct GithubRepository {
    name_with_owner: String,
    url: String,
    default_branch: Option<String>,
    is_private: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GithubRepositoryRecord {
    name_with_owner: String,
    url: String,
    default_branch_ref: Option<GithubDefaultBranch>,
    is_private: bool,
}

#[derive(Deserialize)]
struct GithubDefaultBranch {
    name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OpenAiConnection {
    provider: String,
    adapter: String,
    auth_method: String,
    credential_profile: String,
    status: String,
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

#[tauri::command]
async fn get_github_connection(
    state: State<'_, BridgeState>,
) -> Result<Option<GithubConnection>, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        warp_core
            .document(PROVIDER_COLLECTION, GITHUB_CONNECTION_ID)
            .map_err(|error| error.to_string())?
            .map(|stored| {
                stored
                    .document
                    .ok_or_else(|| "GitHub connection has been deleted".to_string())
                    .and_then(|value| {
                        serde_json::from_value(value).map_err(|error| error.to_string())
                    })
            })
            .transpose()
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Uses GitHub CLI's browser authorization boundary. GitHub CLI owns the
/// access token; NCC persists only non-secret account and adapter metadata.
#[tauri::command]
async fn authorize_github_account(
    state: State<'_, BridgeState>,
) -> Result<GithubConnection, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || connect_github_account(&warp_core))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn list_github_repositories(
    state: State<'_, BridgeState>,
) -> Result<Vec<GithubRepository>, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Refresh the native CLI session and its non-secret Warp Core metadata
        // before asking GitHub for repositories visible to that account.
        connect_github_account(&warp_core)?;
        let repositories = gh_text(&[
            "repo",
            "list",
            "--limit",
            "100",
            "--json",
            "nameWithOwner,url,defaultBranchRef,isPrivate",
        ])?;
        parse_github_repositories(&repositories)
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
    // Connecting a repository never grants mutation authority. Only the
    // dedicated native GitHub authorization command may upgrade this record.
    connection.access = ProjectAccess::ReadOnly;
    connection.credential_profile = None;
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

/// Uses GitHub CLI's browser authorization and API client. The CLI owns its
/// token; NCC receives only the account and repository permission, then stores
/// an opaque credential-profile reference in Warp Core.
#[tauri::command]
async fn authorize_github_writes(
    state: State<'_, BridgeState>,
) -> Result<GithubWriteAuthorization, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let stored = warp_core
            .document(SYSTEM_COLLECTION, PROJECT_CONNECTION_ID)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "connect a project before authorizing GitHub".to_string())?;
        let mut connection: ProjectConnection = stored
            .document
            .ok_or_else(|| "project connection has been deleted".to_string())
            .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))?;

        if connection.adapter != "github" {
            return Err("write authorization is available only for the GitHub adapter".into());
        }

        let repository = github_repository_slug(&connection.repository)?;
        let github = connect_github_account(&warp_core)?;
        let account = github.account;
        let permission = gh_text(&[
            "repo",
            "view",
            &repository,
            "--json",
            "viewerPermission",
            "--jq",
            ".viewerPermission",
        ])?;

        if !matches!(permission.as_str(), "WRITE" | "MAINTAIN" | "ADMIN") {
            return Err(format!(
                "GitHub reports {permission} access for {account}; write access was not granted"
            ));
        }

        connection.access = ProjectAccess::ReadWrite;
        connection.credential_profile = Some(github.credential_profile);
        connection.validate().map_err(|error| error.to_string())?;

        let mut intent = SaveIntent::upsert(
            SYSTEM_COLLECTION,
            PROJECT_CONNECTION_ID,
            serde_json::to_value(&connection).map_err(|error| error.to_string())?,
        );
        intent.expected_local_version = Some(stored.local_version);
        warp_core.save(intent).map_err(|error| error.to_string())?;

        Ok(GithubWriteAuthorization {
            connection,
            account,
            permission,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn connect_github_account(warp_core: &WarpCore) -> Result<GithubConnection, String> {
    ensure_github_cli_authorized()?;
    let account = gh_text(&["api", "user", "--jq", ".login"])?;
    let connection = GithubConnection {
        provider: "github".into(),
        adapter: "gh-cli".into(),
        credential_profile: format!("gh-cli:github.com/{account}"),
        account,
        status: "connected".into(),
    };

    let current = warp_core
        .document(PROVIDER_COLLECTION, GITHUB_CONNECTION_ID)
        .map_err(|error| error.to_string())?;
    let mut intent = SaveIntent::upsert(
        PROVIDER_COLLECTION,
        GITHUB_CONNECTION_ID,
        serde_json::to_value(&connection).map_err(|error| error.to_string())?,
    );
    intent.expected_local_version = current.map(|stored| stored.local_version);
    warp_core.save(intent).map_err(|error| error.to_string())?;
    Ok(connection)
}

fn ensure_github_cli_authorized() -> Result<(), String> {
    let status = Command::new("gh")
        .args(["auth", "status", "--hostname", "github.com"])
        .output()
        .map_err(|error| {
            format!(
                "GitHub CLI is required for native authorization but could not be started: {error}"
            )
        })?;

    if status.status.success() {
        return Ok(());
    }

    let login = Command::new("gh")
        .args([
            "auth",
            "login",
            "--hostname",
            "github.com",
            "--git-protocol",
            "https",
            "--web",
            "--clipboard",
        ])
        .output()
        .map_err(|error| format!("could not start GitHub browser authorization: {error}"))?;

    if login.status.success() {
        Ok(())
    } else {
        Err("GitHub browser authorization was not completed".into())
    }
}

fn gh_text(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("gh")
        .args(arguments)
        .output()
        .map_err(|error| format!("could not call the GitHub API: {error}"))?;
    if !output.status.success() {
        return Err("GitHub API request failed; check the selected account and repository".into());
    }

    let value = String::from_utf8(output.stdout)
        .map_err(|_| "GitHub API returned non-UTF-8 output".to_string())?
        .trim()
        .to_string();
    if value.is_empty() {
        return Err("GitHub API returned an empty response".into());
    }
    Ok(value)
}

fn github_repository_slug(repository: &str) -> Result<String, String> {
    let repository = repository
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(".git");
    let path = repository
        .strip_prefix("https://github.com/")
        .or_else(|| repository.strip_prefix("http://github.com/"))
        .or_else(|| repository.strip_prefix("git@github.com:"))
        .or_else(|| repository.strip_prefix("github.com/"))
        .ok_or_else(|| "GitHub repository must use a github.com URL".to_string())?;
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() != 2 || parts.iter().any(|part| part.trim().is_empty()) {
        return Err("GitHub repository must identify exactly one owner and repository".into());
    }
    if parts.iter().flat_map(|part| part.chars()).any(|character| {
        !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    }) {
        return Err("GitHub repository contains unsupported characters".into());
    }
    Ok(format!("{}/{}", parts[0], parts[1]))
}

fn parse_github_repositories(value: &str) -> Result<Vec<GithubRepository>, String> {
    let records: Vec<GithubRepositoryRecord> = serde_json::from_str(value)
        .map_err(|error| format!("GitHub returned an invalid repository list: {error}"))?;
    Ok(records
        .into_iter()
        .map(|record| GithubRepository {
            name_with_owner: record.name_with_owner,
            url: record.url,
            default_branch: record.default_branch_ref.map(|branch| branch.name),
            is_private: record.is_private,
        })
        .collect())
}

#[tauri::command]
async fn get_openai_connection(
    state: State<'_, BridgeState>,
) -> Result<Option<OpenAiConnection>, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        warp_core
            .document(PROVIDER_COLLECTION, OPENAI_CONNECTION_ID)
            .map_err(|error| error.to_string())?
            .map(|stored| {
                stored
                    .document
                    .ok_or_else(|| "OpenAI connection has been deleted".to_string())
                    .and_then(|value| {
                        serde_json::from_value(value).map_err(|error| error.to_string())
                    })
            })
            .transpose()
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Reuses Codex's supported OpenAI authentication boundary. The browser flow
/// and token cache belong to Codex; NCC stores only a non-secret profile
/// reference and never receives a ChatGPT password or access token.
#[tauri::command]
async fn authorize_openai(state: State<'_, BridgeState>) -> Result<OpenAiConnection, String> {
    let warp_core = state.warp_core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let auth_method = ensure_codex_openai_authorized()?;
        let connection = OpenAiConnection {
            provider: "openai".into(),
            adapter: "codex-cli".into(),
            auth_method,
            credential_profile: "codex-cli:shared-login".into(),
            status: "connected".into(),
        };

        let current = warp_core
            .document(PROVIDER_COLLECTION, OPENAI_CONNECTION_ID)
            .map_err(|error| error.to_string())?;
        let mut intent = SaveIntent::upsert(
            PROVIDER_COLLECTION,
            OPENAI_CONNECTION_ID,
            serde_json::to_value(&connection).map_err(|error| error.to_string())?,
        );
        intent.expected_local_version = current.map(|stored| stored.local_version);
        warp_core.save(intent).map_err(|error| error.to_string())?;
        Ok(connection)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn ensure_codex_openai_authorized() -> Result<String, String> {
    if let Some(method) = codex_login_status()? {
        return Ok(method);
    }

    let login = Command::new("codex")
        .args(["--config", "model_reasoning_effort=xhigh", "login"])
        .output()
        .map_err(|error| format!("could not start OpenAI browser sign-in: {error}"))?;
    if !login.status.success() {
        return Err("OpenAI browser sign-in was not completed".into());
    }

    codex_login_status()?
        .ok_or_else(|| "Codex did not report an authenticated OpenAI session".to_string())
}

fn codex_login_status() -> Result<Option<String>, String> {
    let output = Command::new("codex")
        .args([
            "--config",
            "model_reasoning_effort=xhigh",
            "login",
            "status",
        ])
        .output()
        .map_err(|error| format!("Codex CLI is required for OpenAI sign-in: {error}"))?;
    if !output.status.success() {
        return Ok(None);
    }

    let status = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(parse_codex_auth_method(&status).map(str::to_string))
}

fn parse_codex_auth_method(status: &str) -> Option<&'static str> {
    let normalized = status.to_ascii_lowercase();
    if normalized.contains("not logged") || normalized.contains("signed out") {
        None
    } else if normalized.contains("chatgpt") {
        Some("chatgpt")
    } else if normalized.contains("api key") || normalized.contains("api-key") {
        Some("api_key")
    } else if normalized.contains("logged in") {
        Some("codex")
    } else {
        None
    }
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
            get_github_connection,
            connect_project,
            authorize_github_account,
            list_github_repositories,
            authorize_github_writes,
            get_openai_connection,
            authorize_openai,
            submit_captain_message,
            submit_staffing_brief
        ])
        .run(tauri::generate_context!())
        .expect("failed to run NCC-1701-H");
}

#[cfg(test)]
mod tests {
    use super::{github_repository_slug, parse_codex_auth_method, parse_github_repositories};

    #[test]
    fn parses_https_and_ssh_github_repositories() {
        assert_eq!(
            github_repository_slug("https://github.com/example/project.git").unwrap(),
            "example/project"
        );
        assert_eq!(
            github_repository_slug("git@github.com:example/project").unwrap(),
            "example/project"
        );
    }

    #[test]
    fn rejects_non_github_and_nested_paths() {
        assert!(github_repository_slug("https://example.com/owner/repo").is_err());
        assert!(github_repository_slug("https://github.com/owner/repo/issues").is_err());
    }

    #[test]
    fn recognizes_codex_openai_authentication_methods() {
        assert_eq!(
            parse_codex_auth_method("Logged in using ChatGPT"),
            Some("chatgpt")
        );
        assert_eq!(
            parse_codex_auth_method("Logged in using an API key"),
            Some("api_key")
        );
        assert_eq!(parse_codex_auth_method("Not logged in"), None);
    }

    #[test]
    fn parses_github_repository_list_without_credentials() {
        let repositories = parse_github_repositories(
            r#"[
                {
                    "nameWithOwner": "example/private-project",
                    "url": "https://github.com/example/private-project",
                    "defaultBranchRef": {"name": "trunk"},
                    "isPrivate": true
                },
                {
                    "nameWithOwner": "example/empty-project",
                    "url": "https://github.com/example/empty-project",
                    "defaultBranchRef": null,
                    "isPrivate": false
                }
            ]"#,
        )
        .unwrap();

        assert_eq!(repositories.len(), 2);
        assert_eq!(repositories[0].name_with_owner, "example/private-project");
        assert_eq!(repositories[0].default_branch.as_deref(), Some("trunk"));
        assert!(repositories[0].is_private);
        assert_eq!(repositories[1].default_branch, None);
        assert!(!repositories[1].is_private);
    }
}

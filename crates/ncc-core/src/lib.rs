//! Domain contracts for NCC-1701-H.
//!
//! This crate deliberately contains no UI, networking, provider SDK, or
//! persistence code. It is the stable vocabulary shared by every adapter.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CrewId(pub String);

impl CrewId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelAssignment {
    /// Adapter name, such as `openai`, `anthropic`, or `local`.
    pub provider: String,
    /// Provider-specific model identifier.
    pub model: String,
    /// Optional provider endpoint override for gateways and local runtimes.
    pub endpoint: Option<String>,
}

impl ModelAssignment {
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            endpoint: None,
        }
    }

    pub fn label(&self) -> String {
        format!("{} / {}", self.provider, self.model)
    }
}

/// A project commissioned into the harness.
///
/// This record intentionally contains no access token or project-specific
/// schema. The named adapter resolves credentials and translates the generic
/// project contract at runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConnection {
    /// Adapter identifier, such as `local-git`, `github`, or a private sidecar.
    pub adapter: String,
    /// Human-facing project name shown on the Bridge.
    pub display_name: String,
    /// Adapter-specific repository locator, normally a Git remote URL.
    pub repository: String,
    /// Optional existing checkout used by local tools and engineering agents.
    pub workspace_path: Option<String>,
    pub default_branch: String,
}

impl ProjectConnection {
    pub fn validate(&self) -> Result<(), ProjectConnectionError> {
        for (field, value) in [
            ("adapter", self.adapter.as_str()),
            ("display_name", self.display_name.as_str()),
            ("repository", self.repository.as_str()),
            ("default_branch", self.default_branch.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ProjectConnectionError::EmptyField(field));
            }
        }

        if self
            .workspace_path
            .as_deref()
            .is_some_and(|path| path.trim().is_empty())
        {
            return Err(ProjectConnectionError::EmptyField("workspace_path"));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectConnectionError {
    EmptyField(&'static str),
}

impl std::fmt::Display for ProjectConnectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyField(field) => write!(formatter, "project {field} must not be empty"),
        }
    }
}

impl std::error::Error for ProjectConnectionError {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TeamLeader {
    pub id: CrewId,
    pub display_name: String,
    pub professional_role: String,
    pub department: String,
    pub model: ModelAssignment,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CrewManifest {
    /// The model behind the owner-facing Captain interface. Older manifests
    /// deserialize without it and can be commissioned through the Bridge.
    #[serde(default)]
    pub command_model: Option<ModelAssignment>,
    pub leaders: Vec<TeamLeader>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    DuplicateCrewId(String),
    EmptyField {
        crew_id: String,
        field: &'static str,
    },
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateCrewId(id) => write!(formatter, "duplicate crew id: {id}"),
            Self::EmptyField { crew_id, field } => {
                write!(formatter, "{crew_id} has an empty {field}")
            }
        }
    }
}

impl std::error::Error for ManifestError {}

impl CrewManifest {
    pub fn validate(&self) -> Result<(), ManifestError> {
        let mut ids = HashSet::with_capacity(self.leaders.len());

        for leader in &self.leaders {
            let id = leader.id.0.trim();
            if id.is_empty() {
                return Err(ManifestError::EmptyField {
                    crew_id: "<unknown>".into(),
                    field: "id",
                });
            }
            if !ids.insert(id) {
                return Err(ManifestError::DuplicateCrewId(id.into()));
            }

            for (field, value) in [
                ("display_name", leader.display_name.as_str()),
                ("professional_role", leader.professional_role.as_str()),
                ("department", leader.department.as_str()),
                ("model.provider", leader.model.provider.as_str()),
                ("model.model", leader.model.model.as_str()),
            ] {
                if value.trim().is_empty() {
                    return Err(ManifestError::EmptyField {
                        crew_id: id.into(),
                        field,
                    });
                }
            }
        }

        Ok(())
    }

    pub fn leader(&self, id: &CrewId) -> Option<&TeamLeader> {
        self.leaders.iter().find(|leader| leader.id == *id)
    }

    pub fn assign_model(
        &mut self,
        id: &CrewId,
        model: ModelAssignment,
    ) -> Result<(), AssignModelError> {
        if model.provider.trim().is_empty() || model.model.trim().is_empty() {
            return Err(AssignModelError::InvalidModel);
        }

        let leader = self
            .leaders
            .iter_mut()
            .find(|leader| leader.id == *id)
            .ok_or_else(|| AssignModelError::UnknownLeader(id.0.clone()))?;

        leader.model = model;
        Ok(())
    }

    pub fn assign_command_model(&mut self, model: ModelAssignment) -> Result<(), AssignModelError> {
        if model.provider.trim().is_empty() || model.model.trim().is_empty() {
            return Err(AssignModelError::InvalidModel);
        }

        self.command_model = Some(model);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssignModelError {
    UnknownLeader(String),
    InvalidModel,
}

impl std::fmt::Display for AssignModelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownLeader(id) => write!(formatter, "unknown team leader: {id}"),
            Self::InvalidModel => write!(formatter, "provider and model must not be empty"),
        }
    }
}

impl std::error::Error for AssignModelError {}

/// A demonstrative crew. Model identifiers are placeholders, not defaults tied
/// to a particular commercial provider.
pub fn example_manifest() -> CrewManifest {
    let leader = |id: &str,
                  display_name: &str,
                  professional_role: &str,
                  department: &str,
                  provider: &str,
                  model: &str| TeamLeader {
        id: CrewId::new(id),
        display_name: display_name.into(),
        professional_role: professional_role.into(),
        department: department.into(),
        model: ModelAssignment::new(provider, model),
    };

    CrewManifest {
        command_model: Some(ModelAssignment::new("provider-a", "command-model")),
        leaders: vec![
            leader(
                "riker",
                "William Riker",
                "Agent Operations Director",
                "Command",
                "provider-a",
                "orchestration-model",
            ),
            leader(
                "data",
                "Data",
                "Principal Analyst",
                "Research & Analysis",
                "provider-b",
                "reasoning-model",
            ),
            leader(
                "la-forge",
                "Geordi La Forge",
                "Principal Software Engineer",
                "Engineering",
                "provider-c",
                "coding-model",
            ),
            leader(
                "worf",
                "Worf",
                "Security & Risk Director",
                "Security",
                "provider-b",
                "security-model",
            ),
            leader(
                "troi",
                "Deanna Troi",
                "Organisational Psychologist",
                "People & Users",
                "provider-a",
                "human-context-model",
            ),
            leader(
                "crusher",
                "Beverly Crusher",
                "Quality & Safety Director",
                "Quality",
                "provider-b",
                "diagnostic-model",
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaders_can_use_different_models() {
        let manifest = example_manifest();
        let models: HashSet<_> = manifest
            .leaders
            .iter()
            .map(|leader| leader.model.label())
            .collect();

        assert!(models.len() > 1);
        assert_eq!(manifest.validate(), Ok(()));
    }

    #[test]
    fn model_assignment_changes_only_the_selected_leader() {
        let mut manifest = example_manifest();
        let data_before = manifest.leader(&CrewId::new("data")).unwrap().model.clone();

        manifest
            .assign_model(
                &CrewId::new("la-forge"),
                ModelAssignment::new("local", "engineering-specialist"),
            )
            .unwrap();

        assert_eq!(
            manifest.leader(&CrewId::new("data")).unwrap().model,
            data_before
        );
        assert_eq!(
            manifest
                .leader(&CrewId::new("la-forge"))
                .unwrap()
                .model
                .label(),
            "local / engineering-specialist"
        );
    }

    #[test]
    fn command_model_is_independent_from_department_models() {
        let mut manifest = example_manifest();
        let engineering_before = manifest
            .leader(&CrewId::new("la-forge"))
            .unwrap()
            .model
            .clone();

        manifest
            .assign_command_model(ModelAssignment::new("local", "captain-model"))
            .unwrap();

        assert_eq!(
            manifest.command_model.as_ref().unwrap().label(),
            "local / captain-model"
        );
        assert_eq!(
            manifest.leader(&CrewId::new("la-forge")).unwrap().model,
            engineering_before
        );
    }

    #[test]
    fn project_connections_are_adapter_owned_and_secret_free() {
        let connection = ProjectConnection {
            adapter: "github".into(),
            display_name: "Example Private Project".into(),
            repository: "https://github.com/example/private-project".into(),
            workspace_path: Some(r"C:\src\private-project".into()),
            default_branch: "main".into(),
        };

        assert_eq!(connection.validate(), Ok(()));
        let json = serde_json::to_string(&connection).unwrap();
        assert!(!json.contains("token"));
        assert!(!json.contains("credential"));
    }
}

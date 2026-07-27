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
}

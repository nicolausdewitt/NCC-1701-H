//! Asynchronous command plane for the Warp Core.

use ncc_core::{AssignModelError, CrewId, CrewManifest, ModelAssignment};
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub enum Command {
    AssignLeaderModel {
        leader_id: CrewId,
        model: ModelAssignment,
    },
    RequestSnapshot,
    ShutDown,
}

#[derive(Clone, Debug)]
pub enum BridgeEvent {
    Online(CrewManifest),
    CrewUpdated(CrewManifest),
    CommandRejected(String),
    Offline,
}

#[derive(Clone)]
pub struct CommandDeck {
    sender: mpsc::Sender<Command>,
}

#[derive(Debug)]
pub enum DispatchError {
    Busy,
    Offline,
}

impl std::fmt::Display for DispatchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => write!(formatter, "the command queue is at capacity"),
            Self::Offline => write!(formatter, "the Warp Core is offline"),
        }
    }
}

impl std::error::Error for DispatchError {}

impl CommandDeck {
    /// Non-blocking dispatch for use by the UI thread.
    pub fn try_dispatch(&self, command: Command) -> Result<(), DispatchError> {
        self.sender.try_send(command).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => DispatchError::Busy,
            mpsc::error::TrySendError::Closed(_) => DispatchError::Offline,
        })
    }
}

pub fn launch(
    manifest: CrewManifest,
    command_capacity: usize,
    event_capacity: usize,
) -> (CommandDeck, mpsc::Receiver<BridgeEvent>) {
    assert!(command_capacity > 0, "command capacity must be positive");
    assert!(event_capacity > 0, "event capacity must be positive");

    let (command_sender, command_receiver) = mpsc::channel(command_capacity);
    let (event_sender, event_receiver) = mpsc::channel(event_capacity);

    tokio::spawn(run_warp_core(manifest, command_receiver, event_sender));

    (
        CommandDeck {
            sender: command_sender,
        },
        event_receiver,
    )
}

async fn run_warp_core(
    mut manifest: CrewManifest,
    mut commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<BridgeEvent>,
) {
    if let Err(error) = manifest.validate() {
        let _ = events
            .send(BridgeEvent::CommandRejected(error.to_string()))
            .await;
        let _ = events.send(BridgeEvent::Offline).await;
        return;
    }

    if events
        .send(BridgeEvent::Online(manifest.clone()))
        .await
        .is_err()
    {
        return;
    }

    while let Some(command) = commands.recv().await {
        let event = match command {
            Command::AssignLeaderModel { leader_id, model } => {
                match manifest.assign_model(&leader_id, model) {
                    Ok(()) => BridgeEvent::CrewUpdated(manifest.clone()),
                    Err(error) => BridgeEvent::CommandRejected(format_assignment_error(error)),
                }
            }
            Command::RequestSnapshot => BridgeEvent::CrewUpdated(manifest.clone()),
            Command::ShutDown => {
                let _ = events.send(BridgeEvent::Offline).await;
                return;
            }
        };

        if events.send(event).await.is_err() {
            return;
        }
    }
}

fn format_assignment_error(error: AssignModelError) -> String {
    format!("model assignment rejected: {error}")
}

#[cfg(test)]
mod tests {
    use ncc_core::{CrewId, ModelAssignment, example_manifest};

    use super::*;

    #[tokio::test]
    async fn publishes_independent_leader_model_changes() {
        let (deck, mut events) = launch(example_manifest(), 8, 8);
        assert!(matches!(events.recv().await, Some(BridgeEvent::Online(_))));

        deck.try_dispatch(Command::AssignLeaderModel {
            leader_id: CrewId::new("la-forge"),
            model: ModelAssignment::new("local", "code-specialist"),
        })
        .unwrap();

        let Some(BridgeEvent::CrewUpdated(manifest)) = events.recv().await else {
            panic!("expected a crew update");
        };

        assert_eq!(
            manifest
                .leader(&CrewId::new("la-forge"))
                .unwrap()
                .model
                .label(),
            "local / code-specialist"
        );
        assert_ne!(
            manifest.leader(&CrewId::new("data")).unwrap().model.label(),
            "local / code-specialist"
        );
    }
}

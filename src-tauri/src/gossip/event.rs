use iroh::NodeId;
use iroh_docs::engine::LiveEvent;
use n0_future::{boxed::BoxStream, task::AbortOnDropHandle, StreamExt as _};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter as _};
use tokio::sync::Mutex as TokioMutex;
use tracing::info;

use crate::state::ActiveChannel;

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    #[serde(rename_all = "camelCase")]
    NeighborUp {
        node_id: NodeId,
    },
    #[serde(rename_all = "camelCase")]
    NeighborDown {
        node_id: NodeId,
    },
    #[serde(rename_all = "camelCase")]
    Errorred {
        message: String,
    },
    Disconnected,
}

/// Spawns a background task to listen for chat events and emit them to the frontend.
pub fn spawn_event_listener(
    app: tauri::AppHandle,
    mut events: BoxStream<anyhow::Result<LiveEvent>>,
    active_channel: Arc<TokioMutex<Option<ActiveChannel>>>,
    latest_ticket: Arc<TokioMutex<Option<String>>>,
) -> AbortOnDropHandle<()> {
    AbortOnDropHandle::new(n0_future::task::spawn(async move {
        while let Some(Ok(event)) = events.next().await {
            info!("Received event: {:?}", &event);
            // for any event, check if we've updated the peers list (and emit peer-events)
            match &event {
                LiveEvent::InsertLocal { entry } => todo!(),
                LiveEvent::InsertRemote {
                    from,
                    entry,
                    content_status,
                } => todo!(),
                LiveEvent::ContentReady { hash } => todo!(),
                LiveEvent::PendingContentReady => todo!(),
                LiveEvent::NeighborUp(public_key) => todo!(),
                LiveEvent::NeighborDown(public_key) => todo!(),
                LiveEvent::SyncFinished(sync_event) => todo!(),
            };
            // emit a chat-event for each event
            if let Err(e) = app.emit("chat-event", &event) {
                tracing::error!("Failed to emit event to frontend: {}", e);
            }
            // If a peer joins or a new neighbor comes up, update the latest_ticket
            update_ticket(&event, &app, &latest_ticket, &active_channel).await;
        }
    }))
}

/// If a peer joins or a new neighbor comes up, update the latest_ticket
/// with new peer nodes to assist reconnections.
async fn update_ticket(
    event: &LiveEvent,
    app: &AppHandle,
    latest_ticket: &Arc<TokioMutex<Option<String>>>,
    active_channel: &Arc<TokioMutex<Option<ActiveChannel>>>,
) {
    match &event {
        LiveEvent::NeighborUp { .. } => {
            tracing::debug!("Peer event detected, attempting to update latest ticket.");
            if let Some(active_channel_guard) = active_channel.lock().await.as_ref() {
                let new_ticket_str = active_channel_guard.activity.ticket();
                *latest_ticket.lock().await = Some(new_ticket_str.clone());
                tracing::info!("Updated latest_ticket due to new peer joining/neighbor up.");
                if let Err(e) = app.emit("ticket-updated", new_ticket_str) {
                    tracing::warn!("Failed to emit ticket-updated event: {}", e);
                }
            }
        }
        _ => {} // Other events do not trigger ticket update
    }
}

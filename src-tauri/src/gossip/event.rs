use iroh::NodeId;
use iroh_docs::{engine::LiveEvent, Entry};
use n0_future::{boxed::BoxStream, task::AbortOnDropHandle, StreamExt as _};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter as _};
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, error, info};

use crate::{
    gossip::doc::{chat::ChatMessage, peers::PeerInfo, MESSAGES_PREFIX, PEERS_PREFIX},
    state::ActiveChannel,
};

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

/// Helper function to process an entry and emit specific update events.
async fn process_entry_for_updates(entry: &Entry, channel: &ActiveChannel, app: &AppHandle) {
    let key = entry.key();
    if key.starts_with(PEERS_PREFIX) {
        debug!("Processing peers entry");
        match channel.activity.read_bytes(entry.clone()).await {
            Ok(bytes) => match postcard::from_bytes::<PeerInfo>(&bytes) {
                Ok(peer_info) => {
                    info!("Peer info updated/received: {:?}", peer_info);
                    if let Err(e) = app.emit("peer-update", &peer_info) {
                        error!("Failed to emit peer-update event: {e:?}");
                    }
                }
                Err(e) => error!("Failed to deserialize PeerInfo {e:?}"),
            },
            Err(e) => error!("Failed to read bytes for peer entry {e:?}"),
        }
    } else if key.starts_with(MESSAGES_PREFIX) {
        debug!("Processing message entry");
        match channel.activity.read_bytes(entry.clone()).await {
            Ok(bytes) => match postcard::from_bytes::<ChatMessage>(&bytes) {
                Ok(chat_message) => {
                    info!("New/updated chat message: {:?}", chat_message);
                    if let Err(e) = app.emit("new-message", &chat_message) {
                        error!("Failed to emit new-message event: {}", e);
                    }
                }
                Err(e) => error!("Failed to deserialize ChatMessage {e:?}",),
            },
            Err(e) => error!("Failed to read bytes for message entry {e:?}",),
        }
    }
}

/// Spawns a background task to listen for chat events and emit them to the frontend.
pub fn spawn_event_listener(
    app: tauri::AppHandle,
    mut events: BoxStream<anyhow::Result<LiveEvent>>,
    active_channel: Arc<TokioMutex<Option<ActiveChannel>>>,
) -> AbortOnDropHandle<()> {
    AbortOnDropHandle::new(n0_future::task::spawn(async move {
        while let Some(Ok(event)) = events.next().await {
            info!("Received LiveEvent: {:?}", &event);
            let active_channel_guard = active_channel.lock().await;
            let Some(channel) = active_channel_guard.as_ref() else {
                info!("No active channel, should never occur when listening for events.");
                continue;
            };
            match &event {
                LiveEvent::InsertLocal { entry } => {
                    process_entry_for_updates(entry, channel, &app).await;
                }
                LiveEvent::InsertRemote { entry, .. } => {
                    // Content might be missing initially. read_bytes should attempt to fetch.
                    // If it fails, ContentReady might signal availability later,
                    // but for now, we attempt a direct read.
                    process_entry_for_updates(entry, channel, &app).await;
                }
                LiveEvent::SyncFinished(_sync_event) => {
                    info!("Sync finished, frontend should request all data.");
                    if let Err(e) = app.emit("update-all", ()) {
                        error!("Failed to emit update-all event: {}", e);
                    };
                }
                LiveEvent::NeighborUp(node_id) => {
                    info!("Neighbor up: {}", node_id);
                    let payload = Event::NeighborUp { node_id: *node_id };
                    if let Err(e) = app.emit("chat-event", &payload) {
                        error!("Failed to emit neighbor-up event: {}", e);
                    }
                }
                LiveEvent::NeighborDown(node_id) => {
                    info!("Neighbor down: {}", node_id);
                    let payload = Event::NeighborDown { node_id: *node_id };
                    if let Err(e) = app.emit("chat-event", &payload) {
                        error!("Failed to emit neighbor-down event: {}", e);
                    }
                }
                LiveEvent::ContentReady { hash } => {
                    info!(
                        "Content ready for hash: {:?}. Dependent data might now be available.",
                        hash
                    );
                    // For now, just logging. The frontend might need to poll or have a refresh button
                    // if data was initially missed. Alternatively, specific entries that failed
                    // to load previously could be retried if their state was tracked.
                }
                LiveEvent::PendingContentReady => {
                    info!("Pending content ready event received. System is fetching data.");
                }
            }
            if let Err(e) = app.emit("raw-live-event", &event) {
                error!("Failed to emit raw live-event to frontend: {}", e);
            }
        }
    }))
}

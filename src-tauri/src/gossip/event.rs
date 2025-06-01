use bytes::Bytes;
use iroh::NodeId;
use iroh_blobs::Hash;
use iroh_docs::{engine::LiveEvent, ContentStatus, Entry, RecordIdentifier};
use n0_future::{boxed::BoxStream, task::AbortOnDropHandle, StreamExt as _};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tauri::{AppHandle, Emitter as _};
use tokio::{sync::Mutex as TokioMutex, time::sleep};
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
    SyncFinished,
    PeerUpdate {
        info: PeerInfo,
    },
    NewMessage {
        message: ChatMessage,
    },
    ContentReady,
    PendingContentReady,
}

/// Helper function to process an entry and emit specific update events.
async fn process_entry_for_updates(
    key: &[u8],
    hash: Hash,
    channel: &ActiveChannel,
    app: &AppHandle,
) {
    info!("Processing entry: {:?}", key.to_ascii_lowercase());
    if key.starts_with(PEERS_PREFIX) {
        debug!("Processing peers entry");
        match channel.activity.read_bytes(hash).await {
            Ok(bytes) => match postcard::from_bytes::<PeerInfo>(&bytes) {
                Ok(info) => {
                    info!("Peer info updated/received: {:?}", info);
                    if let Err(e) = app.emit("chat-event", Event::PeerUpdate { info }) {
                        error!("Failed to emit peer-update event: {e:?}");
                    }
                }
                Err(e) => error!("Failed to deserialize PeerInfo {e:?}"),
            },
            Err(e) => error!("Failed to read bytes for peer entry {e:?}"),
        }
    } else if key.starts_with(MESSAGES_PREFIX) {
        debug!("Processing message entry");
        match channel.activity.read_bytes(hash).await {
            Ok(bytes) => match postcard::from_bytes::<ChatMessage>(&bytes) {
                Ok(message) => {
                    info!("New/updated chat message: {:?}", message);
                    if let Err(e) = app.emit("chat-event", Event::NewMessage { message }) {
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
        while active_channel.lock().await.is_none() {
            info!("Waiting for active channel...");
            sleep(Duration::from_secs(1)).await;
        }
        let mut pending_entries: HashMap<Hash, Vec<u8>> = HashMap::new();
        while let Some(Ok(event)) = events.next().await {
            info!("Received LiveEvent: {:?}", &event);
            let active_channel_guard = active_channel.lock().await;
            let Some(channel) = active_channel_guard.as_ref() else {
                panic!("No active channel, should never occur.");
            };
            info!("{} pending entries", pending_entries.len());
            match &event {
                LiveEvent::InsertLocal { entry } => {
                    let (key, hash) = (entry.key(), entry.content_hash());
                    process_entry_for_updates(key, hash, channel, &app).await;
                }
                LiveEvent::InsertRemote {
                    entry,
                    content_status,
                    ..
                } => {
                    // Content might be missing initially. read_bytes should attempt to fetch.
                    // If it fails, ContentReady should signal availability later.
                    let (key, hash) = (entry.key(), entry.content_hash());
                    match content_status {
                        ContentStatus::Complete => {
                            process_entry_for_updates(key, hash, channel, &app).await
                        }
                        ContentStatus::Incomplete | ContentStatus::Missing => {
                            pending_entries.insert(hash, key.to_vec());
                        }
                    }
                }
                LiveEvent::SyncFinished(_sync_event) => {
                    info!("Sync finished, frontend should request all data.");
                    app.emit("chat-event", Event::SyncFinished).ok();
                }
                LiveEvent::NeighborUp(node_id) => {
                    info!("Neighbor up: {}", node_id);
                    let payload = Event::NeighborUp { node_id: *node_id };
                    app.emit("chat-event", &payload).ok();
                }
                LiveEvent::NeighborDown(node_id) => {
                    info!("Neighbor down: {}", node_id);
                    let payload = Event::NeighborDown { node_id: *node_id };
                    app.emit("chat-event", &payload).ok();
                }
                LiveEvent::ContentReady { hash } => {
                    info!("Content ready for hash: {:?}.", hash);
                    app.emit("chat-event", Event::ContentReady).ok();
                    if let Some(key) = pending_entries.remove(hash) {
                        process_entry_for_updates(&key, *hash, channel, &app).await;
                    }
                }
                LiveEvent::PendingContentReady => {
                    info!("Pending content ready event received. System is fetching data.");
                    app.emit("chat-event", Event::PendingContentReady).ok();
                }
            }
        }
    }))
}

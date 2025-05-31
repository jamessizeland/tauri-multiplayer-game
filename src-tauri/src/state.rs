use crate::gossip::{
    doc::SharedActivity,
    peers::{PeerInfo, PeerMap},
    types::ChatMessage,
    Event, GossipNode,
};
use anyhow::anyhow;
use iroh::NodeId;
use iroh_docs::{engine::LiveEvent, DocTicket};
use n0_future::{boxed::BoxStream, task::AbortOnDropHandle, StreamExt as _};
use std::{collections::HashSet, sync::Arc};
use tauri::{AppHandle, Emitter as _};
use tokio::{
    select,
    sync::Mutex as TokioMutex,
    time::{interval, Duration},
};

/// Holds information about the currently active game.
pub struct ActiveChannel {
    activity: SharedActivity,
    receiver_handle: AbortOnDropHandle<()>,
}

impl ActiveChannel {
    pub fn new(activity: SharedActivity, receiver_handle: AbortOnDropHandle<()>) -> Self {
        Self {
            activity,
            receiver_handle,
        }
    }
}

/// Holds the application's runtime context, including the iroh client,
/// game document handle, current game state, and background task handles.
pub struct AppContext {
    // The iroh client instance used for all interactions.
    pub node: GossipNode,
    active_channel: Arc<TokioMutex<Option<ActiveChannel>>>,
    pub latest_ticket: Arc<TokioMutex<Option<String>>>,
    peers: Arc<TokioMutex<PeerMap>>,
}

impl AppContext {
    /// Creates a new, empty AppContext.
    pub fn new(gossip_node: GossipNode) -> Self {
        Self {
            node: gossip_node,
            active_channel: Arc::new(TokioMutex::new(None)),
            latest_ticket: Arc::new(TokioMutex::new(None)),
            peers: Arc::new(TokioMutex::new(Default::default())),
        }
    }
    /// Return a list of the known members of this Gossip Swarm.
    #[allow(unused)]
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.lock().await;
        peers.to_vec()
    }
    /// Send a message on the active channel.
    pub async fn send_message(&self, message: &str) -> anyhow::Result<()> {
        match self.active_channel.lock().await.as_ref() {
            Some(channel) => channel.activity.send_message(message).await,
            None => Err(anyhow!("Could not send message. No active channel.")),
        }
    }
    pub async fn get_message_log(&self) -> anyhow::Result<Vec<ChatMessage>> {
        match self.active_channel.lock().await.as_ref() {
            Some(channel) => Ok(channel.activity.get_messages().await?),
            None => Err(anyhow!("Could not get message log. No active channel.")),
        }
    }
    /// Return the active channel's id.
    pub async fn get_topic_id(&self) -> anyhow::Result<String> {
        match self.active_channel.lock().await.as_ref() {
            Some(channel) => Ok(channel.activity.id().to_string()),
            None => Err(anyhow!("Could not get topic ID. No active channel.")),
        }
    }
    /// Generate a new ticket token string.
    pub async fn generate_ticket(&self) -> anyhow::Result<String> {
        match self.active_channel.lock().await.as_ref() {
            Some(channel) => Ok(channel.activity.ticket()),
            None => Err(anyhow!("Could not generate ticket. No active channel.")),
        }
    }
    /// Close our connection to this room.  Returns deactivated topic ID.
    pub async fn drop_channel(&self) -> anyhow::Result<Option<String>> {
        match self.active_channel.lock().await.take() {
            Some(channel) => {
                channel.receiver_handle.abort();
                Ok(Some(channel.activity.id().to_string()))
            }
            None => Ok(None),
        }
    }
    /// Start the Docs channel.
    pub async fn start_channel(
        &self,
        doc_ticket: Option<DocTicket>,
        app_handle: &AppHandle,
        nickname: &str,
    ) -> anyhow::Result<String> {
        let activity = SharedActivity::new(doc_ticket, self.node.clone()).await?;
        let events: BoxStream<anyhow::Result<LiveEvent>> =
            Box::pin(activity.activity_subscribe().await?);

        // Spawn the event listener task
        let receiver_handle = self.spawn_event_listener(app_handle.clone(), events);
        let active_channel = ActiveChannel::new(activity, receiver_handle);
        active_channel.activity.set_nickname(nickname).await?;

        let topic_id = active_channel.activity.id().to_string();
        // Store the active channel info
        *self.active_channel.lock().await = Some(active_channel);
        // Get the topic_id from the established channel for logging
        Ok(topic_id)
    }

    /// Spawns a background task to listen for chat events and emit them to the frontend.
    fn spawn_event_listener(
        &self,
        app: tauri::AppHandle,
        mut events: BoxStream<anyhow::Result<LiveEvent>>,
    ) -> AbortOnDropHandle<()> {
        let mut tick_interval = interval(Duration::from_secs(2));
        let peers = self.peers.clone();
        let active_channel = self.active_channel.clone();
        let latest_ticket = self.latest_ticket.clone();

        // keep track of newly 'joined' peers to look out for their first
        // presense message.
        let mut new_starters: HashSet<NodeId> = HashSet::new();

        AbortOnDropHandle::new(n0_future::task::spawn(async move {
            loop {
                select! {
                    biased; // prioritize events if both are ready
                    event_result = events.next() => {
                        if handle_event(event_result, &peers, &active_channel, &latest_ticket, &app, &mut new_starters).await {
                            break; // Stop listening when the stream ends
                        };
                    },
                    _ = tick_interval.tick() => peers.lock().await.update(None, &mut new_starters, &app)
                }
            }
        }))
    }
}

/// Handle the event stream, if we want to break the loop we return True.
async fn handle_event(
    event_result: Option<anyhow::Result<LiveEvent>>,
    peers_clone: &Arc<TokioMutex<PeerMap>>,
    active_channel_clone: &Arc<TokioMutex<Option<ActiveChannel>>>,
    latest_ticket_clone: &Arc<TokioMutex<Option<String>>>,
    app: &AppHandle,
    new_starters: &mut HashSet<NodeId>,
) -> bool {
    match event_result {
        Some(Ok(event)) => {
            // for any event, check if we've updated the peers list (and emit peer-events)
            peers_clone
                .lock()
                .await
                .update(Some(&event), new_starters, app);
            // emit a chat-event for each event
            if let Err(e) = app.emit("chat-event", &event) {
                tracing::error!("Failed to emit event to frontend: {}", e);
            }
            // If a peer joins or a new neighbor comes up, update the latest_ticket
            update_ticket(&event, app, latest_ticket_clone, active_channel_clone).await;
        }
        Some(Err(e)) => {
            tracing::error!("Error receiving chat event: {}", e);
            let _ = app.emit(
                "chat-error",
                Event::Errorred {
                    message: e.to_string(),
                },
            );
        }
        None => {
            tracing::info!("Chat event stream ended.");
            let _ = app.emit("chat-event", Event::Disconnected);
            return true; // Stop listening when the stream ends
        }
    };
    false // Continue listening
}

/// If a peer joins or a new neighbor comes up, update the latest_ticket
/// with new peer nodes to assist reconnections.
async fn update_ticket(
    event: &LiveEvent,
    app: &AppHandle,
    latest_ticket_clone: &Arc<TokioMutex<Option<String>>>,
    active_channel_clone: &Arc<TokioMutex<Option<ActiveChannel>>>,
) {
    match &event {
        LiveEvent::NeighborUp { .. } => {
            tracing::debug!("Peer event detected, attempting to update latest ticket.");
            if let Some(active_channel_guard) = active_channel_clone.lock().await.as_ref() {
                let new_ticket_str = active_channel_guard.activity.ticket();
                *latest_ticket_clone.lock().await = Some(new_ticket_str.clone());
                tracing::info!("Updated latest_ticket due to new peer joining/neighbor up.");
                if let Err(e) = app.emit("ticket-updated", new_ticket_str) {
                    tracing::warn!("Failed to emit ticket-updated event: {}", e);
                }
            }
        }
        _ => {} // Other events do not trigger ticket update
    }
}

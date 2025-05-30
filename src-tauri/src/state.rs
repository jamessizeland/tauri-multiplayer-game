use crate::gossip::{
    ephemeral::{
        peers::{PeerInfo, PeerMap},
        ChatReceiver,
    },
    ChatSender, Event, GossipChannel, GossipNode, SharedActivity,
};
use anyhow::anyhow;
use iroh::NodeId;
use iroh_docs::DocTicket;
use n0_future::{task::AbortOnDropHandle, StreamExt as _};
use std::{collections::HashSet, sync::Arc};
use tauri::{AppHandle, Emitter as _};
use tokio::{
    select,
    sync::Mutex as TokioMutex,
    time::{interval, Duration},
};

/// Holds information about the currently active chat channel.
pub struct ActiveChannel {
    activity: SharedActivity,
    chat: GossipChannel,
    receiver_handle: AbortOnDropHandle<()>,
}

impl ActiveChannel {
    pub fn new(
        chat: GossipChannel,
        doc: SharedActivity,
        receiver_handle: AbortOnDropHandle<()>,
    ) -> Self {
        Self {
            chat,
            activity: doc,
            receiver_handle,
        }
    }
}

/// Holds the application's runtime context, including the iroh client,
/// game document handle, current game state, and background task handles.
pub struct AppContext {
    // The iroh client instance used for all interactions. Option<> because it's initialized async.
    pub node: Arc<TokioMutex<Option<GossipNode>>>,
    active_channel: Arc<TokioMutex<Option<ActiveChannel>>>,
    pub latest_ticket: Arc<TokioMutex<Option<String>>>,
    peers: Arc<TokioMutex<PeerMap>>,
}

impl AppContext {
    /// Creates a new, empty AppContext.
    pub fn new() -> Self {
        Self {
            node: Arc::new(TokioMutex::new(None)),
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
    /// Get the active channel's topic ID.
    pub async fn get_topic_id(&self) -> anyhow::Result<String> {
        match self.active_channel.lock().await.as_ref() {
            Some(channel) => Ok(channel.chat.id()),
            None => Err(anyhow!("Could not get Topic ID. No active channel.")),
        }
    }
    /// Generate a new ticket token string.
    pub async fn generate_ticket(&self) -> anyhow::Result<String> {
        match self.active_channel.lock().await.as_ref() {
            Some(channel) => channel.chat.ticket(options),
            None => Err(anyhow!("Could not generate ticket. No active channel.")),
        }
    }
    /// Send a message on the active channel. Returns active topic ID.
    pub async fn get_sender(&self) -> anyhow::Result<ChatSender> {
        match self.active_channel.lock().await.as_ref() {
            Some(channel) => Ok(channel.chat.sender()),
            None => Err(anyhow!("Could not get sender. No active channel.")),
        }
    }
    /// Close our connection to this room.  Returns deactivated topic ID.
    pub async fn drop_channel(&self) -> anyhow::Result<Option<String>> {
        match self.active_channel.lock().await.take() {
            Some(channel) => {
                channel.receiver_handle.abort();
                Ok(Some(channel.chat.id()))
            }
            None => Ok(None),
        }
    }
    /// Start both the Docs channel and the Ephemeral Chat channel.
    /// The chat channel's ticket is held inside the doc, so should be collected
    /// from there, or generated if it doesn't exist.
    pub async fn start_channel(
        &self,
        doc_ticket: Option<DocTicket>,
        app_handle: &AppHandle,
        nickname: &str,
    ) -> anyhow::Result<String> {
        let node_guard = self.node.lock().await;
        let Some(node) = node_guard.as_ref() else {
            return Err(anyhow!("Node not initialized").into());
        };
        let activity = SharedActivity::new(doc_ticket, node.clone()).await?;
        let chat_ticket = activity.get_chat_ticket().await?;

        // Use generate_channel from [chat::channel]
        let mut channel = node
            .generate_channel(chat_ticket, nickname)
            .map_err(|e| anyhow!("Failed to generate channel: {}", e))?;

        // Take the receiver from the Channel object to give to spawn_event_listener
        let rx = channel
            .take_receiver()
            .ok_or_else(|| anyhow!("Receiver already taken from channel object"))?;
        // Spawn the event listener task
        let receiver_handle = self.spawn_event_listener(app_handle.clone(), rx);
        let active_channel = ActiveChannel::new(channel, activity, receiver_handle);
        active_channel
            .chat
            .sender()
            .set_nickname(nickname.to_string());
        // Store the active channel info
        *self.active_channel.lock().await = Some(active_channel);
        // Get the topic_id from the established channel for logging
        let topic_id_str = self.get_topic_id().await?;
        tracing::info!(
            "Active channel SET in join_room for topic: {}",
            topic_id_str
        );
        Ok(topic_id_str)
    }

    /// Spawns a background task to listen for chat events and emit them to the frontend.
    fn spawn_event_listener(
        &self,
        app: tauri::AppHandle,
        mut receiver: ChatReceiver,
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
                    biased; // Optional: prioritize receiver events if both are ready
                    event_result = receiver.next() => { // `receiver` is moved into the task
                        if handle_event(event_result, &peers, &active_channel, &latest_ticket, &app, &mut new_starters).await {
                            break; // Stop listening when the stream ends
                        };
                    },
                    _ = tick_interval.tick() => {
                        // This branch runs every second
                        peers.lock().await.update(None, &mut new_starters, &app);
                    },
                }
            }
        }))
    }
}

/// Handle the event stream, if we want to break the loop we return True.
async fn handle_event(
    event_result: Option<anyhow::Result<Event>>,
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
    event: &Event,
    app: &AppHandle,
    latest_ticket_clone: &Arc<TokioMutex<Option<String>>>,
    active_channel_clone: &Arc<TokioMutex<Option<ActiveChannel>>>,
) {
    match &event {
        Event::Joined { .. } | Event::NeighborUp { .. } => {
            tracing::debug!("Peer event detected, attempting to update latest ticket.");
            if let Some(active_channel_guard) = active_channel_clone.lock().await.as_ref() {
                match active_channel_guard.chat.ticket(TicketOpts::all()) {
                    Ok(new_ticket_str) => {
                        *latest_ticket_clone.lock().await = Some(new_ticket_str.clone());
                        tracing::info!(
                            "Updated latest_ticket due to new peer joining/neighbor up."
                        );
                        if let Err(e) = app.emit("ticket-updated", new_ticket_str) {
                            tracing::warn!("Failed to emit ticket-updated event: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to regenerate ticket after peer event: {}", e);
                    }
                }
            }
        }
        _ => {} // Other events do not trigger ticket update
    }
}

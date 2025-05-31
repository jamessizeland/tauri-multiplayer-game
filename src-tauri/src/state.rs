use crate::gossip::{
    doc::{chat::ChatMessage, peers::PeerStatus, SharedActivity},
    spawn_event_listener, GossipNode,
};
use anyhow::anyhow;
use iroh_docs::DocTicket;
use n0_future::task::AbortOnDropHandle;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex as TokioMutex;

/// Holds information about the currently active game.
pub struct ActiveChannel {
    name: String,
    pub activity: SharedActivity,
    receiver_handle: AbortOnDropHandle<()>,
}

impl ActiveChannel {
    pub fn new(
        activity: SharedActivity,
        receiver_handle: AbortOnDropHandle<()>,
        name: &str,
    ) -> Self {
        Self {
            name: name.to_string(),
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
}

impl AppContext {
    /// Creates a new, empty AppContext.
    pub fn new(gossip_node: GossipNode) -> Self {
        Self {
            node: gossip_node,
            active_channel: Arc::new(TokioMutex::new(None)),
            latest_ticket: Arc::new(TokioMutex::new(None)),
        }
    }
    /// Return a list of the known members of this Gossip Swarm.
    #[allow(unused)]
    pub async fn get_peers(&self) -> Vec<String> {
        // let peers = self.peers.lock().await;
        // peers.to_vec()
        todo!()
    }
    /// Send a message on the active channel.
    pub async fn send_message(&self, message: &str) -> anyhow::Result<()> {
        match self.active_channel.lock().await.as_ref() {
            Some(channel) => channel.activity.send_message(&channel.name, message).await,
            None => Err(anyhow!("Could not send message. No active channel.")),
        }
    }
    /// Return the full message log so far for all connected participants.
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

        // Spawn the event listener task
        let receiver_handle = spawn_event_listener(
            app_handle.clone(),
            Box::pin(activity.activity_subscribe().await?),
            self.active_channel.clone(),
            self.latest_ticket.clone(),
        );
        let active_channel = ActiveChannel::new(activity, receiver_handle, nickname);
        active_channel
            .activity
            .set_peer_info(nickname, PeerStatus::Online { ready: false })
            .await?;

        let topic_id = active_channel.activity.id().to_string();
        // Store the active channel info
        *self.active_channel.lock().await = Some(active_channel);
        // Get the topic_id from the established channel for logging
        Ok(topic_id)
    }
}

use crate::gossip::{
    doc::{
        chat::ChatMessage,
        peers::{PeerInfo, PeerStatus},
        SharedActivity,
    },
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
    pub latest_ticket: TokioMutex<Option<String>>,
}

impl AppContext {
    /// Creates a new, empty AppContext.
    pub fn new(gossip_node: GossipNode) -> Self {
        Self {
            node: gossip_node,
            active_channel: Arc::new(TokioMutex::new(None)),
            latest_ticket: TokioMutex::new(None),
        }
    }
    /// Return a list of the known members of this Gossip Swarm.
    pub async fn get_peers(&self) -> anyhow::Result<Vec<PeerInfo>> {
        match self.active_channel.lock().await.as_ref() {
            Some(channel) => channel.activity.get_all_peer_info().await,
            None => Err(anyhow!("Could not get peers. No active channel.")),
        }
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
    /// Generate a new ticket token string or use the existing one.
    pub async fn generate_ticket(&self) -> anyhow::Result<String> {
        let mut latest_ticket = self.latest_ticket.lock().await;
        match self.active_channel.lock().await.as_ref() {
            Some(channel) => {
                let ticket = channel.activity.ticket().await?;
                *latest_ticket = Some(ticket.clone());
                Ok(ticket)
            }
            None => match latest_ticket.clone() {
                Some(ticket) => Ok(ticket),
                None => Err(anyhow!("Could not generate ticket. No active channel.")),
            },
        }
    }
    /// Close our connection to this room.  Returns deactivated topic ID.
    pub async fn drop_channel(&self) -> anyhow::Result<Option<String>> {
        self.generate_ticket().await.ok(); // try to generate a new ticket
        match self.active_channel.lock().await.take() {
            Some(channel) => {
                channel.activity.set_status(PeerStatus::Offline).await?;
                let id = channel.activity.id().to_string();
                channel.activity.close().await?;
                channel.receiver_handle.abort();
                Ok(Some(id))
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
        );
        let active_channel = ActiveChannel::new(activity, receiver_handle, nickname);

        active_channel.activity.set_nickname(nickname).await?;
        active_channel
            .activity
            .set_status(PeerStatus::Online)
            .await?;

        let topic_id = active_channel.activity.id().to_string();
        // Store the active channel info
        *self.active_channel.lock().await = Some(active_channel);

        Ok(topic_id)
    }
}

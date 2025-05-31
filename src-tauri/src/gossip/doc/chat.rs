use crate::{
    gossip::doc::{SharedActivity, MESSAGES_PREFIX},
    utils::get_timestamp,
};
use iroh::NodeId;
use iroh_docs::{
    store::{Query, SortBy, SortDirection},
    AuthorId,
};
use n0_future::StreamExt as _;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ChatMessage {
    /// NodeId of the peer who sent the message
    pub sender_node_id: NodeId,
    /// Nickname of the sender at the time of sending
    pub nickname: String,
    /// Message payload
    pub content: String,
    /// milliseconds since EPOCH
    pub timestamp: u64,
}

// Helper to create unique, sortable message keys
fn message_key(timestamp_millis: u64, author_id: &AuthorId) -> Vec<u8> {
    let mut key = MESSAGES_PREFIX.to_vec();
    key.extend_from_slice(&timestamp_millis.to_be_bytes());
    key.extend_from_slice(b"_"); // Separator
    key.extend_from_slice(&author_id.as_bytes()[..8]); // Suffix for uniqueness
    key
}

impl SharedActivity {
    /// Add a message to the shared log.
    pub async fn send_message(&self, name: &str, message_content: &str) -> anyhow::Result<()> {
        let timestamp = get_timestamp(); // in millis
        let key = message_key(timestamp, &self.author_id);
        let chat_message = ChatMessage {
            timestamp,
            sender_node_id: self.gossip.node_id(),
            nickname: name.to_string(),
            content: message_content.to_string(),
        };
        self.activity
            .set_bytes(self.author_id, key, postcard::to_stdvec(&chat_message)?)
            .await?;
        Ok(())
    }

    /// Get all messages from the shared log, ordered by time.
    /// Returns a vector of (timestamp_millis, author_id_prefix, message_string).
    pub async fn get_messages(&self) -> anyhow::Result<Vec<ChatMessage>> {
        let query = Query::key_prefix(MESSAGES_PREFIX);
        let mut entries = self.activity.get_many(query).await?;
        let mut messages: Vec<ChatMessage> = Vec::new();

        while let Some(Ok(entry)) = entries.next().await {
            let bytes = self.read_bytes(entry).await?;
            let message: ChatMessage = postcard::from_bytes(&bytes)?;
            messages.push(message);
        }
        messages.sort_by_key(|m| m.timestamp);
        Ok(messages)
    }

    #[allow(dead_code)]
    fn parse_message_key(key: &[u8]) -> Option<(u64, Vec<u8>)> {
        if key.starts_with(MESSAGES_PREFIX) && key.len() > MESSAGES_PREFIX.len() + 8 + 1 {
            let ts_bytes_end = MESSAGES_PREFIX.len() + 8;
            let timestamp =
                u64::from_be_bytes(key[MESSAGES_PREFIX.len()..ts_bytes_end].try_into().ok()?);
            let author_prefix = key[ts_bytes_end + 1..].to_vec(); // +1 for the separator '_'
            Some((timestamp, author_prefix))
        } else {
            None
        }
    }
}

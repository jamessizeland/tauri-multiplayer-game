use crate::gossip::doc::SharedActivity;
use crate::{
    gossip::{types::ChatMessage, GossipNode},
    utils::get_timestamp,
};
use bytes::Bytes;
use iroh::{NodeId, PublicKey};
use iroh_blobs::Hash;
use iroh_docs::{
    engine::LiveEvent,
    rpc::client::docs::{Doc, ShareMode},
    store::{Query, SortBy, SortDirection},
    AuthorId, DocTicket, Entry, NamespaceId,
};
use n0_future::{Stream, StreamExt as _};
use tracing::info;

// Key constants for structuring data in the iroh-doc
pub const PEERS_PREFIX: &[u8] = b"peers/";
pub const NICKNAME_KEY_SUFFIX: &[u8] = b"/nickname";
pub const MESSAGES_PREFIX: &[u8] = b"messages/";
pub const GAME_STATE_KEY: &[u8] = b"game_state";

// Helper to create peer-specific nickname keys
fn peer_nickname_key(node_id: &iroh::NodeId) -> Vec<u8> {
    [PEERS_PREFIX, node_id.as_bytes(), NICKNAME_KEY_SUFFIX].concat()
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
    /// Helper function to write to the document
    async fn write(&self, key: impl Into<Bytes>, value: impl Into<Bytes>) -> anyhow::Result<Hash> {
        self.activity
            .set_bytes(self.author_id, key.into(), value.into())
            .await
    }
    /// Helper function to read one entry from the document
    async fn read_unique(&self, key: impl Into<Bytes>) -> anyhow::Result<Option<Entry>> {
        let query = Query::key_exact(key.into());
        self.activity.get_one(query).await
    }
    async fn read_bytes(&self, entry: Entry) -> anyhow::Result<Bytes> {
        self.gossip.blobs.read_to_bytes(entry.content_hash()).await
    }
    /// Set the nickname for the current node.
    pub async fn set_nickname(&self, nickname: &str) -> anyhow::Result<()> {
        info!("Setting nickname to {}", nickname);
        let key = peer_nickname_key(&self.gossip.node_id());
        self.write(key, nickname.as_bytes().to_vec()).await?;
        let resp = self.get_nickname(&self.gossip.node_id()).await?;
        info!("Nickname set to {:?}", resp);
        Ok(())
    }

    /// Get the nickname for a given node_id.
    pub async fn get_nickname(&self, node_id: &iroh::NodeId) -> anyhow::Result<Option<String>> {
        let key = peer_nickname_key(node_id);
        match self.read_unique(key).await? {
            None => Ok(None),
            Some(entry) => {
                let bytes = self.read_bytes(entry).await?;
                Ok(Some(String::from_utf8(bytes.into())?))
            }
        }
    }

    /// Send a message to the shared log.
    pub async fn send_message(&self, message_content: &str) -> anyhow::Result<()> {
        let timestamp_millis = get_timestamp();
        let key = message_key(timestamp_millis, &self.author_id);
        self.activity
            .set_bytes(self.author_id, key, message_content.as_bytes().to_vec())
            .await?;
        Ok(())
    }

    /// Get all messages from the shared log, ordered by time.
    /// Returns a vector of (timestamp_millis, author_id_prefix, message_string).
    pub async fn get_messages(&self) -> anyhow::Result<Vec<ChatMessage>> {
        let query = Query::key_prefix(MESSAGES_PREFIX.to_vec())
            .sort_by(SortBy::KeyAuthor, SortDirection::Asc);
        let mut entries = self.activity.get_many(query).await?;
        let mut messages: Vec<ChatMessage> = Vec::new();

        while let Some(Ok(entry)) = entries.next().await {
            let key = entry.key();
            // Expected key structure: MESSAGES_PREFIX | 8_byte_timestamp_millis | b"_" | 8_byte_author_id_prefix
            if key.len() > MESSAGES_PREFIX.len() + 8 + 1 {
                let timestamp_bytes_slice = &key[MESSAGES_PREFIX.len()..MESSAGES_PREFIX.len() + 8];
                let mut timestamp_bytes = [0u8; 8];
                timestamp_bytes.copy_from_slice(timestamp_bytes_slice);
                let timestamp = u64::from_be_bytes(timestamp_bytes);
                let author_id = entry.author();
                let sender_node_id = NodeId::from_bytes(author_id.into_public_key()?.as_bytes())?;

                let nickname = self
                    .get_nickname(&sender_node_id)
                    .await?
                    .unwrap_or_else(|| {
                        // Provide a default nickname if none is set for the author
                        format!(
                            "peer:{}",
                            sender_node_id
                                .to_string()
                                .chars()
                                .take(6)
                                .collect::<String>()
                        )
                    });
                let content = entry.content_hash();
                let message_string = content.to_string();
                let message = ChatMessage {
                    sender_node_id,
                    nickname,
                    content: message_string,
                    timestamp,
                };
                messages.push(message);
            }
        }
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

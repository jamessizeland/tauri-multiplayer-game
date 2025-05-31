use crate::gossip::doc::{SharedActivity, NICKNAME_KEY_SUFFIX, PEERS_PREFIX};
use iroh_docs::store::Query;
use n0_future::StreamExt as _;
use serde::{Deserialize, Serialize};
use tracing::info;

// Helper to create peer-specific nickname keys
fn peer_nickname_key(node_id: &iroh::NodeId) -> Vec<u8> {
    [PEERS_PREFIX, node_id.as_bytes(), NICKNAME_KEY_SUFFIX].concat()
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum PeerStatus {
    Online { ready: bool },
    Offline,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerInfo {
    pub node_id: iroh::NodeId,
    pub nickname: String,
    pub status: PeerStatus,
}

impl SharedActivity {
    /// Set the nickname and status for the current node.
    pub async fn set_peer_info(&self, nickname: &str, status: PeerStatus) -> anyhow::Result<()> {
        info!("Setting nickname to {}", nickname);
        let key = peer_nickname_key(&self.gossip.node_id());
        let peer_info = PeerInfo {
            node_id: self.gossip.node_id(),
            nickname: nickname.to_string(),
            status,
        };
        self.write(key, postcard::to_stdvec(&peer_info)?).await?;
        Ok(())
    }

    /// Get the peer info for a given node_id.
    pub async fn get_peer_info(&self, node_id: &iroh::NodeId) -> anyhow::Result<Option<PeerInfo>> {
        let key = peer_nickname_key(node_id);
        match self.read_unique(key).await? {
            None => Ok(None),
            Some(entry) => {
                let bytes = self.read_bytes(entry).await?;
                Ok(Some(postcard::from_bytes(&bytes)?))
            }
        }
    }
    /// Get all peers that have been registered in this document.
    pub async fn get_all_peer_info(&self) -> anyhow::Result<Vec<PeerInfo>> {
        let query = Query::key_prefix(PEERS_PREFIX);
        let mut entries = self.activity.get_many(query).await?;
        let mut peers = Vec::new();
        while let Some(Ok(entry)) = entries.next().await {
            let bytes = self.read_bytes(entry).await?;
            let peer_info: PeerInfo = postcard::from_bytes(&bytes)?;
            peers.push(peer_info);
        }
        Ok(peers)
    }
}

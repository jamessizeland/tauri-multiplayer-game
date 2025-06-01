use crate::gossip::doc::{SharedActivity, NICKNAME_KEY_SUFFIX, PEERS_PREFIX};
use anyhow::anyhow;
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
    Online,
    Offline,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerInfo {
    pub id: iroh::NodeId,
    pub nickname: String,
    pub status: PeerStatus,
    pub ready: bool,
}

impl SharedActivity {
    /// Set our nickname.
    pub async fn set_nickname(&self, nickname: &str) -> anyhow::Result<()> {
        info!("Setting nickname to {}", nickname);
        let node_id = self.gossip.node_id();
        let key = peer_nickname_key(&node_id);
        let peer_info = match self.get_peer_info(&node_id).await? {
            None => PeerInfo {
                id: self.gossip.node_id(),
                nickname: nickname.to_string(),
                status: PeerStatus::Online,
                ready: false,
            },
            Some(mut peer) => {
                peer.nickname = nickname.to_string();
                peer
            }
        };
        self.write(key, postcard::to_stdvec(&peer_info)?).await?;
        Ok(())
    }
    /// Set our status
    pub async fn set_status(&self, status: PeerStatus) -> anyhow::Result<()> {
        info!("Setting status to {:?}", status);
        let node_id = self.gossip.node_id();
        let key = peer_nickname_key(&node_id);
        let Some(mut peer) = self.get_peer_info(&node_id).await? else {
            return Err(anyhow!("Peer not found"));
        };
        peer.status = status;
        self.write(key, postcard::to_stdvec(&peer)?).await?;
        Ok(())
    }
    /// Get the peer info for a given node_id.
    pub async fn get_peer_info(&self, node_id: &iroh::NodeId) -> anyhow::Result<Option<PeerInfo>> {
        let key = peer_nickname_key(node_id);
        match self.read_unique(key).await? {
            None => Ok(None),
            Some(entry) => {
                let bytes = self.read_bytes(entry.content_hash()).await?;
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
            let bytes = self.read_bytes(entry.content_hash()).await?;
            let peer_info: PeerInfo = postcard::from_bytes(&bytes)?;
            peers.push(peer_info);
        }
        Ok(peers)
    }
}

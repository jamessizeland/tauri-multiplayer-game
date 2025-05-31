use std::collections::{HashMap, HashSet};

use iroh::{NodeId, PublicKey};
use iroh_docs::engine::LiveEvent;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter as _};

use crate::utils::get_timestamp;

#[derive(Default)]
pub struct PeerMap(HashMap<NodeId, PeerInfo>);

impl PeerMap {
    pub fn get_mut(&mut self, id: &NodeId) -> Option<&mut PeerInfo> {
        self.0.get_mut(id)
    }
    pub fn to_vec(&self) -> Vec<PeerInfo> {
        self.0.values().cloned().collect()
    }
    /// Update the activity of the peers list. Returns a list of updated peers if updated.
    pub fn update(
        &mut self,
        event: Option<&LiveEvent>,
        new_starters: &mut HashSet<PublicKey>,
        app: &AppHandle,
    ) {
        let before = self.to_vec();
        let map = &mut self.0;
        match event {
            Some(LiveEvent::NeighborDown(id)) => {
                // node reported to have left the room.
                map.entry(*id)
                    .and_modify(|peer| {
                        peer.status = PeerStatus::Offline;
                        peer.last_seen = get_timestamp();
                    })
                    .or_insert(PeerInfo::new(*id, None));
            }
            Some(LiveEvent::NeighborUp(id)) => {
                // node reported to have rejoined the room
                map.entry(*id)
                    .and_modify(|peer| {
                        peer.status = PeerStatus::Online;
                        peer.last_seen = get_timestamp();
                    })
                    .or_insert(PeerInfo::new(*id, None));
            }
            None => {
                // tick at regular intervals to update the peerStatus
                for peer in map.values_mut() {
                    let millis_since_last_seen =
                        (get_timestamp().saturating_sub(peer.last_seen)) / 1000;
                    if millis_since_last_seen > 10_000 && peer.status != PeerStatus::Offline {
                        peer.status = PeerStatus::Away;
                    };
                }
            }
            _ => return, // ignore other events for now,
        }
        let after = self.to_vec();
        if before != after {
            if let Err(e) = app.emit("peers-event", after) {
                tracing::error!("Failed to emit event to frontend: {}", e);
            }
        }
    }
}

/// Information for the frontend to display about known peers
/// in the Gossip Swarm.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub id: NodeId,
    pub nickname: String,
    pub last_seen: u64,
    pub role: PeerRole,
    pub status: PeerStatus,
}

impl PeerInfo {
    fn new(id: NodeId, nickname: Option<String>) -> Self {
        Self {
            id,
            nickname: nickname.unwrap_or("identifying...".to_string()),
            last_seen: get_timestamp(),
            role: PeerRole::RemoteNode,
            status: PeerStatus::Online,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PeerRole {
    Myself,
    RemoteNode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PeerStatus {
    Online,
    Away,
    Offline,
}

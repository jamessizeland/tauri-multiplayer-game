pub use iroh::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

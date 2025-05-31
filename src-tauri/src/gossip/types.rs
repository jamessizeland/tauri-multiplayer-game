use iroh::NodeId;
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

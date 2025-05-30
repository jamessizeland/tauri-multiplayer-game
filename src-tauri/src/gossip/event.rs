use anyhow::Context as _;
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

// impl TryFrom<iroh_gossip::net::Event> for Event {
//     type Error = anyhow::Error;
//     fn try_from(event: iroh_gossip::net::Event) -> Result<Self, Self::Error> {
//         let converted = match event {
//             iroh_gossip::net::Event::Gossip(event) => match event {
//                 GossipEvent::Joined(neighbors) => Self::Joined { neighbors },
//                 GossipEvent::NeighborUp(node_id) => Self::NeighborUp { node_id },
//                 GossipEvent::NeighborDown(node_id) => Self::NeighborDown { node_id },
//                 GossipEvent::Received(message) => {
//                     let message = SignedMessage::verify_and_decode(&message.content)
//                         .context("failed to parse and verify signed message")?;
//                     match message.message {
//                         Message::Presence { nickname } => Self::Presence {
//                             from: message.from,
//                             nickname,
//                             sent_timestamp: message.timestamp,
//                         },
//                         Message::Message { text, nickname } => Self::MessageReceived {
//                             from: message.from,
//                             text,
//                             nickname,
//                             sent_timestamp: message.timestamp,
//                         },
//                     }
//                 }
//             },
//             iroh_gossip::net::Event::Lagged => Self::Lagged,
//         };
//         Ok(converted)
//     }
// }

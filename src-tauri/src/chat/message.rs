use anyhow::Result;
pub use iroh::NodeId;
use iroh::{PublicKey, SecretKey};
use iroh_base::Signature;
use serde::{Deserialize, Serialize};

use crate::{game::GameState, utils::get_timestamp};

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedMessage {
    from: PublicKey,
    data: Vec<u8>,
    signature: Signature,
}

impl SignedMessage {
    pub fn verify_and_decode(bytes: &[u8]) -> Result<ReceivedMessage> {
        let signed_message: Self = postcard::from_bytes(bytes)?;
        let key: PublicKey = signed_message.from;
        key.verify(&signed_message.data, &signed_message.signature)?;
        let message: WireMessage = postcard::from_bytes(&signed_message.data)?;
        let WireMessage::VO { timestamp, message } = message;
        Ok(ReceivedMessage {
            from: signed_message.from,
            timestamp,
            message,
        })
    }

    pub fn sign_and_encode(secret_key: &SecretKey, message: Message) -> Result<Vec<u8>> {
        let timestamp = get_timestamp();
        let wire_message = WireMessage::VO { timestamp, message };
        let data = postcard::to_stdvec(&wire_message)?;
        let signature = secret_key.sign(&data);
        let from: PublicKey = secret_key.public();
        let signed_message = Self {
            from,
            data,
            signature,
        };
        let encoded = postcard::to_stdvec(&signed_message)?;
        Ok(encoded)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WireMessage {
    VO { timestamp: u64, message: Message },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    /// Ping other connected users to tell them we're still here.
    Presence { nickname: String },
    /// Send a new text message to connected users.
    Message { text: String, nickname: String },
    /// Send the latest game state as far as we know it.
    GameState { state: GameState },
    /// Request catch up information from connected users.
    RequestSync,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceivedMessage {
    pub timestamp: u64,
    pub from: NodeId,
    pub message: Message,
}

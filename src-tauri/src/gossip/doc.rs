//! Any logic related to updating and synchronizing of the Document used for
//! sharing state data between nodes.

use std::str::FromStr as _;

use crate::gossip::{types::DocsRPCConnector, GossipNode};
use anyhow::Context as _;
use iroh_docs::{
    engine::LiveEvent,
    rpc::client::docs::{Doc, ShareMode},
    store::Query,
    AuthorId, DocTicket,
};
use n0_future::Stream;
use tracing::info;

const PUBLIC_CHAT_KEY: &str = "public-chat";

/// Shared state data synchronized between connected nodes.
/// The doc holds all information about the current shared activity.
/// This includes the ephemeral chat ticket.
/// There can be multiple shared activities within this application,
/// representing a person starting multiple games at once.
pub struct SharedActivity {
    gossip: GossipNode,
    activity: Doc<DocsRPCConnector>,
    ticket: DocTicket,
    author: AuthorId,
}

impl SharedActivity {
    /// Begin or join a new shared activity session.
    pub async fn new(ticket: Option<String>, gossip: GossipNode) -> anyhow::Result<Self> {
        let author = gossip.docs.authors().create().await?;
        let activity: Doc<DocsRPCConnector> = match ticket {
            None => gossip.docs.create().await?,
            Some(ticket) => {
                let ticket = DocTicket::from_str(&ticket)?;
                gossip.docs.import(ticket).await?
            }
        };
        let ticket = activity.share(ShareMode::Write, Default::default()).await?;
        Ok(Self {
            gossip,
            activity,
            ticket,
            author,
        })
    }
    /// Get the stringified ticket information to share with others.
    pub fn ticket(&self) -> String {
        self.ticket.to_string()
    }

    /// Subscribe to updates from this activity.
    pub async fn activity_subscribe(
        &self,
    ) -> anyhow::Result<impl Stream<Item = anyhow::Result<LiveEvent>>> {
        self.activity.subscribe().await
    }
    /// We are storing a simple chat ticket inside this document for ephemeral chats
    pub async fn get_chat_ticket(&self) -> anyhow::Result<String> {
        let entry = self
            .activity
            .get_one(Query::single_latest_per_key().key_exact(PUBLIC_CHAT_KEY))
            .await?
            .ok_or_else(|| anyhow::anyhow!("no chat ticket found"))?;
        let id = String::from_utf8(entry.key().to_owned()).context("invalid key")?;
        info!("chat ticket id: {id}");
        let bytes = self
            .gossip
            .blobs
            .read_to_bytes(entry.content_hash())
            .await?;
        serde_json::from_slice(&bytes).context("invalid json")
    }
}

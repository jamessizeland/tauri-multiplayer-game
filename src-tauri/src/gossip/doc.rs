//! Any logic related to updating and synchronizing of the Document used for
//! sharing state data between nodes.

use crate::gossip::{ephemeral::ticket::ChatTicket, GossipNode};
use anyhow::Context as _;
use iroh_docs::{
    engine::LiveEvent,
    rpc::client::docs::{Doc, ShareMode},
    store::Query,
    AuthorId, DocTicket,
};
use n0_future::Stream;
use tracing::info;

use iroh_blobs::rpc::{client::blobs, proto as blobs_proto};
use iroh_docs::rpc::{client::docs, proto as docs_proto};
use quic_rpc::transport::flume::FlumeConnector;

pub type BlobsRPCConnector = FlumeConnector<blobs_proto::Response, blobs_proto::Request>;

pub type DocsRPCConnector = FlumeConnector<docs_proto::Response, docs_proto::Request>;

pub type BlobsClient = blobs::Client<BlobsRPCConnector>;

pub type DocsClient = docs::Client<DocsRPCConnector>;

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
    author_id: AuthorId,
}

impl SharedActivity {
    /// Begin or join a new shared activity session.
    pub async fn new(ticket: Option<DocTicket>, gossip: GossipNode) -> anyhow::Result<Self> {
        let author = gossip.docs.authors().create().await?;
        let activity: Doc<DocsRPCConnector> = match ticket {
            None => gossip.docs.create().await?,
            Some(ticket) => gossip.docs.import(ticket).await?,
        };
        let ticket = activity.share(ShareMode::Write, Default::default()).await?;
        Ok(Self {
            gossip,
            activity,
            ticket,
            author_id: author,
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
    async fn insert_bytes(
        &self,
        key: impl AsRef<[u8]>,
        content: bytes::Bytes,
    ) -> anyhow::Result<()> {
        self.activity
            .set_bytes(self.author_id, key.as_ref().to_vec(), content)
            .await?;
        Ok(())
    }
    /// Write a new chat ticket or update it if we've added new peers.
    pub fn update_chat_ticket(&self, ticket: ChatTicket) -> anyhow::Result<()> {
        self.insert_bytes(PUBLIC_CHAT_KEY, serde_json::to_vec(&ticket)?.into());
        Ok(())
    }
    /// We are storing a simple chat ticket inside this document for ephemeral chats
    pub async fn get_chat_ticket(&self) -> anyhow::Result<ChatTicket> {
        let query = self
            .activity
            .get_one(Query::single_latest_per_key().key_exact(PUBLIC_CHAT_KEY));
        match query.await? {
            None => {
                // generate a new ticket and add it to the document.
                let chat_ticket = ChatTicket::new_random();
                self.update_chat_ticket(chat_ticket.clone())?;
                Ok(chat_ticket)
            }
            Some(entry) => {
                // read the existing chat key.
                let id = String::from_utf8(entry.key().to_owned()).context("invalid key")?;
                info!("chat ticket id: {id}");
                let bytes = self
                    .gossip
                    .blobs
                    .read_to_bytes(entry.content_hash())
                    .await?;
                let ticket: String = serde_json::from_slice(&bytes).context("invalid json")?;
                Ok(ChatTicket::deserialize(&ticket)?)
            }
        }
    }
}

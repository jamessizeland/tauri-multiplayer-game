//! Any logic related to updating and synchronizing of the Document used for
//! sharing state data between nodes.

use crate::gossip::GossipNode;
use anyhow::Context as _;
use iroh_docs::{
    engine::LiveEvent,
    rpc::client::docs::{Doc, ShareMode},
    AuthorId, DocTicket, NamespaceId,
};
use n0_future::Stream;

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
    /// Return the ID of this activity
    pub fn id(&self) -> NamespaceId {
        self.activity.id()
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
}

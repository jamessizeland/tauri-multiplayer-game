//! Any logic related to updating and synchronizing of the Document used for
//! sharing state data between nodes.

pub mod accessors;

use crate::{
    gossip::{types::ChatMessage, GossipNode},
    utils::get_timestamp,
};
use iroh::{NodeId, PublicKey};
use iroh_docs::{
    engine::LiveEvent,
    rpc::client::docs::{Doc, ShareMode},
    store::{Query, SortBy, SortDirection},
    AuthorId, DocTicket, Entry, NamespaceId,
};
use n0_future::{Stream, StreamExt as _};

use iroh_blobs::rpc::{client::blobs, proto as blobs_proto};
use iroh_docs::rpc::{client::docs, proto as docs_proto};
use quic_rpc::transport::flume::FlumeConnector;

pub type BlobsRPCConnector = FlumeConnector<blobs_proto::Response, blobs_proto::Request>;

pub type DocsRPCConnector = FlumeConnector<docs_proto::Response, docs_proto::Request>;

pub type BlobsClient = blobs::Client<BlobsRPCConnector>;

pub type DocsClient = docs::Client<DocsRPCConnector>;

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
}

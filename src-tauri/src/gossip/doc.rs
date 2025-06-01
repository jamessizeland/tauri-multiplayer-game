//! Any logic related to updating and synchronizing of the Document used for
//! sharing state data between nodes.

pub mod chat;
pub mod peers;

use std::ops::Deref;

use crate::gossip::GossipNode;
use bytes::Bytes;
use iroh_blobs::rpc::{client::blobs, proto as blobs_proto};
use iroh_blobs::Hash;
use iroh_docs::rpc::{client::docs, proto as docs_proto};
use iroh_docs::store::Query;
use iroh_docs::Entry;
use iroh_docs::{
    engine::LiveEvent,
    rpc::client::docs::{Doc, ShareMode},
    AuthorId, DocTicket, NamespaceId,
};
use n0_future::Stream;
use quic_rpc::transport::flume::FlumeConnector;

pub type BlobsRPCConnector = FlumeConnector<blobs_proto::Response, blobs_proto::Request>;

pub type DocsRPCConnector = FlumeConnector<docs_proto::Response, docs_proto::Request>;

pub type BlobsClient = blobs::Client<BlobsRPCConnector>;

pub type DocsClient = docs::Client<DocsRPCConnector>;

// Key constants for structuring data in the iroh-doc
pub const PEERS_PREFIX: &[u8] = b"peers/";
pub const NICKNAME_KEY_SUFFIX: &[u8] = b"/nickname";
pub const MESSAGES_PREFIX: &[u8] = b"messages/";
pub const GAME_STATE_KEY: &[u8] = b"game_state";

/// Shared state data synchronized between connected nodes.
/// The doc holds all information about the current shared activity.
/// This includes the ephemeral chat ticket.
/// There can be multiple shared activities within this application,
/// representing a person starting multiple games at once.
pub struct SharedActivity {
    gossip: GossipNode,
    activity: Doc<DocsRPCConnector>,
    #[allow(unused)]
    ticket: DocTicket,
    author_id: AuthorId,
}

impl Deref for SharedActivity {
    type Target = Doc<DocsRPCConnector>;
    fn deref(&self) -> &Self::Target {
        &self.activity
    }
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
    pub async fn ticket(&self) -> anyhow::Result<String> {
        let ticket = self
            .activity
            .share(ShareMode::Write, Default::default())
            .await?;
        Ok(ticket.to_string())
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

    /// Helper function to write to the document
    pub(self) async fn write(
        &self,
        key: impl Into<Bytes>,
        value: impl Into<Bytes>,
    ) -> anyhow::Result<Hash> {
        self.activity
            .set_bytes(self.author_id, key.into(), value.into())
            .await
    }
    /// Helper function to read one entry from the document
    pub(self) async fn read_unique(&self, key: impl Into<Bytes>) -> anyhow::Result<Option<Entry>> {
        let query = Query::key_exact(key.into());
        self.activity.get_one(query).await
    }
    /// Helper function to get the content bytes associated with an entry
    pub async fn read_bytes(&self, hash: Hash) -> anyhow::Result<Bytes> {
        self.gossip.blobs.read_to_bytes(hash).await
    }
}

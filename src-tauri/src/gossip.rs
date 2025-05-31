pub mod doc;
mod event;
pub mod peers;
pub mod types;

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use doc::{BlobsClient, DocsClient};
pub use event::Event;
pub use iroh::NodeId;
use iroh::{endpoint::RemoteInfo, protocol::Router, SecretKey};
use iroh_blobs::net_protocol::Blobs;
use iroh_docs::protocol::Docs;
use iroh_gossip::net::Gossip;
use tracing::{info, warn};

#[derive(Clone)]
pub struct GossipNode {
    secret_key: SecretKey,
    router: Router,
    gossip: Gossip,
    blobs: BlobsClient,
    docs: DocsClient,
}

impl GossipNode {
    /// Spawns a gossip node.
    pub async fn spawn(secret_key: Option<SecretKey>, path: PathBuf) -> Result<Self> {
        let secret_key = secret_key.unwrap_or_else(|| SecretKey::generate(rand::rngs::OsRng));
        let endpoint = iroh::Endpoint::builder()
            .discovery_n0()
            .secret_key(secret_key.clone())
            .bind()
            .await?;

        let node_id = endpoint.node_id();
        info!("endpoint bound");
        info!("node id: {node_id:#?}");

        // build the protocol router
        let mut builder = iroh::protocol::Router::builder(endpoint);

        let gossip = Gossip::builder().spawn(builder.endpoint().clone()).await?;
        builder = builder.accept(iroh_gossip::ALPN, Arc::new(gossip.clone()));
        info!("gossip spawned");
        let blobs = Blobs::persistent(&path).await?.build(builder.endpoint());
        builder = builder.accept(iroh_blobs::ALPN, blobs.clone());
        info!("blobs spawned");
        let docs = Docs::persistent(path).spawn(&blobs, &gossip).await?;
        builder = builder.accept(iroh_docs::ALPN, Arc::new(docs.clone()));
        info!("docs spawned");
        Ok(Self {
            gossip,
            secret_key,
            router: builder.spawn(),
            blobs: blobs.client().clone(),
            docs: docs.client().clone(),
        })
    }

    /// Returns the node id of this node.
    pub fn node_id(&self) -> NodeId {
        self.router.endpoint().node_id()
    }

    #[allow(unused)]
    /// Returns information about all the remote nodes this [`Endpoint`] knows about.
    pub fn remote_info(&self) -> Vec<RemoteInfo> {
        self.router
            .endpoint()
            .remote_info_iter()
            .collect::<Vec<_>>()
    }

    pub async fn shutdown(&self) {
        if let Err(err) = self.router.shutdown().await {
            warn!("failed to shutdown router cleanly: {err}");
        }
        self.router.endpoint().close().await;
    }
}

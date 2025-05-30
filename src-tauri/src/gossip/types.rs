use crate::gossip::Event;
use iroh_blobs::rpc::{client::blobs, proto as blobs_proto};
use iroh_docs::rpc::{client::docs, proto as docs_proto};
use n0_future::boxed::BoxStream;
use quic_rpc::transport::flume::FlumeConnector;

pub type ChatReceiver = BoxStream<anyhow::Result<Event>>;

pub type BlobsRPCConnector = FlumeConnector<blobs_proto::Response, blobs_proto::Request>;

pub type DocsRPCConnector = FlumeConnector<docs_proto::Response, docs_proto::Request>;

pub type BlobsClient = blobs::Client<BlobsRPCConnector>;

pub type DocsClient = docs::Client<DocsRPCConnector>;

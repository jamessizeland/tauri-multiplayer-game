pub(super) mod event;
pub(super) mod message;
pub mod peers;
pub(super) mod sender;
pub(super) mod ticket;

use event::Event;

use n0_future::boxed::BoxStream;

pub type ChatReceiver = BoxStream<anyhow::Result<Event>>;

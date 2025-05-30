use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::gossip::{
    message::{Message, SignedMessage},
    ChatReceiver, ChatSender, ChatTicket, Event, GossipNode, TicketOpts,
};
pub use iroh::NodeId;
pub use iroh_gossip::proto::TopicId;
use n0_future::{
    boxed::BoxStream,
    task::{self, AbortOnDropHandle},
    StreamExt as _,
};
use tokio::sync::Notify;
use tracing::{debug, info, warn};

const PRESENCE_INTERVAL: Duration = Duration::from_secs(5);

pub struct GossipChannel {
    topic_id: TopicId,
    me: NodeId,
    bootstrap: BTreeSet<NodeId>,
    neighbors: Arc<Mutex<BTreeSet<NodeId>>>,
    sender: ChatSender,
    receiver: Option<ChatReceiver>,
}

impl GossipChannel {
    pub fn sender(&self) -> ChatSender {
        self.sender.clone()
    }

    pub fn take_receiver(&mut self) -> Option<ChatReceiver> {
        self.receiver.take()
    }

    pub fn ticket(&self, opts: TicketOpts) -> anyhow::Result<String> {
        let mut ticket = ChatTicket::new(self.topic_id);
        if opts.include_myself {
            ticket.bootstrap.insert(self.me);
        }
        if opts.include_bootstrap {
            ticket.bootstrap.extend(self.bootstrap.iter().copied());
        }
        if opts.include_neighbors {
            let neighbors = self.neighbors.lock().unwrap();
            ticket.bootstrap.extend(neighbors.iter().copied())
        }
        tracing::info!("opts {:?} ticket {:?}", opts, ticket);
        Ok(ticket.serialize())
    }

    pub fn id(&self) -> String {
        self.topic_id.to_string()
    }

    #[allow(unused)]
    pub fn neighbors(&self) -> Vec<String> {
        self.neighbors
            .lock()
            .unwrap()
            .iter()
            .map(|x| x.to_string())
            .collect()
    }
}

impl GossipNode {
    /// Create and initialize the channel used for communicating between nodes.
    pub fn generate_channel(
        &self,
        ticket: ChatTicket,
        nickname: &str,
    ) -> anyhow::Result<GossipChannel> {
        let (sender, receiver) = self.join(&ticket, nickname.to_owned())?;
        let neighbors = Arc::new(Mutex::new(BTreeSet::new()));
        let receiver_stream = build_receiver_stream(receiver, neighbors.clone());

        // Add ourselves to the ticket.
        let mut ticket = ticket;
        ticket.bootstrap.insert(self.node_id());

        let topic = GossipChannel {
            topic_id: ticket.topic_id,
            bootstrap: ticket.bootstrap,
            neighbors,
            me: self.node_id(),
            sender,
            receiver: Some(receiver_stream),
        };
        Ok(topic)
    }

    /// Joins a chat channel from a ticket.
    ///
    /// Returns a [`ChatSender`] to send messages or change our nickname
    /// and a stream of [`Event`] items for incoming messages and other event.s
    fn join(
        &self,
        ticket: &ChatTicket,
        nickname: String,
    ) -> anyhow::Result<(ChatSender, ChatReceiver)> {
        let topic_id = ticket.topic_id;
        let bootstrap = ticket.bootstrap.iter().cloned().collect();
        info!(?bootstrap, "joining {topic_id}");
        let gossip_topic = self.gossip.subscribe(topic_id, bootstrap)?;
        let (sender, receiver) = gossip_topic.split();

        let nickname = Arc::new(Mutex::new(nickname));
        let trigger_presence = Arc::new(Notify::new());

        // We spawn a task that occasionally sends a Presence message with our nickname.
        // This allows to track which peers are online currently.
        let presence_task = AbortOnDropHandle::new(task::spawn({
            let secret_key = self.secret_key.clone();
            let sender = sender.clone();
            let trigger_presence = trigger_presence.clone();
            let nickname = nickname.clone();

            async move {
                loop {
                    let nickname = nickname.lock().expect("poisened").clone();
                    let message = Message::Presence { nickname };
                    debug!("send presence {message:?}");
                    let signed_message = SignedMessage::sign_and_encode(&secret_key, message)
                        .expect("failed to encode message");
                    if let Err(err) = sender.broadcast(signed_message.into()).await {
                        tracing::warn!("presence task failed to broadcast: {err}");
                        break;
                    }
                    n0_future::future::race(
                        n0_future::time::sleep(PRESENCE_INTERVAL),
                        trigger_presence.notified(),
                    )
                    .await;
                }
            }
        }));

        // We create a stream of events, coming from the gossip topic event receiver.
        // We'll want to map the events to our own event type, which includes parsing
        // the messages and verifying the signatures, and trigger presence
        // once the swarm is joined initially.
        let receiver = n0_future::stream::try_unfold(receiver, {
            let trigger_presence = trigger_presence.clone();
            move |mut receiver| {
                let trigger_presence = trigger_presence.clone();
                async move {
                    loop {
                        // Store if we were joined before the next event comes in.
                        let was_joined = receiver.is_joined();

                        // Fetch the next event.
                        let Some(event) = receiver.try_next().await? else {
                            return Ok(None);
                        };
                        // Convert into our event type. this fails if we receive a message
                        // that cannot be decoced into our event type. If that is the case,
                        // we just keep and log the error.
                        let event: Event = match event.try_into() {
                            Ok(event) => event,
                            Err(err) => {
                                warn!("received invalid message: {err}");
                                continue;
                            }
                        };
                        // If we just joined, trigger sending our presence message.
                        if !was_joined && receiver.is_joined() {
                            trigger_presence.notify_waiters()
                        };

                        break Ok(Some((event, receiver)));
                    }
                }
            }
        });

        let sender = ChatSender::new(
            nickname,
            self.secret_key.clone(),
            sender,
            trigger_presence,
            presence_task,
        );
        Ok((sender, Box::pin(receiver)))
    }
}

/// Augment the raw ChatReceiver stream with additional logic to manage the list of neighbors.
fn build_receiver_stream(
    receiver: ChatReceiver,
    neighbors: Arc<Mutex<BTreeSet<NodeId>>>,
) -> BoxStream<anyhow::Result<Event>> {
    let receiver = receiver.map(move |event| {
        let mut nodes = neighbors.lock().unwrap();
        match &event {
            Ok(Event::Joined { neighbors }) => {
                nodes.extend(neighbors);
            }
            Ok(Event::NeighborUp { node_id }) => {
                nodes.insert(*node_id);
            }
            Ok(Event::NeighborDown { node_id }) => {
                nodes.remove(node_id);
            }
            _ => {}
        }
        event
    });
    Box::pin(receiver)
}

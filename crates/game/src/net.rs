use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use n0_future::{StreamExt, time::Duration};
use networking::{EndpointId, GameInviter, GameTicket, TopicId};
use serde::{Deserialize, Serialize};
use tracing::level_filters::LevelFilter;
use tracing_subscriber_wasm::MakeConsoleWriter;
use wasm_bindgen::{JsError, JsValue, prelude::wasm_bindgen};
use wasm_streams::ReadableStream;

#[wasm_bindgen(start)]
fn start() {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .with_writer(
            // To avoid trace events in the browser from showing their JS backtrace
            MakeConsoleWriter::default().map_trace_level_to(tracing::Level::DEBUG),
        )
        // If we don't do this in the browser, we get a runtime error.
        .without_time()
        .with_ansi(false)
        .init();

    tracing::info!("(testing logging) Logging setup");
}

#[wasm_bindgen]
pub struct NetworkNode(networking::Node);

#[wasm_bindgen]
impl NetworkNode {
    /// Spawns a gossip node.
    pub async fn spawn() -> Result<Self, JsError> {
        let inner = networking::Node::spawn(None).await.map_err(to_js_err)?;
        Ok(Self(inner))
    }

    #[wasm_bindgen(js_name = "endpointId")]
    /// Returns the endpoint id of this node.
    pub fn endpoint_id(&self) -> String {
        self.0.endpoint_id().to_string()
    }

    pub async fn create(&self, nickname: String) -> Result<Channel, JsError> {
        let ticket = GameTicket::new_random();
        self.join_inner(ticket, nickname).await
    }

    pub async fn join(&self, ticket: String, nickname: String) -> Result<Channel, JsError> {
        let ticket = GameTicket::deserialize(&ticket).map_err(to_js_err)?;
        self.join_inner(ticket, nickname).await
    }

    async fn join_inner(&self, ticket: GameTicket, nickname: String) -> Result<Channel, JsError> {
        let (sender, receiver) = self.0.join(&ticket, nickname).await.map_err(to_js_err)?;
        let sender = ChannelSender(sender);
        let neighbors = Arc::new(Mutex::new(BTreeSet::new()));
        let neighbors2 = neighbors.clone();
        let receiver = receiver.map(move |event| {
            if let Ok(event) = &event {
                match event {
                    networking::Event::Joined { neighbors } => {
                        neighbors2.lock().unwrap().extend(neighbors.iter().cloned());
                    }
                    networking::Event::NeighborUp { endpoint_id } => {
                        neighbors2.lock().unwrap().insert(*endpoint_id);
                    }
                    networking::Event::NeighborDown { endpoint_id } => {
                        neighbors2.lock().unwrap().remove(endpoint_id);
                    }
                    _ => {}
                }
            }
            event
                .map_err(|err| JsValue::from(&err.to_string()))
                .map(|event| serde_wasm_bindgen::to_value(&event).unwrap())
        });
        let receiver = ReadableStream::from_stream(receiver).into_raw();

        // Add ourselves to the ticket.
        let mut ticket = ticket;
        ticket.bootstrap.insert(self.0.endpoint_id());
        // ticket.bootstrap = [self.0.endpoint_id()].into_iter().collect();

        let topic = Channel {
            topic_id: ticket.topic_id,
            bootstrap: ticket.bootstrap,
            neighbors,
            me: self.0.endpoint_id(),
            sender,
            receiver,
        };
        Ok(topic)
    }
}

type ChannelReceiver = wasm_streams::readable::sys::ReadableStream;

#[wasm_bindgen]
pub struct Channel {
    topic_id: TopicId,
    me: EndpointId,
    bootstrap: BTreeSet<EndpointId>,
    neighbors: Arc<Mutex<BTreeSet<EndpointId>>>,
    sender: ChannelSender,
    receiver: ChannelReceiver,
}

#[wasm_bindgen]
impl Channel {
    pub(crate) fn ticket_with_game_options(
        &self,
        opts: TicketOpts,
        game_options_json: Option<String>,
    ) -> Result<String, JsError> {
        let mut ticket = GameTicket::new(self.topic_id);
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
        ticket.game_options_json = game_options_json;
        tracing::info!("ticket generated for topic {}", ticket.topic_id);
        Ok(ticket.serialize())
    }

    #[wasm_bindgen(getter)]
    pub fn sender(&self) -> ChannelSender {
        self.sender.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn receiver(&mut self) -> ChannelReceiver {
        self.receiver.clone()
    }

    pub fn ticket(&self, opts: JsValue) -> Result<String, JsError> {
        let opts: TicketOpts = serde_wasm_bindgen::from_value(opts)?;
        self.ticket_with_game_options(opts, None)
    }

    pub fn id(&self) -> String {
        self.topic_id.to_string()
    }

    pub fn neighbors(&self) -> Vec<String> {
        self.neighbors
            .lock()
            .unwrap()
            .iter()
            .map(|x| x.to_string())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub endpoint_id: EndpointId,
    pub nickname: String,
    pub last_active: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketOpts {
    pub include_myself: bool,
    pub include_bootstrap: bool,
    pub include_neighbors: bool,
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct ChannelSender(GameInviter);

#[wasm_bindgen]
impl ChannelSender {
    pub async fn broadcast(&self, text: String) -> Result<(), JsError> {
        self.0.send(text).await.map_err(to_js_err)?;
        Ok(())
    }

    pub fn set_nickame(&self, nickname: String) {
        self.0.set_nickname(nickname);
    }
}

fn to_js_err(err: impl Into<anyhow::Error>) -> JsError {
    let err: anyhow::Error = err.into();
    JsError::new(&err.to_string())
}

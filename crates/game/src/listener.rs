use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    rc::Rc,
};

use futures_util::StreamExt;
use js_sys::Array;
use js_sys::Date;
use serde::Deserialize;
use svelte_store::Readable;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

use crate::UiState;
use crate::mutation::{Mutation, MutationOrigin, apply_mutation_with_effects};
use crate::{NetworkPeerStatus, NetworkSnapshot, PeerPresenceEntry};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum NetworkEvent {
    #[serde(rename_all = "camelCase")]
    Joined {
        neighbors: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    MessageReceived {
        from: String,
        text: String,
        nickname: String,
        #[allow(dead_code)]
        #[serde(default)]
        sent_timestamp: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Presence {
        from: String,
        nickname: String,
        #[allow(dead_code)]
        #[serde(default)]
        sent_timestamp: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    NeighborUp {
        endpoint_id: String,
    },
    #[serde(rename_all = "camelCase")]
    NeighborDown {
        endpoint_id: String,
    },
    Lagged,
}

pub(crate) fn spawn_network_listener(
    receiver: wasm_streams::readable::sys::ReadableStream,
    cell_metadata: Rc<RefCell<Readable<Array>>>,
    score: Rc<RefCell<Readable<crate::ScoreState>>>,
    ui_state: UiState,
    connected_endpoints: Rc<RefCell<BTreeSet<String>>>,
    peer_presence: Rc<RefCell<HashMap<String, PeerPresenceEntry>>>,
    network_snapshot: Rc<RefCell<Readable<NetworkSnapshot>>>,
    last_inbound_mutation_ms: Rc<RefCell<Option<f64>>>,
    last_outbound_mutation_ms: Rc<RefCell<Option<f64>>>,
    local_endpoint_id: Option<String>,
    topic_id: Option<String>,
) {
    spawn_local(async move {
        let mut stream = wasm_streams::ReadableStream::from_raw(receiver).into_stream();
        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    let now_ms = Date::now();
                    let parsed_event: NetworkEvent =
                        match serde_wasm_bindgen::from_value(event.clone()) {
                            Ok(value) => value,
                            Err(err) => {
                                tracing::warn!(
                                    "failed to deserialize network event from JS value: {:?}",
                                    err
                                );
                                continue;
                            }
                        };

                    match parsed_event {
                        NetworkEvent::Joined { neighbors } => {
                            let mut connected_endpoints_ref = connected_endpoints.borrow_mut();
                            connected_endpoints_ref.clear();
                            connected_endpoints_ref.extend(neighbors.iter().cloned());

                            for endpoint_id in neighbors {
                                upsert_peer_presence(&peer_presence, &endpoint_id, None, None);
                            }
                        }
                        NetworkEvent::NeighborUp { endpoint_id } => {
                            connected_endpoints.borrow_mut().insert(endpoint_id.clone());
                            upsert_peer_presence(&peer_presence, &endpoint_id, None, None);
                        }
                        NetworkEvent::NeighborDown { endpoint_id } => {
                            connected_endpoints.borrow_mut().remove(&endpoint_id);
                            upsert_peer_presence(&peer_presence, &endpoint_id, None, None);
                        }
                        NetworkEvent::Presence { from, nickname, .. } => {
                            let is_local_endpoint = local_endpoint_id
                                .as_deref()
                                .is_some_and(|local| local == from);
                            if !is_local_endpoint {
                                connected_endpoints.borrow_mut().insert(from.clone());
                                upsert_peer_presence(
                                    &peer_presence,
                                    &from,
                                    Some(nickname),
                                    Some(now_ms),
                                );
                            }
                        }
                        NetworkEvent::MessageReceived {
                            from,
                            text,
                            nickname,
                            ..
                        } => {
                            let is_local_endpoint = local_endpoint_id
                                .as_deref()
                                .is_some_and(|local| local == from);

                            if !is_local_endpoint {
                                connected_endpoints.borrow_mut().insert(from.clone());
                                upsert_peer_presence(
                                    &peer_presence,
                                    &from,
                                    Some(nickname),
                                    Some(now_ms),
                                );
                            }

                            if is_local_endpoint {
                                continue;
                            }

                            if let Err(err) = apply_incoming_mutation(
                                &cell_metadata,
                                &score,
                                &ui_state,
                                MutationOrigin::Peer,
                                &text,
                            ) {
                                tracing::warn!(
                                    "failed to apply state mutation from peer: {:?}",
                                    err
                                );
                            } else {
                                *last_inbound_mutation_ms.borrow_mut() = Some(now_ms);
                            }
                        }
                        NetworkEvent::Lagged => {
                            tracing::warn!(
                                "network stream lagged; some events may have been dropped"
                            );
                        }
                    }

                    publish_network_snapshot(
                        &network_snapshot,
                        &connected_endpoints,
                        &peer_presence,
                        &last_inbound_mutation_ms,
                        &last_outbound_mutation_ms,
                        &local_endpoint_id,
                        &topic_id,
                    );
                }
                Err(err) => {
                    tracing::warn!("network event stream error: {:?}", err);
                    break;
                }
            }
        }
    });
}

fn upsert_peer_presence(
    peer_presence: &Rc<RefCell<HashMap<String, PeerPresenceEntry>>>,
    endpoint_id: &str,
    nickname: Option<String>,
    last_seen_ms: Option<f64>,
) {
    let mut peer_presence = peer_presence.borrow_mut();
    let peer_entry = peer_presence.entry(endpoint_id.to_string()).or_default();

    if let Some(nickname) = nickname {
        peer_entry.nickname = Some(nickname);
    }

    if let Some(last_seen_ms) = last_seen_ms {
        peer_entry.last_seen_ms = Some(last_seen_ms);
    }
}

fn publish_network_snapshot(
    network_snapshot_store: &Rc<RefCell<Readable<NetworkSnapshot>>>,
    connected_endpoints: &Rc<RefCell<BTreeSet<String>>>,
    peer_presence: &Rc<RefCell<HashMap<String, PeerPresenceEntry>>>,
    last_inbound_mutation_ms: &Rc<RefCell<Option<f64>>>,
    last_outbound_mutation_ms: &Rc<RefCell<Option<f64>>>,
    local_endpoint_id: &Option<String>,
    topic_id: &Option<String>,
) {
    let connected_endpoints_snapshot = connected_endpoints.borrow().clone();
    let mut all_endpoints = connected_endpoints_snapshot.clone();
    all_endpoints.extend(peer_presence.borrow().keys().cloned());

    let peer_presence = peer_presence.borrow();
    let peers: Vec<NetworkPeerStatus> = all_endpoints
        .into_iter()
        .map(|endpoint_id| {
            let peer_entry = peer_presence.get(&endpoint_id);
            NetworkPeerStatus::new(
                endpoint_id.clone(),
                peer_entry.and_then(|entry| entry.nickname.clone()),
                peer_entry.and_then(|entry| entry.last_seen_ms),
                connected_endpoints_snapshot.contains(&endpoint_id),
            )
        })
        .collect();

    let network_snapshot = NetworkSnapshot {
        has_node: local_endpoint_id.is_some(),
        listener_started: true,
        endpoint_id: local_endpoint_id.clone(),
        topic_id: topic_id.clone(),
        peers,
        last_inbound_mutation_ms: *last_inbound_mutation_ms.borrow(),
        last_outbound_mutation_ms: *last_outbound_mutation_ms.borrow(),
        sampled_at_ms: Date::now(),
    };
    network_snapshot_store.borrow_mut().set(network_snapshot);
}

fn apply_incoming_mutation(
    cell_metadata: &Rc<RefCell<Readable<Array>>>,
    score: &Rc<RefCell<Readable<crate::ScoreState>>>,
    ui_state: &UiState,
    origin: MutationOrigin,
    message_text: &str,
) -> Result<(), JsValue> {
    let Some(mutation) = Mutation::decode(message_text) else {
        return Ok(());
    };

    apply_mutation_with_effects(cell_metadata, score, ui_state, mutation, origin)
}

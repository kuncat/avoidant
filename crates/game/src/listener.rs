use std::{cell::RefCell, rc::Rc};

use futures_util::StreamExt;
use js_sys::{Array, Reflect};
use svelte_store::Readable;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

use crate::{
    MESSAGE_RECEIVED_EVENT, Mutation, MutationOrigin, UiState, apply_mutation_with_effects,
};

struct NetworkTextMessage {
    from: Option<String>,
    text: String,
}

pub(crate) fn spawn_network_listener(
    receiver: wasm_streams::readable::sys::ReadableStream,
    cells: Rc<RefCell<Readable<Array>>>,
    ui_state: UiState,
    local_endpoint_id: Option<String>,
) {
    spawn_local(async move {
        let mut stream = wasm_streams::ReadableStream::from_raw(receiver).into_stream();
        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    if let Some(message) = extract_message_text(&event) {
                        if message
                            .from
                            .as_deref()
                            .zip(local_endpoint_id.as_deref())
                            .is_some_and(|(from, local)| from == local)
                        {
                            continue;
                        }

                        if let Err(err) = apply_incoming_mutation(
                            &cells,
                            &ui_state,
                            MutationOrigin::Peer,
                            &message.text,
                        ) {
                            tracing::warn!("failed to apply state mutation from peer: {:?}", err);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("network event stream error: {:?}", err);
                    break;
                }
            }
        }
    });
}

fn extract_message_text(event: &JsValue) -> Option<NetworkTextMessage> {
    let event_type = Reflect::get(event, &JsValue::from_str("type"))
        .ok()?
        .as_string()?;
    if event_type != MESSAGE_RECEIVED_EVENT {
        return None;
    }

    let text = Reflect::get(event, &JsValue::from_str("text"))
        .ok()?
        .as_string()?;
    let from = Reflect::get(event, &JsValue::from_str("from"))
        .ok()
        .and_then(|value| value.as_string());

    Some(NetworkTextMessage { from, text })
}

fn apply_incoming_mutation(
    cells: &Rc<RefCell<Readable<Array>>>,
    ui_state: &UiState,
    origin: MutationOrigin,
    message_text: &str,
) -> Result<(), JsValue> {
    let Some(mutation) = Mutation::decode(message_text) else {
        return Ok(());
    };

    apply_mutation_with_effects(cells, ui_state, mutation, origin)
}

use crate::mutation::{Mutation, MutationOrigin, animate_reveal};
use crate::net::ChannelSender;
use crate::sync::{Message, PROTOCOL, Request, Session};
use crate::{CellMetadataEntry, ScoreState, UiState};
use js_sys::Array;
use std::{cell::RefCell, rc::Rc};
use svelte_store::Readable;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

#[derive(Clone)]
pub(crate) struct SyncBridge {
    pub session: Rc<RefCell<Session>>,
    pub cells: Rc<RefCell<Readable<Array>>>,
    pub metadata: Rc<RefCell<Readable<Array>>>,
    pub score: Rc<RefCell<Readable<ScoreState>>>,
    pub ui: UiState,
    pub sender: Option<ChannelSender>,
}

impl SyncBridge {
    fn send(&self, message: Message) {
        let Some(sender) = self.sender.clone() else {
            return;
        };
        let Ok(text) = serde_json::to_string(&message) else {
            return;
        };
        spawn_local(async move {
            if let Err(err) = sender.broadcast(text).await {
                tracing::warn!("state synchronization send failed (will retry): {:?}", err);
            }
        });
    }

    fn send_state(&self, receipt: Option<(String, u64)>, movement: Option<Request>) {
        self.send(Message::State {
            version: PROTOCOL,
            snapshot: self.session.borrow().snapshot(),
            receipt,
            movement,
        });
    }

    pub fn tick(&self) {
        let session = self.session.borrow();
        if session.stopped {
            return;
        }
        if session.is_host() {
            drop(session);
            self.send_state(None, None);
        } else {
            let pending: Vec<_> = session.pending.values().cloned().collect();
            drop(session);
            self.send(Message::Sync { version: PROTOCOL });
            for request in pending {
                self.send(Message::Request {
                    version: PROTOCOL,
                    request,
                });
            }
        }
    }

    pub fn local(&self, index: usize, position: [f64; 3]) -> Result<(), JsValue> {
        let request = self.session.borrow_mut().request(index, position);
        let Some(request) = request else {
            return Ok(());
        };
        if self.session.borrow().is_host() {
            let actor = self
                .session
                .borrow()
                .local_id
                .clone()
                .unwrap_or_else(|| "offline".into());
            self.session.borrow_mut().apply(&actor, &request);
            self.present(Some(&request), MutationOrigin::Local)?;
            self.send_state(None, Some(request));
        } else {
            self.send(Message::Request {
                version: PROTOCOL,
                request,
            });
        }
        Ok(())
    }

    pub fn receive(&self, from: &str, text: &str) -> Result<(), JsValue> {
        let Ok(message) = serde_json::from_str::<Message>(text) else {
            return Ok(());
        };
        match message {
            Message::Request {
                version: PROTOCOL,
                request,
            } if self.session.borrow().is_host() => {
                let changed = self.session.borrow_mut().apply(from, &request);
                if changed {
                    self.present(Some(&request), MutationOrigin::Peer)?;
                }
                // Duplicate requests still receive a receipt and the latest snapshot.
                self.send_state(Some((from.into(), request.id)), changed.then_some(request));
            }
            Message::Sync { version: PROTOCOL } if self.session.borrow().is_host() => {
                self.send_state(None, None)
            }
            Message::State {
                version: PROTOCOL,
                snapshot,
                receipt,
                movement,
            } => {
                let was_ready = self.session.borrow().ready;
                let local_move = receipt.as_ref().is_some_and(|(actor, _)| {
                    self.session.borrow().local_id.as_ref() == Some(actor)
                });
                if self.session.borrow_mut().accept(from, &snapshot, &receipt) {
                    self.present(
                        if was_ready { movement.as_ref() } else { None },
                        if local_move {
                            MutationOrigin::Local
                        } else {
                            MutationOrigin::Peer
                        },
                    )?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Publish canonical score immediately; animation only controls presentation flags.
    pub fn present(
        &self,
        movement: Option<&Request>,
        origin: MutationOrigin,
    ) -> Result<(), JsValue> {
        let (entries, score) = {
            let session = self.session.borrow();
            (session.metadata.clone(), session.score.clone())
        };
        let previous: Array = {
            let store = self.metadata.borrow();
            let array: &Array = &**store;
            array.clone()
        };
        let next = Array::new();
        let mut animate = vec![];
        for (index, mut cell) in entries.into_iter().enumerate() {
            let old =
                serde_wasm_bindgen::from_value::<CellMetadataEntry>(previous.get(index as u32))
                    .ok();
            if cell.is_explored {
                if let Some(old) = old.filter(|old| old.is_explored || old.is_revealing) {
                    cell.is_explored = old.is_explored;
                    cell.is_revealing = old.is_revealing;
                } else if movement.is_some() {
                    cell.is_explored = false;
                    animate.push(index);
                }
            }
            next.push(&serde_wasm_bindgen::to_value(&cell)?);
        }
        self.metadata.borrow_mut().set(next);
        self.score.borrow_mut().set(score);
        if let Some(request) = movement {
            animate_reveal(
                &self.cells,
                &self.metadata,
                &self.ui,
                Mutation::ExploreCell {
                    index: request.index,
                    pulse_position: request.position,
                },
                origin,
                animate,
            )?;
        }
        Ok(())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use crate::{GameOptions, GameState, MapShape};
    use wasm_bindgen_test::wasm_bindgen_test;

    fn game(peer: bool) -> GameState {
        let options: GameOptions = serde_wasm_bindgen::from_value(js_sys::JSON::parse(
            if peer {
                r#"{"numCells":80,"rngSeed":42,"firstSafeCell":0,"authority":"host","syncVersion":1}"#
            } else { r#"{"numCells":80,"rngSeed":42}"# }
        ).unwrap()).unwrap();
        let mut game = GameState::new(options).unwrap();
        let cells = crate::mapgen::generate_map(80, 42, &MapShape::Icosahedron { radius: 50.0 })
            .unwrap()
            .cells;
        game.apply_map_cells(serde_wasm_bindgen::to_value(&cells).unwrap())
            .unwrap();
        game.session.borrow_mut().local_id = Some(if peer { "peer" } else { "host" }.into());
        game
    }

    fn transmit(
        host: &SyncBridge,
        peer: &SyncBridge,
        receipt: Option<(String, u64)>,
        movement: Option<Request>,
    ) {
        let text = serde_json::to_string(&Message::State {
            version: PROTOCOL,
            snapshot: host.session.borrow().snapshot(),
            receipt,
            movement,
        })
        .unwrap();
        peer.receive("host", &text).unwrap();
    }

    #[wasm_bindgen_test]
    async fn late_join_and_concurrent_retries_preserve_score_through_animation_cleanup() {
        let host = game(false);
        host.queue_explore_pulse(0, 0.0, 0.0, 0.0).unwrap();
        assert!(host.session.borrow().score.safe_explored > 0);
        let peer = game(true);
        let h = host.sync_bridge();
        let p = peer.sync_bridge();
        // Join while the host's opening animation is still in flight.
        transmit(&h, &p, None, None);
        assert!(peer.session.borrow().ready);
        let index = host
            .session
            .borrow()
            .metadata
            .iter()
            .position(|c| !c.is_explored)
            .unwrap();
        p.local(index, [0.0; 3]).unwrap();
        let request = peer
            .session
            .borrow()
            .pending
            .values()
            .next()
            .unwrap()
            .clone();
        let wire = serde_json::to_string(&Message::Request {
            version: PROTOCOL,
            request: request.clone(),
        })
        .unwrap();
        h.receive("peer", &wire).unwrap();
        h.receive("peer", &wire).unwrap();
        // A racing host click on the same cell is also a no-op.
        h.local(index, [0.0; 3]).unwrap();
        transmit(&h, &p, Some(("peer".into(), request.id)), Some(request));
        assert!(peer.session.borrow().pending.is_empty());
        let expected = serde_json::to_value(&host.session.borrow().score).unwrap();
        n0_future::time::sleep(n0_future::time::Duration::from_millis(1500)).await;
        let host_score: ScoreState = {
            let store = host.score.borrow();
            (**store).clone()
        };
        let peer_score: ScoreState = {
            let store = peer.score.borrow();
            (**store).clone()
        };
        assert_eq!(serde_json::to_value(host_score).unwrap(), expected);
        assert_eq!(serde_json::to_value(peer_score).unwrap(), expected);
        for game in [&host, &peer] {
            let store = game.cell_metadata.borrow();
            let array: &Array = &**store;
            let display: Vec<CellMetadataEntry> =
                serde_wasm_bindgen::from_value(array.clone().into()).unwrap();
            assert_eq!(
                display.iter().map(|c| c.is_explored).collect::<Vec<_>>(),
                game.session.borrow().snapshot().explored
            );
        }
    }
}

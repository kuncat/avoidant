//! Regression tests for fallible, typed JavaScript/WASM boundaries.
#![cfg(target_arch = "wasm32")]

use avoidant::{GameOptions, GameState};
use tsify::Ts;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn invalid_options_return_errors_without_poisoning_subsequent_calls() {
    for value in [
        JsValue::NULL,
        JsValue::UNDEFINED,
        JsValue::from_str("invalid"),
    ]
    .into_iter()
    .cycle()
    .take(100)
    {
        assert!(GameState::new(Ts::<GameOptions>::new_unchecked(value)).is_err());
    }
    let options = js_sys::JSON::parse(r#"{"numCells":80,"rngSeed":42}"#).unwrap();
    let game = GameState::new(Ts::new_unchecked(options)).unwrap();
    assert!(game.network_peers().unwrap().is_empty());
    assert!(!game.has_network_node());
}

#[wasm_bindgen_test]
fn incomplete_options_and_invalid_tickets_return_errors() {
    let options = js_sys::JSON::parse(r#"{"numCells":80}"#).unwrap();
    assert!(GameState::new(Ts::new_unchecked(options)).is_err());
    assert!(GameState::options_from_ticket("invalid".into()).is_err());
}

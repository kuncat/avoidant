mod state_bridge;
mod utils;

use crate::state_bridge::{call_method1, StateCell};
use js_sys::{Object, Set};
use svelte_state_macros::SvelteStateModel;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, back!");
}

#[wasm_bindgen]
#[derive(SvelteStateModel)]
pub struct CounterState {
    #[svelte_state(state_object)]
    state_object: Option<Object>,

    #[svelte_state(key = "counter", default = 0.0)]
    counter: StateCell<f64>,

    #[svelte_state(key = "counter2", default = 0.0)]
    counter2: StateCell<f64>,

    #[svelte_state(key = "set", default = js_sys::Set::new(&JsValue::UNDEFINED))]
    set: StateCell<Set>,

    #[svelte_state(default)]
    local_only_counter: RefCell<i64>,
}

#[wasm_bindgen]
impl CounterState {
    #[wasm_bindgen(constructor)]
    pub fn new(state_object: Option<JsValue>) -> Result<CounterState, JsValue> {
        let state_object = match state_object {
            Some(value) => Some(value.dyn_into::<Object>().map_err(|_| {
                JsValue::from_str("CounterState constructor expects a plain object when provided")
            })?),
            None => None,
        };

        CounterState::build_from_state_object(state_object)
    }

    #[wasm_bindgen(getter, js_name = stateObject)]
    pub fn state_object(&self) -> JsValue {
        self.state_object
            .as_ref()
            .map(|value| value.clone().into())
            .unwrap_or(JsValue::UNDEFINED)
    }

    #[wasm_bindgen(getter)]
    pub fn counter(&self) -> Result<f64, JsValue> {
        self.counter.get()
    }

    #[wasm_bindgen(getter)]
    pub fn counter2(&self) -> Result<f64, JsValue> {
        self.counter2.get()
    }

    pub fn add_to_counter(&self, amount: f64) -> Result<f64, JsValue> {
        self.counter.update(|value| value + amount)
    }

    pub fn add_to_counter2(&self, amount: f64) -> Result<f64, JsValue> {
        self.counter2.update(|value| value + amount)
    }

    pub fn add_to_set(&self, value: f64) -> Result<u32, JsValue> {
        let set = self.set.get()?;
        let set_value: JsValue = set.clone().into();
        call_method1(&set_value, "add", &JsValue::from_f64(value))?;
        self.set.set(set.clone())?;
        Ok(set.size())
    }

    pub fn add_to_local_only_counter(&self, amount: i64) -> i64 {
        let mut current = self.local_only_counter.borrow_mut();
        *current += amount;
        *current
    }
}

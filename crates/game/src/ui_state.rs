use std::{cell::RefCell, rc::Rc};

use js_sys::{Array, Reflect};
use n0_future::time::Duration;
use svelte_store::Readable;
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
use wasm_bindgen_futures::spawn_local;

use crate::{Pulse, Pulses};

#[derive(Clone)]
#[wasm_bindgen]
pub struct UiState {
    pulses: Rc<RefCell<Readable<Array>>>,
    next_pulse_id: Rc<RefCell<u32>>,
}

#[wasm_bindgen]
impl UiState {
    #[wasm_bindgen(getter, js_name = pulses)]
    pub fn pulses_store(&self) -> Pulses {
        self.pulses.borrow().get_store().into()
    }

    #[wasm_bindgen(js_name = "addPulse")]
    pub fn add_pulse(
        &self,
        origin_cell: usize,
        x: f64,
        y: f64,
        z: f64,
        duration_ms: u32,
    ) -> Result<u32, JsValue> {
        self.add_pulse_internal(origin_cell, x, y, z, duration_ms, false)
    }
}

impl UiState {
    pub(crate) fn new() -> Self {
        Self {
            pulses: Rc::new(RefCell::new(Readable::new(Array::new()))),
            next_pulse_id: Rc::new(RefCell::new(0)),
        }
    }

    pub(crate) fn add_pulse_internal(
        &self,
        origin_cell: usize,
        x: f64,
        y: f64,
        z: f64,
        duration_ms: u32,
        is_remote: bool,
    ) -> Result<u32, JsValue> {
        let mut next_pulse_id = self.next_pulse_id.borrow_mut();
        let pulse_id = *next_pulse_id;
        *next_pulse_id = next_pulse_id.wrapping_add(1);

        let created_at_ms = monotonic_now_ms();
        let pulse = Pulse::new(
            pulse_id,
            origin_cell,
            [x, y, z],
            created_at_ms,
            duration_ms,
            is_remote,
        );
        let pulse: JsValue = pulse.into();
        self.pulses.borrow_mut().set_with(|pulses_array| {
            pulses_array.push(pulse.as_ref());
        });

        let ui_state = self.clone();
        spawn_local(async move {
            n0_future::time::sleep(Duration::from_millis(duration_ms as u64)).await;
            if let Err(err) = ui_state.remove_pulse_by_id(pulse_id) {
                tracing::warn!("failed to remove expired pulse: {:?}", err);
            }
        });

        Ok(pulse_id)
    }

    fn remove_pulse_by_id(&self, pulse_id: u32) -> Result<(), JsValue> {
        self.pulses.borrow_mut().set_with(|pulses_array| {
            let filtered = Array::new();
            for idx in 0..pulses_array.length() {
                let pulse = pulses_array.get(idx);
                let id = Reflect::get(&pulse, &JsValue::from_str("id"))?
                    .as_f64()
                    .unwrap_or(-1.0) as u32;
                if id != pulse_id {
                    filtered.push(&pulse);
                }
            }
            *pulses_array = filtered;
            Ok::<(), JsValue>(())
        })
    }
}

fn monotonic_now_ms() -> f64 {
    let global = js_sys::global();
    let performance = Reflect::get(&global, &JsValue::from_str("performance"))
        .ok()
        .filter(|value| !value.is_null() && !value.is_undefined());

    if let Some(performance) = performance {
        let now_fn = Reflect::get(&performance, &JsValue::from_str("now"))
            .ok()
            .and_then(|value| value.dyn_into::<js_sys::Function>().ok());
        if let Some(now_fn) = now_fn {
            if let Ok(result) = now_fn.call0(&performance) {
                if let Some(ms) = result.as_f64() {
                    return ms;
                }
            }
        }
    }

    js_sys::Date::now()
}

use std::{cell::RefCell, rc::Rc};

use js_sys::{Array, Reflect};
use svelte_store::Readable;
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};

use crate::{Pulse, Pulses};

#[derive(Clone)]
#[wasm_bindgen]
pub struct UiState {
    pulses_store: Rc<RefCell<Readable<Array>>>,
    pulses: Rc<RefCell<Vec<Pulse>>>,
    next_pulse_id: Rc<RefCell<u32>>,
}

#[wasm_bindgen]
impl UiState {
    #[wasm_bindgen(getter, js_name = pulses)]
    pub fn pulses_store(&self) -> Pulses {
        self.pulses_store.borrow().get_store().into()
    }
}

impl UiState {
    pub(crate) fn new() -> Self {
        Self {
            pulses_store: Rc::new(RefCell::new(Readable::new(Array::new()))),
            pulses: Rc::new(RefCell::new(Vec::new())),
            next_pulse_id: Rc::new(RefCell::new(0)),
        }
    }

    /// Add a pulse to the store without scheduling its removal. Callers must call [`UiState::remove_pulse_by_id`] when the pulse should disappear. This lets the chord auto-reveal atomically clean up cell metadata *before* removing the pulse, which would otherwise race with the metadata flip and leave one frame with `is_revealing=1` but no active pulse driving the sweep.
    pub(crate) fn add_pulse_internal(
        &self,
        origin_cell: usize,
        x: f64,
        y: f64,
        z: f64,
        duration_ms: u32,
        is_remote: bool,
        max_radius: f64,
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
            max_radius,
        );
        self.pulses.borrow_mut().push(pulse);
        self.sync_pulses_store();

        Ok(pulse_id)
    }

    pub(crate) fn remove_pulse_by_id(&self, pulse_id: u32) -> Result<(), JsValue> {
        self.pulses
            .borrow_mut()
            .retain(|pulse| pulse.id() != pulse_id);
        self.sync_pulses_store();
        Ok(())
    }

    fn sync_pulses_store(&self) {
        let pulses = self.pulses.borrow();
        self.pulses_store.borrow_mut().set_with(|pulses_array| {
            let next = Array::new();
            for pulse in pulses.iter() {
                let pulse: JsValue = pulse.clone().into();
                next.push(pulse.as_ref());
            }
            *pulses_array = next;
        });
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

use js_sys::{Function, Object, Reflect, Set};
use std::any::type_name;
use std::cell::RefCell;
use std::marker::PhantomData;
use wasm_bindgen::{JsCast, JsValue};

pub(super) trait ManagedValue: Clone {
    fn try_from_js(value: &JsValue) -> Option<Self>;
    fn to_js_value(&self) -> JsValue;
}

impl ManagedValue for f64 {
    fn try_from_js(value: &JsValue) -> Option<Self> {
        value.as_f64()
    }

    fn to_js_value(&self) -> JsValue {
        JsValue::from_f64(*self)
    }
}

impl ManagedValue for bool {
    fn try_from_js(value: &JsValue) -> Option<Self> {
        value.as_bool()
    }

    fn to_js_value(&self) -> JsValue {
        JsValue::from_bool(*self)
    }
}

impl ManagedValue for String {
    fn try_from_js(value: &JsValue) -> Option<Self> {
        value.as_string()
    }

    fn to_js_value(&self) -> JsValue {
        JsValue::from_str(self)
    }
}

impl ManagedValue for Set {
    fn try_from_js(value: &JsValue) -> Option<Self> {
        value.dyn_ref::<Set>().cloned()
    }

    fn to_js_value(&self) -> JsValue {
        self.clone().into()
    }
}

#[derive(Clone)]
pub(super) struct JsBinding {
    object: Object,
    property: JsValue,
    path: String,
}

pub(super) enum StateCell<T: ManagedValue> {
    Rust(RefCell<T>),
    Js(JsBinding, PhantomData<T>),
}

impl<T: ManagedValue> StateCell<T> {
    pub(super) fn from_optional_state_object(
        state_object: Option<&Object>,
        key: &'static str,
        default: T,
    ) -> Result<Self, JsValue> {
        let Some(root) = state_object else {
            return Ok(Self::Rust(RefCell::new(default)));
        };

        let key_js = JsValue::from_str(key);
        let raw_value = Reflect::get(root, &key_js)?;

        if raw_value.is_undefined() {
            return Ok(Self::Rust(RefCell::new(default)));
        }

        if let Some(nested_object) = raw_value.dyn_ref::<Object>() {
            let boxed_value_key = JsValue::from_str("value");
            let boxed_value = Reflect::get(nested_object, &boxed_value_key)?;

            if !boxed_value.is_undefined() && T::try_from_js(&boxed_value).is_some() {
                return Ok(Self::Js(
                    JsBinding {
                        object: nested_object.clone(),
                        property: boxed_value_key,
                        path: format!("{key}.value"),
                    },
                    PhantomData,
                ));
            }
        }

        if T::try_from_js(&raw_value).is_some() {
            return Ok(Self::Js(
                JsBinding {
                    object: root.clone(),
                    property: key_js,
                    path: key.to_owned(),
                },
                PhantomData,
            ));
        }

        Err(JsValue::from_str(&format!(
            "state.{key} must be a {} or boxed as {{ value: {} }}",
            type_name::<T>(),
            type_name::<T>()
        )))
    }

    pub(super) fn get(&self) -> Result<T, JsValue> {
        match self {
            StateCell::Rust(value) => Ok(value.borrow().clone()),
            StateCell::Js(binding, _) => {
                let raw_value = Reflect::get(&binding.object, &binding.property)?;
                T::try_from_js(&raw_value).ok_or_else(|| {
                    JsValue::from_str(&format!(
                        "state.{} must stay a {}",
                        binding.path,
                        type_name::<T>()
                    ))
                })
            }
        }
    }

    pub(super) fn set(&self, next: T) -> Result<(), JsValue> {
        match self {
            StateCell::Rust(value) => {
                *value.borrow_mut() = next;
                Ok(())
            }
            StateCell::Js(binding, _) => {
                Reflect::set(&binding.object, &binding.property, &next.to_js_value())?;
                Ok(())
            }
        }
    }

    pub(super) fn update<F>(&self, updater: F) -> Result<T, JsValue>
    where
        F: FnOnce(T) -> T,
    {
        let current = self.get()?;
        let next = updater(current);
        self.set(next.clone())?;
        Ok(next)
    }
}

pub(super) fn call_method1(
    target: &JsValue,
    method: &str,
    arg: &JsValue,
) -> Result<JsValue, JsValue> {
    let method_value = Reflect::get(target, &JsValue::from_str(method))?;
    let method_fn = method_value
        .dyn_into::<Function>()
        .map_err(|_| JsValue::from_str(&format!("state method '{method}' is not callable")))?;

    method_fn.call1(target, arg)
}

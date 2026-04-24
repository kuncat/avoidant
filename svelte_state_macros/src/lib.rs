use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Data, DeriveInput, Expr, Fields, GenericArgument, LitStr, PathArguments, Type,
};

#[proc_macro_derive(SvelteStateModel, attributes(svelte_state))]
pub fn derive_svelte_state_model(input: TokenStream) -> TokenStream {
    match derive_svelte_state_model_impl(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

enum FieldRole {
    StateObject,
    SvelteState {
        key: LitStr,
        default_expr: Expr,
        inner_ty: Type,
    },
    Default,
}

fn derive_svelte_state_model_impl(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let struct_name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fields = match input.data {
        Data::Struct(data_struct) => match data_struct.fields {
            Fields::Named(fields) => fields.named,
            _ => {
                return Err(syn::Error::new(
                    data_struct.fields.span(),
                    "SvelteStateModel requires a struct with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new(
                struct_name.span(),
                "SvelteStateModel can only be derived for structs",
            ));
        }
    };

    let mut field_initializers = Vec::new();
    let mut generated_accessors = Vec::new();
    let mut generated_wasm_exports = Vec::new();

    for field in fields {
        let field_ident = field
            .ident
            .clone()
            .ok_or_else(|| syn::Error::new(field.span(), "Expected a named field"))?;

        let role = parse_field_role(&field)?;

        match role {
            FieldRole::StateObject => {
                field_initializers.push(quote! {
                    #field_ident: state_object.clone()
                });
            }
            FieldRole::Default => {
                field_initializers.push(quote! {
                    #field_ident: ::core::default::Default::default()
                });
            }
            FieldRole::SvelteState {
                key,
                default_expr,
                inner_ty,
            } => {
                let getter = format_ident!("get_{}", field_ident);
                let setter = format_ident!("set_{}", field_ident);
                let updater = format_ident!("update_{}", field_ident);
                let exported_getter = format_ident!("__svelte_export_{}", getter);
                let exported_setter = format_ident!("__svelte_export_{}", setter);

                field_initializers.push(quote! {
                    #field_ident: StateCell::from_optional_state_object(
                        state_object.as_ref(),
                        #key,
                        #default_expr,
                    )?
                });

                generated_accessors.push(quote! {
                    fn #getter(&self) -> ::core::result::Result<#inner_ty, wasm_bindgen::JsValue> {
                        self.#field_ident.get()
                    }

                    fn #setter(&self, next: #inner_ty) -> ::core::result::Result<(), wasm_bindgen::JsValue> {
                        self.#field_ident.set(next)
                    }

                    fn #updater<F>(&self, updater: F) -> ::core::result::Result<#inner_ty, wasm_bindgen::JsValue>
                    where
                        F: ::core::ops::FnOnce(#inner_ty) -> #inner_ty,
                    {
                        self.#field_ident.update(updater)
                    }
                });

                generated_wasm_exports.push(quote! {
                    #[wasm_bindgen::prelude::wasm_bindgen(js_name = #getter)]
                    pub fn #exported_getter(&self) -> ::core::result::Result<#inner_ty, wasm_bindgen::JsValue> {
                        self.#getter()
                    }

                    #[wasm_bindgen::prelude::wasm_bindgen(js_name = #setter)]
                    pub fn #exported_setter(&self, next: #inner_ty) -> ::core::result::Result<(), wasm_bindgen::JsValue> {
                        self.#setter(next)
                    }
                });
            }
        }
    }

    Ok(quote! {
        impl #impl_generics #struct_name #ty_generics #where_clause {
            fn build_from_state_object(
                state_object: ::core::option::Option<js_sys::Object>,
            ) -> ::core::result::Result<Self, wasm_bindgen::JsValue> {
                Ok(Self {
                    #(#field_initializers,)*
                })
            }

            #(#generated_accessors)*
        }

        #[wasm_bindgen::prelude::wasm_bindgen]
        impl #impl_generics #struct_name #ty_generics #where_clause {
            #(#generated_wasm_exports)*
        }
    })
}

fn parse_field_role(field: &syn::Field) -> syn::Result<FieldRole> {
    let mut is_state_object = false;
    let mut is_default_marker = false;
    let mut key: Option<LitStr> = None;
    let mut default_expr: Option<Expr> = None;

    let mut saw_attribute = false;

    for attr in &field.attrs {
        if !attr.path().is_ident("svelte_state") {
            continue;
        }

        saw_attribute = true;

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("state_object") {
                is_state_object = true;
                return Ok(());
            }

            if meta.path.is_ident("default") {
                if meta.input.peek(syn::Token![=]) {
                    let value = meta.value()?;
                    default_expr = Some(value.parse::<Expr>()?);
                } else {
                    is_default_marker = true;
                }
                return Ok(());
            }

            if meta.path.is_ident("key") {
                let value = meta.value()?;
                key = Some(value.parse::<LitStr>()?);
                return Ok(());
            }

            Err(meta.error("Unsupported svelte_state option"))
        })?;
    }

    if !saw_attribute {
        return Err(syn::Error::new(
            field.span(),
            "Every field in a SvelteStateModel must have #[svelte_state(...)]",
        ));
    }

    if is_state_object {
        if key.is_some() || default_expr.is_some() || is_default_marker {
            return Err(syn::Error::new(
                field.span(),
                "#[svelte_state(state_object)] cannot be combined with other options",
            ));
        }
        return Ok(FieldRole::StateObject);
    }

    if is_default_marker {
        if key.is_some() || default_expr.is_some() {
            return Err(syn::Error::new(
                field.span(),
                "#[svelte_state(default)] cannot be combined with key/default=value",
            ));
        }
        return Ok(FieldRole::Default);
    }

    if let Some(key) = key {
        let default_expr = default_expr.ok_or_else(|| {
            syn::Error::new(
                field.span(),
                "SvelteState fields require both key = \"...\" and default = <expr>",
            )
        })?;

        let inner_ty = extract_state_cell_inner_ty(&field.ty)?;

        return Ok(FieldRole::SvelteState {
            key,
            default_expr,
            inner_ty,
        });
    }

    Err(syn::Error::new(
        field.span(),
        "Invalid #[svelte_state(...)] configuration",
    ))
}

fn extract_state_cell_inner_ty(field_ty: &Type) -> syn::Result<Type> {
    let Type::Path(type_path) = field_ty else {
        return Err(syn::Error::new(
            field_ty.span(),
            "SvelteState fields must use type StateCell<T>",
        ));
    };

    let segment = type_path.path.segments.last().ok_or_else(|| {
        syn::Error::new(field_ty.span(), "SvelteState fields must use type StateCell<T>")
    })?;

    if segment.ident != "StateCell" {
        return Err(syn::Error::new(
            field_ty.span(),
            "SvelteState fields must use type StateCell<T>",
        ));
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(syn::Error::new(
            field_ty.span(),
            "StateCell<T> requires a generic type argument",
        ));
    };

    let first_arg = arguments.args.first().ok_or_else(|| {
        syn::Error::new(
            field_ty.span(),
            "StateCell<T> requires exactly one generic type argument",
        )
    })?;

    let GenericArgument::Type(inner_ty) = first_arg else {
        return Err(syn::Error::new(
            field_ty.span(),
            "StateCell<T> generic argument must be a type",
        ));
    };

    Ok(inner_ty.clone())
}

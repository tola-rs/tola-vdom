//! `#[vdom::processed(FamilyRaw)]` macro implementation
//!
//! Defines a processed data struct for a family.
//!
//! # Usage
//!
//! ```ignore
//! #[vdom::processed(Math)]
//! pub struct MathProcessed {
//!     pub html: String,
//! }
//!
//! // Generates struct with stable_id field and HasStableId impl
//! ```

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse2, DeriveInput, Result};

pub fn expand(_attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let input: DeriveInput = parse2(item)?;
    let name = &input.ident;
    let vis = &input.vis;
    let attrs = &input.attrs;
    let generics = &input.generics;

    let fields = match &input.data {
        syn::Data::Struct(data) => &data.fields,
        _ => return Err(syn::Error::new_spanned(&input, "expected struct")),
    };

    // Extract existing field names to not duplicate stable_id if already present
    let has_stable_id = match fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .any(|f| f.ident.as_ref().is_some_and(|i| i == "stable_id")),
        _ => false,
    };

    let stable_id_field = if has_stable_id {
        quote! {}
    } else {
        quote! { pub stable_id: ::tola_vdom::id::StableId, }
    };

    // Re-extract fields for struct body
    let existing_fields = match fields {
        syn::Fields::Named(named) => {
            let f = named.named.iter();
            quote! { #(#f,)* }
        }
        syn::Fields::Unit => quote! {},
        syn::Fields::Unnamed(_) => {
            return Err(syn::Error::new_spanned(
                &input,
                "tuple structs not supported",
            ))
        }
    };

    Ok(quote! {
        #[derive(Clone, Debug, Default)]
        #(#attrs)*
        #vis struct #name #generics {
            #stable_id_field
            #existing_fields
        }

        impl ::tola_vdom::core::HasStableId for #name {
            fn stable_id(&self) -> ::tola_vdom::id::StableId {
                self.stable_id
            }
        }
    })
}

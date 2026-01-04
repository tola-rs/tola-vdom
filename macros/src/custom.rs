//! `#[vdom::family]` macro implementation
//!
//! Generates a complete `Family` trait implementation for user-defined families.
//!
//! # Usage
//!
//! ```ignore
//! #[vdom::family(processed = MathProcessed)]
//! pub struct Math {
//!     pub formula: String,
//!     pub display: bool,
//! }
//!
//! // Generates:
//! // - Math (the Raw data type, with Default + Clone + Debug)
//! // - MathIndexed (with stable_id field)
//! // - impl Family for MathFamily
//! ```

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{parse2, DeriveInput, Ident, Result, Token};

struct FamilyArgs {
    processed: Option<Ident>,
}

impl Parse for FamilyArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.is_empty() {
            return Ok(Self { processed: None });
        }

        let ident: Ident = input.parse()?;
        if ident != "processed" {
            return Err(syn::Error::new_spanned(ident, "expected `processed`"));
        }
        input.parse::<Token![=]>()?;
        let processed: Ident = input.parse()?;
        Ok(Self {
            processed: Some(processed),
        })
    }
}

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let args: FamilyArgs = parse2(attr)?;
    let input: DeriveInput = parse2(item)?;
    let raw_name = &input.ident;
    let vis = &input.vis;
    let attrs = &input.attrs;
    let generics = &input.generics;

    let fields = match &input.data {
        syn::Data::Struct(data) => &data.fields,
        _ => return Err(syn::Error::new_spanned(&input, "expected struct")),
    };

    // Generate family name (lowercase)
    let family_name_str = raw_name.to_string().to_lowercase();
    let family_struct = format_ident!("{}Family", raw_name);
    let indexed_name = format_ident!("{}Indexed", raw_name);

    // Processed type: explicit or default to Raw
    let processed_name = args.processed.unwrap_or_else(|| format_ident!("{}Processed", raw_name));

    // Extract field names and types for copying to Indexed struct
    let field_names: Vec<_> = match fields {
        syn::Fields::Named(named) => named.named.iter().map(|f| &f.ident).collect(),
        _ => vec![],
    };
    let field_types: Vec<_> = match fields {
        syn::Fields::Named(named) => named.named.iter().map(|f| &f.ty).collect(),
        _ => vec![],
    };

    Ok(quote! {
        // Raw data struct (user-defined fields)
        #[derive(Clone, Default, Debug)]
        #(#attrs)*
        #vis struct #raw_name #generics #fields

        // Indexed data struct (Raw fields + stable_id)
        #[derive(Clone, Default, Debug)]
        #vis struct #indexed_name {
            pub stable_id: ::tola_vdom::id::StableId,
            #(pub #field_names: #field_types,)*
        }

        impl ::tola_vdom::core::HasStableId for #indexed_name {
            fn stable_id(&self) -> ::tola_vdom::id::StableId {
                self.stable_id
            }
        }

        // Family marker struct
        #vis struct #family_struct;

        // Family trait implementation
        impl ::tola_vdom::core::Family for #family_struct {
            const NAME: &'static str = #family_name_str;

            type Raw = #raw_name;
            type Indexed = #indexed_name;
            type Processed = #processed_name;

            fn identify(_tag: &str, _attrs: &::tola_vdom::attr::Attrs) -> bool {
                // User-defined families need explicit ext, not tag-based identification
                false
            }

            fn index(raw: Self::Raw, id: ::tola_vdom::id::StableId) -> Self::Indexed {
                #indexed_name {
                    stable_id: id,
                    #(#field_names: raw.#field_names,)*
                }
            }

            fn process(indexed: &Self::Indexed) -> Self::Processed {
                // Default: just copy stable_id, processed fields come from #[processed] macro
                #processed_name {
                    stable_id: indexed.stable_id,
                    ..Default::default()
                }
            }
        }
    })
}

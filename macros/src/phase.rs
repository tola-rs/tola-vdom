//! `#[vdom::families]` macro implementation
//!
//! Generates a complete phase system with type-safe family enums for all phases.
//!
//! # Generated Structure
//!
//! ```ignore
//! #[vdom::families]
//! pub struct MySite {
//!     link: LinkFamily,
//!     heading: HeadingFamily,
//!     math: MathFamily,  // user-defined
//! }
//!
//! // Generates:
//! mod MySite {
//!     // Extension enums for each phase
//!     pub enum RawExt { Link(LinkRaw), Heading(HeadingRaw), Math(MathRaw), None }
//!     pub enum IndexedExt { Link(LinkIndexed), Heading(HeadingIndexed), Math(MathIndexed), None(NoneIndexed) }
//!     pub enum ProcessedExt { Link(LinkProcessed), Heading(HeadingProcessed), Math(MathProcessed), None(NoneProcessed) }
//!
//!     // Phase markers
//!     pub struct Raw;
//!     pub struct Indexed;
//!     pub struct Processed;
//!
//!     impl PhaseExt for Raw { type Ext = RawExt; ... }
//!     impl PhaseExt for Indexed { type Ext = IndexedExt; ... }
//!     impl PhaseExt for Processed { type Ext = ProcessedExt; ... }
//!
//!     // GAT-based type-safe access (ExtractFamily)
//!     impl ExtractFamily<LinkFamily> for IndexedExt {
//!         type Output = LinkIndexed;
//!         fn get(&self) -> Option<&Self::Output> { ... }
//!     }
//! }
//! ```

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse2, DeriveInput, Fields, Result};

pub fn expand(_attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let input: DeriveInput = parse2(item)?;
    let name = &input.ident;
    let vis = &input.vis;

    // Extract fields (the families)
    let fields = match &input.data {
        syn::Data::Struct(syn::DataStruct {
            fields: Fields::Named(f),
            ..
        }) => &f.named,
        _ => {
            return Err(syn::Error::new_spanned(
                &input,
                "expected struct with named fields",
            ))
        }
    };

    // Collect field names and types
    let field_names: Vec<_> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();
    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();

    // Generate PascalCase variant names from field names
    let variant_names: Vec<_> = field_names
        .iter()
        .map(|n| format_ident!("{}", to_pascal_case(&n.to_string())))
        .collect();

    // Generate ExtractFamily impls for each (Ext enum, Family) combination
    let extract_family_impls = generate_extract_family_impls(&variant_names, &field_types);

    // Generate the module
    Ok(quote! {
        #[allow(non_snake_case)]
        #vis mod #name {
            use super::*;
            use ::tola_vdom::core::{
                Family, HasStableId, Phase, PhaseExt, ElementExt, IndexedExt as IndexedExtTrait,
                NoneFamily, NoneIndexed, NoneProcessed, ExtractFamily,
            };
            use ::tola_vdom::id::StableId;
            use ::tola_vdom::span::SourceSpan;
            use ::tola_vdom::transform::IndexStats;

            // =================================================================
            // Raw Phase Extension Enum
            // =================================================================

            /// Element extension for Raw phase.
            ///
            /// Contains the Raw data for each family, plus source span info.
            #[derive(Debug, Clone, Default)]
            pub enum RawExt {
                #[default]
                None,
                #(#variant_names(<#field_types as Family>::Raw)),*
            }

            impl ElementExt for RawExt {
                fn family_name(&self) -> &'static str {
                    match self {
                        Self::None => "none",
                        #(Self::#variant_names(_) => <#field_types as Family>::NAME),*
                    }
                }
            }

            // =================================================================
            // Indexed Phase Extension Enum
            // =================================================================

            /// Element extension for Indexed phase.
            ///
            /// Contains family-specific indexed data with StableId.
            #[derive(Debug, Clone)]
            pub enum IndexedExt {
                None(NoneIndexed),
                #(#variant_names(<#field_types as Family>::Indexed)),*
            }

            impl Default for IndexedExt {
                fn default() -> Self {
                    Self::None(NoneIndexed::default())
                }
            }

            impl ElementExt for IndexedExt {
                fn family_name(&self) -> &'static str {
                    match self {
                        Self::None(_) => "none",
                        #(Self::#variant_names(_) => <#field_types as Family>::NAME),*
                    }
                }
            }

            impl HasStableId for IndexedExt {
                fn stable_id(&self) -> StableId {
                    match self {
                        Self::None(data) => data.stable_id(),
                        #(Self::#variant_names(data) => data.stable_id()),*
                    }
                }
            }

            // =================================================================
            // Processed Phase Extension Enum
            // =================================================================

            /// Element extension for Processed phase.
            ///
            /// Contains family-specific processed data.
            #[derive(Debug, Clone)]
            pub enum ProcessedExt {
                None(NoneProcessed),
                #(#variant_names(<#field_types as Family>::Processed)),*
            }

            impl Default for ProcessedExt {
                fn default() -> Self {
                    Self::None(NoneProcessed::default())
                }
            }

            impl ElementExt for ProcessedExt {
                fn family_name(&self) -> &'static str {
                    match self {
                        Self::None(_) => "none",
                        #(Self::#variant_names(_) => <#field_types as Family>::NAME),*
                    }
                }
            }

            impl HasStableId for ProcessedExt {
                fn stable_id(&self) -> StableId {
                    match self {
                        Self::None(data) => data.stable_id(),
                        #(Self::#variant_names(data) => data.stable_id()),*
                    }
                }
            }

            // =================================================================
            // GAT-based ExtractFamily implementations
            // =================================================================

            #extract_family_impls

            // =================================================================
            // Phase Markers
            // =================================================================

            /// Raw phase - parser output with source spans.
            #[derive(Debug, Clone, Copy, Default)]
            pub struct Raw;

            impl Phase for Raw {
                const NAME: &'static str = concat!(stringify!(#name), "::Raw");
            }

            impl PhaseExt for Raw {
                type Ext = RawExt;
                type DocExt = RawDocExt;
                type TextExt = RawTextExt;
            }

            /// Indexed phase - StableIds assigned, family data indexed.
            #[derive(Debug, Clone, Copy, Default)]
            pub struct Indexed;

            impl Phase for Indexed {
                const NAME: &'static str = concat!(stringify!(#name), "::Indexed");
            }

            impl PhaseExt for Indexed {
                type Ext = IndexedExt;
                type DocExt = IndexedDocExt;
                type TextExt = IndexedTextExt;
            }

            /// Processed phase - all transformations applied.
            #[derive(Debug, Clone, Copy, Default)]
            pub struct Processed;

            impl Phase for Processed {
                const NAME: &'static str = concat!(stringify!(#name), "::Processed");
            }

            impl PhaseExt for Processed {
                type Ext = ProcessedExt;
                type DocExt = ProcessedDocExt;
                type TextExt = ();
            }

            // =================================================================
            // Document Extensions
            // =================================================================

            /// Document extension for Raw phase.
            #[derive(Debug, Clone, Default)]
            pub struct RawDocExt {
                pub source_path: Option<String>,
            }

            /// Document extension for Indexed phase.
            #[derive(Debug, Clone, Default)]
            pub struct IndexedDocExt {
                pub source_path: Option<String>,
                pub node_count: usize,
            }

            /// Document extension for Processed phase.
            #[derive(Debug, Clone, Default)]
            pub struct ProcessedDocExt {
                pub node_count: usize,
            }

            // =================================================================
            // Text Extensions
            // =================================================================

            /// Text extension for Raw phase - source span.
            #[derive(Debug, Clone, Default)]
            pub struct RawTextExt {
                pub span: Option<SourceSpan>,
            }

            /// Text extension for Indexed phase - StableId.
            #[derive(Debug, Clone, Default)]
            pub struct IndexedTextExt {
                pub stable_id: StableId,
            }

            impl HasStableId for IndexedTextExt {
                fn stable_id(&self) -> StableId {
                    self.stable_id
                }
            }

            // =================================================================
            // Family Identification
            // =================================================================

            /// Identify which family an element belongs to based on tag and attrs.
            pub fn identify(tag: &str, attrs: &::tola_vdom::attr::Attrs) -> &'static str {
                // Check each family in order (user-defined families take precedence)
                #(
                    if <#field_types as Family>::identify(tag, attrs) {
                        return <#field_types as Family>::NAME;
                    }
                )*
                // Fallback to None
                "none"
            }

            /// Create Raw extension from tag and attrs.
            pub fn create_raw_ext(tag: &str, attrs: &::tola_vdom::attr::Attrs) -> RawExt {
                #(
                    if <#field_types as Family>::identify(tag, attrs) {
                        return RawExt::#variant_names(Default::default());
                    }
                )*
                RawExt::None
            }

            /// Create a Raw element with automatic family identification.
            ///
            /// This is the recommended way to create elements:
            /// ```ignore
            /// let elem = MySite::element("a", attrs);
            /// ```
            pub fn element(tag: impl Into<::tola_vdom::attr::Tag>, attrs: ::tola_vdom::attr::Attrs) -> ::tola_vdom::Element<Raw> {
                let tag = tag.into();
                let ext = create_raw_ext(&tag, &attrs);
                let mut elem = ::tola_vdom::Element::with_ext(tag, ext);
                elem.attrs = attrs;
                elem
            }

            /// Create a Raw element with explicit family extension.
            ///
            /// Use when you need to set specific family data:
            /// ```ignore
            /// let elem = MySite::element_with_ext("span", RawExt::Math(Math::inline("x^2")), attrs);
            /// ```
            pub fn element_with_ext(tag: impl Into<::tola_vdom::attr::Tag>, ext: RawExt, attrs: ::tola_vdom::attr::Attrs) -> ::tola_vdom::Element<Raw> {
                let mut elem = ::tola_vdom::Element::with_ext(tag, ext);
                elem.attrs = attrs;
                elem
            }

            // =================================================================
            // Phase Transitions
            // =================================================================

            /// Index: Raw → Indexed
            pub fn index_ext(raw: RawExt, id: StableId) -> IndexedExt {
                match raw {
                    RawExt::None => IndexedExt::None(NoneFamily::index((), id)),
                    #(
                        RawExt::#variant_names(data) => {
                            IndexedExt::#variant_names(<#field_types as Family>::index(data, id))
                        }
                    )*
                }
            }

            /// Process: Indexed → Processed
            pub fn process_ext(indexed: &IndexedExt) -> ProcessedExt {
                match indexed {
                    IndexedExt::None(data) => ProcessedExt::None(NoneFamily::process(data)),
                    #(
                        IndexedExt::#variant_names(data) => {
                            ProcessedExt::#variant_names(<#field_types as Family>::process(data))
                        }
                    )*
                }
            }

            // =================================================================
            // Indexer and Processor Factory Functions
            // =================================================================

            // Type aliases for function pointer types
            type IndexExtFn = fn(RawExt, StableId) -> IndexedExt;
            type IndexTextExtFn = fn(RawTextExt, StableId) -> IndexedTextExt;
            type IndexDocExtFn = fn(RawDocExt, ::tola_vdom::transform::IndexStats) -> IndexedDocExt;
            type ProcessExtFn = fn(&IndexedExt) -> ProcessedExt;
            type ProcessDocExtFn = fn(&IndexedDocExt) -> ProcessedDocExt;

            /// Create an Indexer configured for this site's phases.
            ///
            /// ```ignore
            /// let indexer = MySite::indexer();
            /// let indexed_doc = raw_doc.pipe(indexer);
            /// ```
            pub fn indexer() -> ::tola_vdom::Indexer<Raw, Indexed, IndexExtFn, IndexTextExtFn, IndexDocExtFn> {
                fn do_index_ext(ext: RawExt, id: StableId) -> IndexedExt {
                    index_ext(ext, id)
                }
                fn do_index_text(_text_ext: RawTextExt, id: StableId) -> IndexedTextExt {
                    IndexedTextExt { stable_id: id }
                }
                fn do_index_doc(doc_ext: RawDocExt, stats: ::tola_vdom::transform::IndexStats) -> IndexedDocExt {
                    IndexedDocExt {
                        source_path: doc_ext.source_path,
                        node_count: stats.element_count + stats.text_count,
                    }
                }
                ::tola_vdom::Indexer::new(do_index_ext, do_index_text, do_index_doc)
            }

            /// Create a Processor configured for this site's phases.
            ///
            /// ```ignore
            /// let processor = MySite::processor();
            /// let processed_doc = indexed_doc.pipe(processor);
            /// ```
            pub fn processor() -> ::tola_vdom::Processor<Indexed, Processed, ProcessExtFn, ProcessDocExtFn> {
                fn do_process_ext(ext: &IndexedExt) -> ProcessedExt {
                    process_ext(ext)
                }
                fn do_process_doc(doc_ext: &IndexedDocExt) -> ProcessedDocExt {
                    ProcessedDocExt {
                        node_count: doc_ext.node_count,
                    }
                }
                ::tola_vdom::Processor::new(do_process_ext, do_process_doc)
            }

            // =================================================================
            // Serialization Trait Implementations
            // =================================================================

            impl ::tola_vdom::serialize::SerializableExt for IndexedExt {
                fn to_ser_ext(&self) -> ::tola_vdom::serialize::SerExt {
                    ::tola_vdom::serialize::SerExt {
                        stable_id: self.stable_id().as_raw(),
                        family_name: self.family_name().to_string(),
                    }
                }
            }

            impl ::tola_vdom::serialize::DeserializableExt for IndexedExt {
                fn from_ser_ext(ext: &::tola_vdom::serialize::ArchivedSerExt) -> Result<Self, String> {
                    let stable_id = StableId::from_raw(ext.stable_id.into());
                    let family_name: &str = ext.family_name.as_str();

                    // Reconstruct the appropriate variant based on family_name
                    // For deserialization, we only need stable_id - family data is regenerated
                    Ok(match family_name {
                        "none" => IndexedExt::None(NoneIndexed { stable_id }),
                        #(
                            name if name == <#field_types as Family>::NAME => {
                                // Create indexed data with just the stable_id
                                // Family-specific data will need to be regenerated
                                let raw = <<#field_types as Family>::Raw>::default();
                                IndexedExt::#variant_names(<#field_types as Family>::index(raw, stable_id))
                            }
                        )*
                        _ => IndexedExt::None(NoneIndexed { stable_id }),
                    })
                }
            }

            impl ::tola_vdom::serialize::SerializableTextExt for IndexedTextExt {
                fn stable_id(&self) -> u64 {
                    self.stable_id.as_raw()
                }
            }

            impl ::tola_vdom::serialize::DeserializableTextExt for IndexedTextExt {
                fn from_stable_id(id: u64) -> Self {
                    Self {
                        stable_id: StableId::from_raw(id),
                    }
                }
            }

            impl ::tola_vdom::serialize::SerializableDocExt for IndexedDocExt {
                fn to_ser_doc_meta(&self) -> ::tola_vdom::serialize::SerDocMeta {
                    ::tola_vdom::serialize::SerDocMeta {
                        source_path: self.source_path.clone(),
                        node_count: self.node_count as u32,
                    }
                }
            }

            impl ::tola_vdom::serialize::DeserializableDocExt for IndexedDocExt {
                fn from_ser_doc_meta(meta: &::tola_vdom::serialize::ArchivedSerDocMeta) -> Result<Self, String> {
                    let node_count: u32 = meta.node_count.into();
                    Ok(Self {
                        source_path: meta.source_path.as_ref().map(|s| s.as_str().to_string()),
                        node_count: node_count as usize,
                    })
                }
            }

            // =================================================================
            // FamilyKind - Type aliases for type-safe family selection
            // =================================================================

            /// Type aliases for families in this site.
            ///
            /// Use with `find_by` and `modify_by` for type-safe family operations:
            ///
            /// ```ignore
            /// doc.find_by::<MySite::FamilyKind::Link>();
            /// doc.modify_by::<MySite::FamilyKind::Heading, _>(|elem| { ... });
            /// ```
            pub mod FamilyKind {
                use super::super::*;
                #(
                    pub type #variant_names = #field_types;
                )*
            }
        }
    })
}

/// Generate ExtractFamily implementations for each (Ext enum, Family) combination.
///
/// This provides GAT-based type-safe access:
/// ```ignore
/// let ext: IndexedExt = ...;
/// if let Some(link_data) = ext.get::<LinkFamily>() {
///     // link_data: &LinkIndexed (exact type!)
/// }
/// ```
fn generate_extract_family_impls(
    variant_names: &[proc_macro2::Ident],
    field_types: &[&syn::Type],
) -> TokenStream {
    let mut impls = Vec::new();

    for (variant, family_ty) in variant_names.iter().zip(field_types.iter()) {
        // RawExt: ExtractFamily<F> -> F::Raw
        impls.push(quote! {
            impl ExtractFamily<#family_ty> for RawExt {
                type Output = <#family_ty as Family>::Raw;

                fn get(&self) -> Option<&Self::Output> {
                    match self {
                        Self::#variant(data) => Some(data),
                        _ => None,
                    }
                }

                fn get_mut(&mut self) -> Option<&mut Self::Output> {
                    match self {
                        Self::#variant(data) => Some(data),
                        _ => None,
                    }
                }
            }
        });

        // IndexedExt: ExtractFamily<F> -> F::Indexed
        impls.push(quote! {
            impl ExtractFamily<#family_ty> for IndexedExt {
                type Output = <#family_ty as Family>::Indexed;

                fn get(&self) -> Option<&Self::Output> {
                    match self {
                        Self::#variant(data) => Some(data),
                        _ => None,
                    }
                }

                fn get_mut(&mut self) -> Option<&mut Self::Output> {
                    match self {
                        Self::#variant(data) => Some(data),
                        _ => None,
                    }
                }
            }
        });

        // ProcessedExt: ExtractFamily<F> -> F::Processed
        impls.push(quote! {
            impl ExtractFamily<#family_ty> for ProcessedExt {
                type Output = <#family_ty as Family>::Processed;

                fn get(&self) -> Option<&Self::Output> {
                    match self {
                        Self::#variant(data) => Some(data),
                        _ => None,
                    }
                }

                fn get_mut(&mut self) -> Option<&mut Self::Output> {
                    match self {
                        Self::#variant(data) => Some(data),
                        _ => None,
                    }
                }
            }
        });
    }

    quote! { #(#impls)* }
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

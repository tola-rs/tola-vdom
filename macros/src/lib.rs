//! Proc macros for tola-vdom families
//!
//! # Macros
//!
//! - `#[tola_vdom::family]` or `#[vdom::family]` - Define a custom TagFamily
//! - `#[tola_vdom::families]` or `#[vdom::families]` - Combine families into phase

mod custom;
mod phase;
mod processed;

use proc_macro::TokenStream;

/// Define a custom TagFamily.
///
/// Name auto-derived: MathFamily -> "math"
///
/// # Example
///
/// ```ignore
/// #[tola_vdom::family]
/// pub struct MathFamily {
///     type Indexed = MathIndexed;
///     type Processed = MathProcessed;
/// }
/// ```
#[proc_macro_attribute]
pub fn family(attr: TokenStream, item: TokenStream) -> TokenStream {
    custom::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Define processed data for a custom family.
#[proc_macro_attribute]
pub fn processed(attr: TokenStream, item: TokenStream) -> TokenStream {
    processed::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Combine custom families into a phase module with FamilyExt enum.
///
/// # Example
///
/// ```ignore
/// #[tola_vdom::families]
/// pub struct MySite {
///     math: MathFamily,
///     code: CodeFamily,
/// }
/// // Generates: MySite::Raw, MySite::RawCustom, etc.
/// ```
#[proc_macro_attribute]
pub fn families(attr: TokenStream, item: TokenStream) -> TokenStream {
    phase::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

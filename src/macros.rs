//! Utility macros for VDOM
//!
//! These macros eliminate repetitive pattern matching code.

/// Generate is_xxx, as_xxx, as_xxx_mut for enums with typed variants.
///
/// # Example
/// ```ignore
/// impl<P: Phase> Node<P> {
///     impl_enum_accessors!(P; element, text);
/// }
/// // Generates: is_element, as_element, as_element_mut, is_text, as_text, as_text_mut
/// ```
#[macro_export]
macro_rules! impl_enum_accessors {
    ($phase:ty; $($variant:ident),* $(,)?) => {
        ::paste::paste! {
            $(
                pub fn [<is_ $variant>](&self) -> bool {
                    matches!(self, Self::[<$variant:camel>](_))
                }

                pub fn [<as_ $variant>](&self) -> Option<&[<$variant:camel>]<$phase>> {
                    match self { Self::[<$variant:camel>](v) => Some(v), _ => None }
                }

                pub fn [<as_ $variant _mut>](&mut self) -> Option<&mut [<$variant:camel>]<$phase>> {
                    match self { Self::[<$variant:camel>](v) => Some(v), _ => None }
                }
            )*
        }
    };
}

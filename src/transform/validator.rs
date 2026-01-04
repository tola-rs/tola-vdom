//! Async validation traits for documents.
//!
//! Only available with the `async` feature.

use std::future::Future;

use crate::core::PhaseExt;
use crate::node::Document;

/// Asynchronously validates items collected from a document.
///
/// A `Validator` is a complete validation task that:
/// 1. Collects data from a document (synchronous)
/// 2. Validates the collected data (asynchronous)
///
/// # Example
///
/// ```ignore
/// struct LinkValidator {
///     client: HttpClient,
/// }
///
/// impl<P: PhaseExt> Validator<P> for LinkValidator {
///     type Item = String;
///     type Error = LinkError;
///
///     fn collect(&self, doc: &Document<P>) -> impl IntoIterator<Item = Self::Item> {
///         doc.find_by::<MySite::FamilyKind::Link>()
///             .into_iter()
///             .filter_map(|elem| elem.get_attr("href"))
///             .map(|s| s.to_string())
///     }
///
///     async fn validate(self, items: Vec<Self::Item>) -> Result<(), Self::Error> {
///         for url in items {
///             self.client.head(&url).await?;
///         }
///         Ok(())
///     }
/// }
/// ```
pub trait Validator<P: PhaseExt>: Send {
    /// The type of items collected from the document.
    type Item: Send;

    /// Error type returned when validation fails.
    type Error: Send + 'static;

    /// Collect items from the document.
    ///
    /// Called synchronously when the validator is added to the pipeline.
    /// Returns any iterator over items.
    fn collect(&self, doc: &Document<P>) -> impl IntoIterator<Item = Self::Item>;

    /// Asynchronously validate the collected items.
    ///
    /// This method consumes the validator.
    fn validate(self, items: Vec<Self::Item>) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

// =============================================================================
// NoopValidator
// =============================================================================

/// A validator that does nothing.
///
/// Useful as a placeholder or in generic contexts where a validator is required
/// but no actual validation is needed.
///
/// # Example
///
/// ```ignore
/// use tola_vdom::transform::NoopValidator;
///
/// // Use as a default when no validation is needed
/// let validator: Box<dyn Validator<Indexed>> = if need_validation {
///     Box::new(LinkValidator::new())
/// } else {
///     Box::new(NoopValidator)
/// };
/// ```
///
/// Note: For conditional validation, prefer using `AsyncPipeline::validate_if()`
/// which provides a more ergonomic API.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopValidator;

impl<P: PhaseExt> Validator<P> for NoopValidator {
    type Item = ();
    type Error = std::convert::Infallible;

    #[inline]
    fn collect(&self, _doc: &Document<P>) -> impl IntoIterator<Item = Self::Item> {
        std::iter::empty()
    }

    #[inline]
    async fn validate(self, _items: Vec<Self::Item>) -> Result<(), Self::Error> {
        Ok(())
    }
}

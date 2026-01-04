//! Document transform and validation system.
//!
//! # Module Structure
//!
//! - `Transform` - Core trait for phase transformations
//! - `Indexer` - Raw → Indexed phase transform
//! - `Processor` - Indexed → Processed phase transform
//! - `Pipeline` - Synchronous document processing pipeline
//!
//! With `async` feature:
//! - `Validator` - Async validation trait (collect + validate)
//! - `AsyncPipeline` - Pipeline with async validation support
//! - `ValidateError` / `ValidateErrors` - Validation error types
//!
//! # Example
//!
//! ```ignore
//! use tola_vdom::transform::{Pipeline, Indexer, Processor};
//!
//! let doc = Pipeline::new(raw_doc)
//!     .pipe(indexer)
//!     .pipe(processor)
//!     .into_inner();
//!
//! // With async validation:
//! let doc = Pipeline::new(raw_doc)
//!     .pipe(indexer)
//!     .pipe(processor)
//!     .into_async()
//!     .validate(LinkValidator::new())
//!     .finish()
//!     .await?;
//! ```

mod core;
mod indexer;
mod processor;
mod pipeline;

#[cfg(feature = "async")]
mod validator;
#[cfg(feature = "async")]
mod error;

// Always available
pub use core::{Transform, IdentityTransform};
pub use indexer::{Indexer, IndexStats};
pub use processor::Processor;
pub use pipeline::Pipeline;

// Async feature
#[cfg(feature = "async")]
pub use validator::{Validator, NoopValidator};
#[cfg(feature = "async")]
pub use pipeline::AsyncPipeline;
#[cfg(feature = "async")]
pub use error::{ValidateError, ValidateErrors};

#[cfg(all(test, feature = "macros"))]
mod tests {
    use super::*;
    use crate::core::{HasStableId, ExtractFamily};
    use crate::families::{LinkFamily, HeadingFamily, SvgFamily, MediaFamily};
    use crate::families::link::LinkRaw;
    use crate::families::heading::HeadingRaw;
    use crate::node::{Document, Element};
    use crate::vdom;

    #[vdom::families]
    pub struct TransformTestSite {
        link: LinkFamily,
        heading: HeadingFamily,
        svg: SvgFamily,
        media: MediaFamily,
    }

    // Helper function to create an indexer
    fn make_indexer() -> impl crate::transform::Transform<TransformTestSite::Raw, To = TransformTestSite::Indexed> {
        Indexer::new(
            TransformTestSite::index_ext,
            |_: TransformTestSite::RawTextExt, id| TransformTestSite::IndexedTextExt { stable_id: id },
            |raw: TransformTestSite::RawDocExt, stats: IndexStats| TransformTestSite::IndexedDocExt {
                source_path: raw.source_path,
                node_count: stats.element_count + stats.text_count,
            },
        )
    }

    // Helper function to create a processor
    fn make_processor() -> impl crate::transform::Transform<TransformTestSite::Indexed, To = TransformTestSite::Processed> {
        Processor::new(
            TransformTestSite::process_ext,
            |indexed: &TransformTestSite::IndexedDocExt| TransformTestSite::ProcessedDocExt {
                node_count: indexed.node_count,
            },
        )
    }

    #[test]
    fn test_indexer_basic() {
        // Create raw document
        let root: Element<TransformTestSite::Raw> = Element::new("div")
            .child(Element::new("span"))
            .text("Hello");

        let doc = Document::new(root);
        let indexed = make_indexer().transform(doc);

        // Verify indexing
        assert_eq!(indexed.meta.node_count, 3); // div + span + text
        assert!(indexed.root.ext.stable_id().as_raw() != 0);
    }

    #[test]
    fn test_indexer_with_family() {
        let raw_ext = TransformTestSite::RawExt::Link(LinkRaw::new("https://example.com"));
        let root: Element<TransformTestSite::Raw> = Element::with_ext("a", raw_ext)
            .text("Click me");

        let doc = Document::new(root);
        let indexed = make_indexer().transform(doc);

        // Verify family data is preserved
        assert_eq!(indexed.root.family_name(), "link");
        let link_data = ExtractFamily::<LinkFamily>::get(&indexed.root.ext).unwrap();
        assert_eq!(link_data.href.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn test_processor_basic() {
        // First, create and index a document
        let raw_ext = TransformTestSite::RawExt::Heading(HeadingRaw::new(1));
        let root: Element<TransformTestSite::Raw> = Element::with_ext("h1", raw_ext)
            .text("Title");

        let doc = Document::new(root);
        let indexed = make_indexer().transform(doc);
        let indexed_id = indexed.root.ext.stable_id();

        // Now process
        let processed = make_processor().transform(indexed);

        // Verify processing
        assert_eq!(processed.root.family_name(), "heading");
        assert_eq!(processed.root.ext.stable_id(), indexed_id);

        // Verify processed heading data
        let heading_data = ExtractFamily::<HeadingFamily>::get(&processed.root.ext).unwrap();
        // HeadingProcessed has anchor_id, toc_text, in_toc (level is only in Indexed)
        // Note: anchor_id may be empty if no original_id was set
        assert!(heading_data.in_toc);
    }

    #[test]
    fn test_full_pipeline() {
        // Create a complex document
        let link_ext = TransformTestSite::RawExt::Link(LinkRaw::new("https://external.com"));
        let heading_ext = TransformTestSite::RawExt::Heading(HeadingRaw::new(2));

        let root: Element<TransformTestSite::Raw> = Element::new("article")
            .child(Element::with_ext("h2", heading_ext).text("Section Title"))
            .child(Element::new("p").text("Some content"))
            .child(Element::with_ext("a", link_ext).text("External Link"));

        let doc = Document::new(root);

        // Index
        let indexed = make_indexer().transform(doc);
        assert_eq!(indexed.meta.node_count, 7); // 4 elements + 3 text nodes

        // Process
        let processed = make_processor().transform(indexed);

        // Verify the processed document
        let links = processed.find_by::<TransformTestSite::FamilyKind::Link>();
        assert_eq!(links.len(), 1);
        let link_data = ExtractFamily::<LinkFamily>::get(&links[0].ext).unwrap();
        assert!(link_data.is_external);

        let headings = processed.find_by::<TransformTestSite::FamilyKind::Heading>();
        assert_eq!(headings.len(), 1);
        let heading_data = ExtractFamily::<HeadingFamily>::get(&headings[0].ext).unwrap();
        // HeadingProcessed has in_toc flag
        assert!(heading_data.in_toc);
    }

    #[test]
    fn test_pipe_syntax() {
        let root: Element<TransformTestSite::Raw> = Element::new("div");
        let doc = Document::new(root);

        // Use Pipeline for fluent syntax
        let processed = Pipeline::new(doc)
            .pipe(make_indexer())
            .pipe(make_processor())
            .into_inner();

        assert_eq!(processed.meta.node_count, 1);
    }
}

// =============================================================================
// Async Pipeline Tests
// =============================================================================

#[cfg(all(test, feature = "macros", feature = "async"))]
mod async_tests {
    use super::*;
    use crate::core::PhaseExt;
    use crate::families::{LinkFamily, HeadingFamily, SvgFamily, MediaFamily};
    use crate::families::link::LinkRaw;
    use crate::node::{Document, Element};
    use crate::vdom;

    #[vdom::families]
    pub struct AsyncTestSite {
        link: LinkFamily,
        heading: HeadingFamily,
        svg: SvgFamily,
        media: MediaFamily,
    }

    // Validator that always succeeds
    struct SuccessValidator;

    impl<P: PhaseExt> Validator<P> for SuccessValidator {
        type Item = ();
        type Error = std::convert::Infallible;

        fn collect(&self, _doc: &Document<P>) -> impl IntoIterator<Item = Self::Item> {
            [()]
        }

        async fn validate(self, _items: Vec<Self::Item>) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    // Validator that always fails
    struct FailValidator {
        message: String,
    }

    impl FailValidator {
        fn new(message: impl Into<String>) -> Self {
            Self { message: message.into() }
        }
    }

    #[derive(Debug)]
    struct TestError(String);

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl<P: PhaseExt> Validator<P> for FailValidator {
        type Item = ();
        type Error = TestError;

        fn collect(&self, _doc: &Document<P>) -> impl IntoIterator<Item = Self::Item> {
            [()]
        }

        async fn validate(self, _items: Vec<Self::Item>) -> Result<(), Self::Error> {
            Err(TestError(self.message))
        }
    }

    #[tokio::test]
    async fn test_async_pipeline_success() {
        let root: Element<AsyncTestSite::Raw> = Element::new("div");
        let doc = Document::new(root);

        let result = Pipeline::new(doc)
            .into_async()
            .validate(SuccessValidator)
            .finish()
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_async_pipeline_failure() {
        let root: Element<AsyncTestSite::Raw> = Element::new("div");
        let doc = Document::new(root);

        let result = Pipeline::new(doc)
            .into_async()
            .validate(FailValidator::new("test error"))
            .finish()
            .await;

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors.errors[0].message.contains("test error"));
    }

    #[tokio::test]
    async fn test_async_pipeline_multiple_checks() {
        let root: Element<AsyncTestSite::Raw> = Element::new("div");
        let doc = Document::new(root);

        let result = Pipeline::new(doc)
            .into_async()
            .validate(SuccessValidator)
            .validate(FailValidator::new("error 1"))
            .validate(FailValidator::new("error 2"))
            .finish()
            .await;

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 2);
    }

    #[tokio::test]
    async fn test_async_pipeline_with_pipe() {
        let raw_ext = AsyncTestSite::RawExt::Link(LinkRaw::new("https://example.com"));
        let root: Element<AsyncTestSite::Raw> = Element::with_ext("a", raw_ext);
        let doc = Document::new(root);

        // Create indexer
        fn make_indexer() -> impl Transform<AsyncTestSite::Raw, To = AsyncTestSite::Indexed> {
            Indexer::new(
                AsyncTestSite::index_ext,
                |_: AsyncTestSite::RawTextExt, id| AsyncTestSite::IndexedTextExt { stable_id: id },
                |raw: AsyncTestSite::RawDocExt, stats: IndexStats| AsyncTestSite::IndexedDocExt {
                    source_path: raw.source_path,
                    node_count: stats.element_count + stats.text_count,
                },
            )
        }

        // Create processor
        fn make_processor() -> impl Transform<AsyncTestSite::Indexed, To = AsyncTestSite::Processed> {
            Processor::new(
                AsyncTestSite::process_ext,
                |indexed: &AsyncTestSite::IndexedDocExt| AsyncTestSite::ProcessedDocExt {
                    node_count: indexed.node_count,
                },
            )
        }

        let result = Pipeline::new(doc)
            .pipe(make_indexer())
            .pipe(make_processor())
            .into_async()
            .validate(SuccessValidator)
            .finish()
            .await;

        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(doc.root.family_name(), "link");
    }

    #[tokio::test]
    async fn test_async_pipeline_finish_unchecked() {
        let root: Element<AsyncTestSite::Raw> = Element::new("div");
        let doc = Document::new(root);

        // Even with a failing validator, finish_unchecked returns the document
        let doc = Pipeline::new(doc)
            .into_async()
            .validate(FailValidator::new("ignored"))
            .finish_unchecked();

        assert_eq!(&*doc.root.tag, "div");
    }
}

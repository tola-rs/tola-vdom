//! Document processing pipelines.
//!
//! - `Pipeline`: Synchronous pipeline for transforms (always available)
//! - `AsyncPipeline`: Async pipeline with validation support (requires `async` feature)

use crate::core::PhaseExt;
use crate::node::Document;

use super::Transform;

// =============================================================================
// Pipeline (always available)
// =============================================================================

/// Synchronous pipeline for document processing.
///
/// Wraps a `Document` and provides fluent API for transformations and data collection.
///
/// # Example
///
/// ```ignore
/// use tola_vdom::transform::Pipeline;
///
/// let processed = Pipeline::new(doc)
///     .pipe(indexer)
///     .pipe(processor)
///     .into_inner();
/// ```
pub struct Pipeline<P: PhaseExt> {
    doc: Document<P>,
}

impl<P: PhaseExt> Pipeline<P> {
    /// Create a new pipeline from a document.
    #[inline]
    pub fn new(doc: Document<P>) -> Self {
        Self { doc }
    }

    /// Apply a synchronous transform to the document.
    #[inline]
    pub fn pipe<T>(self, transform: T) -> Pipeline<T::To>
    where
        T: Transform<P>,
    {
        Pipeline {
            doc: transform.transform(self.doc),
        }
    }

    /// Conditionally apply a transform.
    ///
    /// Only applies the transform if `condition` is true.
    /// This is more ergonomic than manual branching.
    ///
    /// # Example
    ///
    /// ```ignore
    /// Pipeline::new(doc)
    ///     .pipe(always_run)
    ///     .pipe_if(should_validate, validator)
    ///     .into_inner()
    /// ```
    #[inline]
    pub fn pipe_if<T>(self, condition: bool, transform: T) -> Pipeline<P>
    where
        T: Transform<P, To = P>,
    {
        if condition {
            self.pipe(transform)
        } else {
            self.pipe(super::IdentityTransform)
        }
    }

    /// Conditionally apply one of two transforms.
    ///
    /// Applies `then_transform` if condition is true, otherwise `else_transform`.
    /// Both transforms must produce the same output phase.
    ///
    /// # Example
    ///
    /// ```ignore
    /// Pipeline::new(doc)
    ///     .pipe_if_else(use_fast_path, FastTransform, SlowTransform)
    ///     .into_inner()
    /// ```
    #[inline]
    pub fn pipe_if_else<T1, T2>(self, condition: bool, then_transform: T1, else_transform: T2) -> Pipeline<T1::To>
    where
        T1: Transform<P>,
        T2: Transform<P, To = T1::To>,
    {
        if condition {
            self.pipe(then_transform)
        } else {
            self.pipe(else_transform)
        }
    }

    /// Inspect the document without consuming the pipeline.
    ///
    /// Useful for logging, debugging, or caching intermediate state.
    #[inline]
    pub fn inspect<F>(self, f: F) -> Self
    where
        F: FnOnce(&Document<P>),
    {
        f(&self.doc);
        self
    }

    /// Conditionally inspect the document.
    ///
    /// Only calls the closure if `condition` is true.
    ///
    /// # Example
    ///
    /// ```ignore
    /// Pipeline::new(doc)
    ///     .pipe(indexer)
    ///     .inspect_if(should_validate, |doc| validate(doc))
    ///     .into_inner()
    /// ```
    #[inline]
    pub fn inspect_if<F>(self, condition: bool, f: F) -> Self
    where
        F: FnOnce(&Document<P>),
    {
        if condition {
            f(&self.doc);
        }
        self
    }

    /// Tap into the pipeline to extract data while continuing the chain.
    ///
    /// The closure receives a reference to the document and can store data externally.
    #[inline]
    pub fn tap<F, R>(self, f: F) -> (Self, R)
    where
        F: FnOnce(&Document<P>) -> R,
    {
        let result = f(&self.doc);
        (self, result)
    }

    /// Get a reference to the underlying document.
    #[inline]
    pub fn document(&self) -> &Document<P> {
        &self.doc
    }

    /// Get a mutable reference to the underlying document.
    #[inline]
    pub fn document_mut(&mut self) -> &mut Document<P> {
        &mut self.doc
    }

    /// Consume the pipeline and return the document.
    #[inline]
    pub fn into_inner(self) -> Document<P> {
        self.doc
    }

    /// Convert to async pipeline for validation.
    #[cfg(feature = "async")]
    #[inline]
    pub fn into_async(self) -> AsyncPipeline<P> {
        AsyncPipeline::from_pipeline(self)
    }
}

impl<P: PhaseExt> From<Document<P>> for Pipeline<P> {
    #[inline]
    fn from(doc: Document<P>) -> Self {
        Self::new(doc)
    }
}

impl<P: PhaseExt> From<Pipeline<P>> for Document<P> {
    #[inline]
    fn from(pipeline: Pipeline<P>) -> Self {
        pipeline.into_inner()
    }
}

// =============================================================================
// AsyncPipeline (async feature)
// =============================================================================

#[cfg(feature = "async")]
mod async_impl {
    use std::future::Future;
    use std::pin::Pin;

    use futures_util::future::join_all;

    use crate::core::PhaseExt;
    use crate::node::Document;

    use super::super::error::{ValidateError, ValidateErrors};
    use super::super::{IdentityTransform, Transform, Validator};
    use super::Pipeline;

    type PendingValidation = Pin<Box<dyn Future<Output = Result<(), ValidateError>> + Send>>;

    /// Async pipeline for document processing with validation.
    ///
    /// Extends [`Pipeline`] with async validation capabilities.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let result = Pipeline::new(doc)
    ///     .pipe(indexer)
    ///     .pipe(processor)
    ///     .into_async()
    ///     .validate(LinkValidator::new(client))
    ///     .finish()
    ///     .await?;
    /// ```
    pub struct AsyncPipeline<P: PhaseExt> {
        inner: Pipeline<P>,
        pending: Vec<PendingValidation>,
    }

    impl<P: PhaseExt> AsyncPipeline<P> {
        /// Create a new async pipeline from a document.
        #[inline]
        pub fn new(doc: Document<P>) -> Self {
            Self::from_pipeline(Pipeline::new(doc))
        }

        /// Create from an existing pipeline.
        #[inline]
        pub fn from_pipeline(pipeline: Pipeline<P>) -> Self {
            Self {
                inner: pipeline,
                pending: Vec::new(),
            }
        }

        /// Apply a synchronous transform.
        #[inline]
        pub fn pipe<T>(self, transform: T) -> AsyncPipeline<T::To>
        where
            T: Transform<P>,
        {
            AsyncPipeline {
                inner: self.inner.pipe(transform),
                pending: self.pending,
            }
        }

        /// Conditionally apply a transform.
        ///
        /// Only applies the transform if `condition` is true.
        #[inline]
        pub fn pipe_if<T>(self, condition: bool, transform: T) -> AsyncPipeline<P>
        where
            T: Transform<P, To = P>,
        {
            if condition {
                self.pipe(transform)
            } else {
                self.pipe(IdentityTransform)
            }
        }

        /// Conditionally apply one of two transforms.
        ///
        /// Applies `then_transform` if condition is true, otherwise `else_transform`.
        /// Both transforms must produce the same output phase.
        #[inline]
        pub fn pipe_if_else<T1, T2>(self, condition: bool, then_transform: T1, else_transform: T2) -> AsyncPipeline<T1::To>
        where
            T1: Transform<P>,
            T2: Transform<P, To = T1::To>,
        {
            if condition {
                self.pipe(then_transform)
            } else {
                self.pipe(else_transform)
            }
        }

        /// Inspect the document without consuming the pipeline.
        ///
        /// Useful for logging, debugging, or caching intermediate state.
        #[inline]
        pub fn inspect<F>(self, f: F) -> Self
        where
            F: FnOnce(&Document<P>),
        {
            f(self.inner.document());
            self
        }

        /// Conditionally inspect the document.
        ///
        /// Only calls the closure if `condition` is true.
        #[inline]
        pub fn inspect_if<F>(self, condition: bool, f: F) -> Self
        where
            F: FnOnce(&Document<P>),
        {
            if condition {
                f(self.inner.document());
            }
            self
        }

        /// Add an async validation.
        ///
        /// The validator's `collect()` runs immediately (synchronously).
        /// The `validate()` future is awaited when `finish()` is called.
        /// Multiple validations run concurrently.
        ///
        /// # Example
        ///
        /// ```ignore
        /// pipeline
        ///     .into_async()
        ///     .validate(LinkValidator::new(client))
        ///     .validate(ImageValidator::new())
        ///     .finish()
        ///     .await?;
        /// ```
        pub fn validate<V>(mut self, validator: V) -> Self
        where
            V: Validator<P> + 'static,
            V::Error: std::fmt::Display,
        {
            let items: Vec<_> = validator.collect(self.inner.document()).into_iter().collect();
            let validator_name = std::any::type_name::<V>().to_string();

            let future = async move {
                validator
                    .validate(items)
                    .await
                    .map_err(|e| ValidateError::new(validator_name, e.to_string()))
            };

            self.pending.push(Box::pin(future));
            self
        }

        /// Conditionally add an async validation.
        ///
        /// Only adds the validator if `condition` is true.
        /// This is more ergonomic than manual branching.
        ///
        /// # Example
        ///
        /// ```ignore
        /// pipeline
        ///     .into_async()
        ///     .validate(AssetValidator::new(dir))
        ///     .validate_if(check_external, ExternalLinkValidator::new())
        ///     .finish()
        ///     .await?;
        /// ```
        #[inline]
        pub fn validate_if<V>(self, condition: bool, validator: V) -> Self
        where
            V: Validator<P> + 'static,
            V::Error: std::fmt::Display,
        {
            if condition {
                self.validate(validator)
            } else {
                self
            }
        }

        /// Finish the pipeline, awaiting all validations concurrently.
        ///
        /// If no validators were added, returns immediately without async overhead.
        pub async fn finish(self) -> Result<Document<P>, ValidateErrors> {
            // Fast path: no validations pending
            if self.pending.is_empty() {
                return Ok(self.inner.into_inner());
            }

            let results = join_all(self.pending).await;

            let mut errors = ValidateErrors::new();
            for result in results {
                if let Err(e) = result {
                    errors.push(e);
                }
            }

            if errors.is_empty() {
                Ok(self.inner.into_inner())
            } else {
                Err(errors)
            }
        }

        /// Finish with fail-fast behavior: stop on first error.
        ///
        /// Unlike `finish()` which runs all validations concurrently and collects
        /// all errors, this method stops as soon as any validator fails.
        ///
        /// Use when:
        /// - Validations are expensive and you want to bail early
        /// - You only need to know if validation passed, not all failures
        pub async fn finish_fail_fast(self) -> Result<Document<P>, ValidateError> {
            if self.pending.is_empty() {
                return Ok(self.inner.into_inner());
            }

            // Check results sequentially, returning on first error
            let results = join_all(self.pending).await;

            for result in results {
                if let Err(e) = result {
                    return Err(e);
                }
            }

            Ok(self.inner.into_inner())
        }

        /// Finish and return document along with any errors.
        ///
        /// This allows continuing with the document even when validation fails.
        /// Useful for "best effort" scenarios where partial results are acceptable.
        ///
        /// # Returns
        /// - `(doc, None)` if all validations passed
        /// - `(doc, Some(errors))` if any validations failed
        pub async fn finish_with_doc(self) -> (Document<P>, Option<ValidateErrors>) {
            if self.pending.is_empty() {
                return (self.inner.into_inner(), None);
            }

            let results = join_all(self.pending).await;

            let mut errors = ValidateErrors::new();
            for result in results {
                if let Err(e) = result {
                    errors.push(e);
                }
            }

            let doc = self.inner.into_inner();
            if errors.is_empty() {
                (doc, None)
            } else {
                (doc, Some(errors))
            }
        }

        /// Finish without waiting for validations.
        #[inline]
        pub fn finish_unchecked(self) -> Document<P> {
            self.inner.into_inner()
        }

        /// Get a reference to the document.
        #[inline]
        pub fn document(&self) -> &Document<P> {
            self.inner.document()
        }

        /// Number of pending validations.
        #[inline]
        pub fn pending_count(&self) -> usize {
            self.pending.len()
        }
    }

    impl<P: PhaseExt> From<Document<P>> for AsyncPipeline<P> {
        #[inline]
        fn from(doc: Document<P>) -> Self {
            Self::new(doc)
        }
    }

    impl<P: PhaseExt> From<Pipeline<P>> for AsyncPipeline<P> {
        #[inline]
        fn from(pipeline: Pipeline<P>) -> Self {
            Self::from_pipeline(pipeline)
        }
    }
}

#[cfg(feature = "async")]
pub use async_impl::AsyncPipeline;

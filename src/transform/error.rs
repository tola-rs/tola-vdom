//! Validation error types.
//!
//! Only available with the `async` feature.

use thiserror::Error;

use crate::id::StableId;

/// Error from a failed validation.
#[derive(Debug, Error)]
#[error("{validator}: {message}{}", format_context(.node_id, .source_hint))]
pub struct ValidateError {
    /// Name/description of the validator that failed.
    pub validator: String,
    /// The error message.
    pub message: String,
    /// The StableId of the node that caused the error (if applicable).
    pub node_id: Option<StableId>,
    /// Source hint for locating the error (e.g., line number, file path).
    pub source_hint: Option<String>,
}

fn format_context(node_id: &Option<StableId>, source_hint: &Option<String>) -> String {
    let mut ctx = String::new();
    if let Some(id) = node_id {
        ctx.push_str(&format!(" [node: {}]", id));
    }
    if let Some(hint) = source_hint {
        ctx.push_str(&format!(" ({})", hint));
    }
    ctx
}

impl ValidateError {
    /// Create a new validation error.
    pub fn new(validator: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            validator: validator.into(),
            message: message.into(),
            node_id: None,
            source_hint: None,
        }
    }

    /// Attach the node ID that caused this error.
    pub fn with_node(mut self, id: StableId) -> Self {
        self.node_id = Some(id);
        self
    }

    /// Attach a source hint (e.g., line number, file path).
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.source_hint = Some(hint.into());
        self
    }
}

/// Errors collected from all failed validations.
#[derive(Debug, Default, Error)]
#[error("{} validation error(s):\n{}", self.errors.len(), format_errors(&self.errors))]
pub struct ValidateErrors {
    /// List of individual validation errors.
    pub errors: Vec<ValidateError>,
}

fn format_errors(errors: &[ValidateError]) -> String {
    errors
        .iter()
        .map(|e| format!("  - {}", e))
        .collect::<Vec<_>>()
        .join("\n")
}

impl ValidateErrors {
    /// Create empty errors.
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Check if there are any errors.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Number of errors.
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Add an error.
    pub fn push(&mut self, error: ValidateError) {
        self.errors.push(error);
    }

    /// Iterate over errors.
    pub fn iter(&self) -> impl Iterator<Item = &ValidateError> {
        self.errors.iter()
    }

    /// Get errors by node ID.
    pub fn by_node(&self, id: StableId) -> impl Iterator<Item = &ValidateError> {
        self.errors.iter().filter(move |e| e.node_id == Some(id))
    }

    /// Get errors by validator name (substring match).
    pub fn by_validator(&self, name: &str) -> impl Iterator<Item = &ValidateError> {
        let name = name.to_string();
        self.errors
            .iter()
            .filter(move |e| e.validator.contains(&name))
    }

    /// Convert to a single error by taking the first one.
    ///
    /// Useful when you need a single `ValidateError` instead of a collection.
    /// The error message will include the count if there were multiple errors.
    ///
    /// # Panics
    ///
    /// Panics if there are no errors. Use `is_empty()` to check first.
    pub fn into_single(self) -> ValidateError {
        let count = self.errors.len();
        let mut errors = self.errors;
        let mut first = errors.remove(0);

        if count > 1 {
            first.message = format!("{} (+{} more errors)", first.message, count - 1);
        }

        first
    }
}

impl IntoIterator for ValidateErrors {
    type Item = ValidateError;
    type IntoIter = std::vec::IntoIter<ValidateError>;

    fn into_iter(self) -> Self::IntoIter {
        self.errors.into_iter()
    }
}

impl<'a> IntoIterator for &'a ValidateErrors {
    type Item = &'a ValidateError;
    type IntoIter = std::slice::Iter<'a, ValidateError>;

    fn into_iter(self) -> Self::IntoIter {
        self.errors.iter()
    }
}

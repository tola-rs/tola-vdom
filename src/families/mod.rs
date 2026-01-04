//! Predefined families for common HTML elements.
//!
//! These families use the same `Family` trait as user-defined families.
//! They are "predefined" for convenience, not "built-in" with special treatment.
//!
//! # Available Families
//!
//! - `LinkFamily` - `<a>`, elements with `href`/`src`
//! - `HeadingFamily` - `<h1>` through `<h6>`
//! - `SvgFamily` - SVG elements
//! - `MediaFamily` - `<img>`, `<video>`, `<audio>`
//!
//! # Usage
//!
//! ```ignore
//! #[vdom::phase]
//! pub struct MySite {
//!     link: vdom::families::LinkFamily,
//!     heading: vdom::families::HeadingFamily,
//!     // ... add your own families
//! }
//! ```

pub mod link;
pub mod heading;
pub mod svg;
pub mod media;

pub use link::*;
pub use heading::*;
pub use svg::*;
pub use media::*;

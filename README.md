# tola-vdom

Type-safe Virtual DOM with multi-phase transformations.

> **Note**: This library is primarily designed for internal use by [tola-ssg](https://github.com/tola-rs/tola-ssg). While it can be used independently, the API is tailored for tola-ssg's hot reload and incremental update needs.

## Overview

tola-vdom provides a typed VDOM system with content-based identity tracking for efficient incremental updates:

- **StableId**: Content-hash identity for each node
- **Multi-phase pipeline**: Raw → Indexed → Processed
- **Family system**: Type-safe element classification (Link, Heading, Svg, Media, custom)
- **Efficient diffing**: Incremental updates with move detection

## Architecture

```
  Document<Raw>          Document<Indexed>        Document<Processed>
  ┌──────────────┐       ┌──────────────┐         ┌──────────┐
  │ Parser output│──────▶│ StableId per │────────▶│ Ready to │
  │ SourceSpan   │       │ node         │         │ render   │
  └──────────────┘       │ **Cacheable**│         └──────────┘
                         └──────────────┘                │
                                │                        │
                              diff()                  render()
                                │                        │
                                ▼                        ▼
                         ┌────────────┐          HTML String
                         │ PatchOp[]  │
                         └────────────┘
```

## Usage

### Define Families

Use `#[vdom::families]` to define your site's element families:

```rust
use tola_vdom::families::{HeadingFamily, LinkFamily, MediaFamily, SvgFamily};
use tola_vdom::vdom::{families, family, processed};

// Custom family with processed data
#[processed(Math)]
pub struct MathProcessed {
    pub html: String,
}

#[family(processed = MathProcessed)]
pub struct Math {
    pub formula: String,
    pub display: bool,
}

// Combine all families
#[families]
pub struct MySite {
    link: LinkFamily,
    heading: HeadingFamily,
    svg: SvgFamily,
    media: MediaFamily,
    math: MathFamily,  // custom
}
```

This generates:
- Phase types: `MySite::Raw`, `MySite::Indexed`, `MySite::Processed`
- Extension enums: `MySite::RawExt`, `MySite::IndexedExt`, `MySite::ProcessedExt`
- Helper functions: `MySite::indexer()`, `MySite::processor()`, `MySite::identify()`, `MySite::element()`

### Pipeline

```rust
use tola_vdom::prelude::*;

// Raw → Indexed → Processed
let indexed = Pipeline::new(raw_doc)
    .pipe(MySite::indexer().with_page_seed(PageSeed::from_path("/page")))
    .pipe(LinkTransform::new(config))
    .inspect(|doc| { /* inspect intermediate state */ })
    .into_inner();

let processed = Pipeline::new(indexed)
    .pipe(MySite::processor())
    .into_inner();

// Render to HTML
let html = render_document(&processed, &RenderConfig::DEV);
```

### Diffing & Hot Reload

```rust
use tola_vdom::algo::diff;

// Cache indexed VDOM
let key = CacheKey::new("/page");
cache.insert(key.clone(), CacheEntry::new(indexed.clone()));

// On file change: diff and patch
let result = diff(&cached.doc, &new_indexed);

if result.should_reload {
    // Full page reload needed
} else if result.ops.is_empty() {
    // No changes
} else {
    let patches = render_patches(&result.ops, &RenderConfig::DEV);
    // Send patches via WebSocket
}
```

### Persistence (rkyv)

```rust
use tola_vdom::serialize::{to_bytes, from_bytes};

// Save to disk
let bytes = to_bytes(&indexed_doc)?;
fs::write("cache.vdom", bytes)?;

// Restore from disk
let bytes = fs::read("cache.vdom")?;
let doc: Document<MySite::Indexed> = from_bytes(&bytes)?;
```

## Modules

| Module | Description |
|--------|-------------|
| `core` | Core traits: `Family`, `Phase`, `PhaseExt`, `HasStableId` |
| `node` | Node types: `Document`, `Element`, `Text`, `Node` |
| `families` | Built-in families: Link, Heading, Svg, Media |
| `transform` | Pipeline: `Indexer`, `Processor`, `Transform` |
| `algo` | Diff algorithm |
| `render` | HTML rendering with optional stable IDs |
| `cache` | Thread-safe VDOM cache |
| `serialize` | rkyv serialization for persistence |

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `cache` | ✓ | rkyv zero-copy serialization |
| `macros` | ✓ | `#[vdom::families]` proc macro |
| `async` | ✓ | Async validation pipeline |
| `parallel` | | Rayon parallel batch operations |

## Requirements

- Rust 1.85+ (Edition 2024)
- Uses GATs (Generic Associated Types)

## License

MIT

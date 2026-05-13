# o3gen

Generate idiomatic, IDE-friendly Rust types from OpenAPI 3.0 specifications.

## Quick Start

Add `o3gen` to your `Cargo.toml`:

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }

[build-dependencies]
o3gen = "0.1"
```

```rust
// build.rs
fn main() {
    o3gen::Generator::builder("openapi.json")
        .rename("User", "AppUser")
        .write_to_out_dir("types.rs")
        .expect("Failed to generate types");
}
```


```rust
include!(concat!(env!("OUT_DIR"), "/types.rs"));
use types::*; // types are generated inside a `types` module

fn main() {
    let user = AppUser { id: 1, name: "Alice".into(), status: None };
    println!("Hello, {}!", user.name);
}
```

## Features

- **Full IDE Support**: Works perfectly with `rust-analyzer` (Go to Definition, Auto-complete).
- **Fluent API**: Easy renaming and extra derives (`.rename("A", "B").derive_extra("B", ["Hash"])`).
- **Idiomatic Rust**: Handles `anyOf` safely, wraps optional fields in `Option`, and implements `as_str()`/`Display`.

## License

MIT

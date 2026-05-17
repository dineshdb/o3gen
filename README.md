# o3gen

Generate idiomatic, IDE-friendly Rust types from OpenAPI 3.0 specifications.

## Quick Start

Add `o3gen` to your `Cargo.toml`:

```toml
[build-dependencies]
o3gen = "0.6"
```

```rust
// build.rs
fn main() {
    o3gen::Generator::builder("openapi.json")
        .rename("Pet", "Dog")
        .rename("CreateUserRequest", "SignupRequest")
        .write_to_out_dir("types.rs")
        .expect("Failed to generate types");
}
```

```rust
include!(concat!(env!("OUT_DIR"), "/types.rs"));
use types::*;

fn main() {
    let user = SignupRequest::builder()
        .name("Alice".to_string())
        .email("alice@example.com".to_string())
        .build()
        .unwrap();
    println!("Hello, {}!", user.name);
}
```

## Features

- **Smart enums**: `anyOf`/`oneOf` become untagged enums with generated `From` impls for each variant — pass inner types directly, no wrapping needed.
- **Full IDE Support**: Works with `rust-analyzer` (Go to Definition, Auto-complete, refactoring).
- **Fluent API**: Rename types, add extra derives, control serde behaviour.
- **Builder pattern**: Every struct gets a `derive_builder`-based builder with `.into()` setters.
- **Idiomatic Rust**: Optional fields become `Option<T>`, string enums get `FromStr`/`Display`/`From<&str>`, validation is generated.

## License

MIT

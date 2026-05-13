# o3gen

A proc-macro to generate Rust types from OpenAPI 3.0 specifications with support for renaming and opt-in derives.

## Features

- **Type Renaming**: Map OpenAPI schema names to idiomatic Rust names.
- **Opt-in Derives**: Add `Eq`, `PartialOrd`, `Ord`, and `Hash` to specific types where valid.
- **Smart anyOf Handling**: Generates untagged enums for `anyOf` components with support for string-like variants.
- **Comprehensive Defaults**: All types derive `Debug`, `Clone`, `Serialize`, `Deserialize`, `PartialEq`, and `Default` by default.

## Usage

Add `o3gen` to your `Cargo.toml`:

```toml
[dependencies]
o3gen = "0.1.0"
```

In your Rust code:

```rust
mod generated {
    o3gen::generate_types! {
        path = "path/to/openapi.json",
        rename = {
            "OriginalSchemaName" => "IdiomaticName",
            "AnotherSchema" => "BetterName"
        },
        derive_extra = {
            "IdiomaticName" => ["Eq", "PartialOrd", "Ord", "Hash"]
        }
    }
}

pub use generated::types::*;
```

## License

This project is licensed under the MIT License.

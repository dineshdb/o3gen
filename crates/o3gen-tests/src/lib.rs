#[allow(dead_code, clippy::all, clippy::pedantic, clippy::nursery)]
pub mod types {
    include!(concat!(env!("OUT_DIR"), "/types.rs"));
}

#[allow(dead_code, clippy::all, clippy::pedantic, clippy::nursery)]
pub mod renamed_types {
    include!(concat!(env!("OUT_DIR"), "/renamed_types.rs"));
}

#[cfg(test)]
pub mod integration_test;

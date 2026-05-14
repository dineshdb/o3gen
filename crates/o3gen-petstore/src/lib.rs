#[allow(dead_code, clippy::all, clippy::pedantic, clippy::nursery)]
pub mod types {
    include!(concat!(env!("OUT_DIR"), "/types.rs"));
}

#[cfg(test)]
pub mod tests;

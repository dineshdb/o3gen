use o3gen::Generator;

fn main() {
    // 1. Standard types
    Generator::builder("fixtures/test_spec.json")
        .write_to_out_dir("types.rs")
        .expect("Failed to generate types.rs");

    // 2. Renamed types
    Generator::builder("fixtures/test_spec.json")
        .rename("User", "AppUser")
        .rename("Role", "AppRole")
        .rename("Status", "AppStatus")
        .rename("Admin", "SuperUser")
        .rename("ComplexAnyOfSubtype1", "SpecialVariantA")
        .derive_extra("AppUser", ["Hash", "Eq"])
        .derive_extra("AppStatus", ["Hash", "Eq"])
        .write_to_out_dir("renamed_types.rs")
        .expect("Failed to generate renamed_types.rs");

    println!("cargo:rerun-if-changed=build.rs");
}

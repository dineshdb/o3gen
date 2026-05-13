use o3gen::Generator;

fn main() {
    // Generate types from the comprehensive spec
    Generator::builder("fixtures/comprehensive_spec.json")
        .write_to_out_dir("types.rs")
        .expect("Failed to generate types.rs");

    // Generate renamed types from the same comprehensive spec
    Generator::builder("fixtures/comprehensive_spec.json")
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
    println!("cargo:rerun-if-changed=fixtures/comprehensive_spec.json");
}

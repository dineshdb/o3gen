use o3gen::Generator;

fn main() {
    // Generate types from petstore OpenAPI with NewPet renamed to NewPetStore
    Generator::builder("fixtures/petstore-openapi.json")
        .rename("NewPet", "NewPetStore")
        .write_to_out_dir("types.rs")
        .expect("Failed to generate types.rs");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=fixtures/petstore-openapi.json");
}

use crate::types;
use crate::types::{Pet, PetStatus, Species};
use chrono::Utc;

#[test]
fn test_rename_category_to_pet_category() {
    // This verifies that `Category` was successfully renamed to `PetCategory`
    let _category = types::PetCategory {
        id: "1".to_string(),
        name: "Dogs".to_string(),
    };
}

#[test]
fn test_pet_builder_validation() {
    // Valid pet
    let pet = Pet::builder()
        .id("123")
        .name("Fido")
        .species(Species::Dog)
        .status(PetStatus::Available)
        .age_months(24)
        .price("100.00")
        .currency("USD")
        .created_at(Utc::now())
        .updated_at(Utc::now())
        .build()
        .unwrap();

    assert_eq!(pet.name, "Fido");
    assert_eq!(pet.species, Species::Dog);

    // Invalid pet (name too short)
    let result = Pet::builder()
        .id("123")
        .name("") // invalid: too short (min = 1 in petstore.json)
        .species(Species::Dog)
        .status(PetStatus::Available)
        .age_months(24)
        .price("100.00")
        .currency("USD")
        .created_at(Utc::now())
        .updated_at(Utc::now())
        .build();

    assert!(
        result.is_err(),
        "Builder should fail for invalid pet (empty name)"
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("name"),
        "Error should mention 'name'"
    );
}

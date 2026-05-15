use crate::types;
use crate::types::{Pet, PetSpecies, PetStatus};
use chrono::Utc;

#[test]
fn test_rename_category_to_pet_category() {
    // This verifies that `Category` was successfully renamed to `PetCategory`
    let category = types::PetCategory {
        id: "cat_1".to_string(),
        name: "Dogs".to_string(),
    };

    assert_eq!(category.id, "cat_1");
    assert_eq!(category.name, "Dogs");

    assert_eq!(category.id, "cat_1");
    assert_eq!(category.name, "Dogs");
}

#[test]
fn test_pet_builder_validation() {
    // Valid pet
    let pet = Pet::builder()
        .id("123")
        .name("Fido")
        .species(PetSpecies::Dog)
        .status(PetStatus::Available)
        .age_months(24)
        .price("100.00")
        .currency("USD")
        .created_at(Utc::now())
        .updated_at(Utc::now())
        .photos(vec!["http://example.com/photo.jpg".to_string()])
        .build()
        .expect("Builder should succeed for valid pet");

    assert_eq!(pet.name, "Fido");

    // Invalid pet (name too short)
    let result = Pet::builder()
        .id("123")
        .name("") // invalid: too short (min = 1 in petstore.json)
        .species(PetSpecies::Dog)
        .status(PetStatus::Available)
        .age_months(24)
        .price("100.00")
        .currency("USD")
        .created_at(Utc::now())
        .updated_at(Utc::now())
        .photos(Vec::<String>::new())
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

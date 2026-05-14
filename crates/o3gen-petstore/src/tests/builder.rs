use crate::types::types;

#[test]
fn test_rename_category_to_pet_category() {
    // This verifies that `Category` was successfully renamed to `PetCategory`
    let category = types::PetCategory {
        id: "cat_1".to_string(),
        name: "Dogs".to_string(),
    };

    assert_eq!(category.id, "cat_1");
    assert_eq!(category.name, "Dogs");

    let json = serde_json::to_string(&category).expect("failed to serialize");
    assert!(json.contains("\"id\":\"cat_1\""));
    assert!(json.contains("\"name\":\"Dogs\""));

    let roundtrip: types::PetCategory = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.id, category.id);
    assert_eq!(roundtrip.name, category.name);
}

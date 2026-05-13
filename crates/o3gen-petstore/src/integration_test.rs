use crate::types::types;

#[test]
fn test_pet_with_required_fields() {
    let pet = types::Pet {
        id: 1,
        name: "doggie".to_string(),
        tag: None,
    };
    assert_eq!(pet.id, 1);
    assert_eq!(pet.name, "doggie");
}

#[test]
fn test_pet_tag_field() {
    let pet = types::Pet {
        id: 1,
        name: "doggie".to_string(),
        tag: Some("friendly".to_string()),
    };
    assert_eq!(pet.tag.as_ref().unwrap(), "friendly");
}

#[test]
fn test_new_pet_fields() {
    let new_pet = types::NewPetStore {
        name: "doggie".to_string(),
        tag: Some("friendly".to_string()),
    };
    assert_eq!(new_pet.name, "doggie");
    assert_eq!(new_pet.tag.as_ref().unwrap(), "friendly");
}

#[test]
fn test_error_with_required_fields() {
    let error = types::Error {
        code: 404,
        message: "Not found".to_string(),
    };
    assert_eq!(error.code, 404);
    assert_eq!(error.message, "Not found");
}

#[test]
fn test_json_serialization_pet() {
    let pet = types::Pet {
        id: 1,
        name: "doggie".to_string(),
        tag: None,
    };

    let json = serde_json::to_string(&pet).expect("failed to serialize");
    assert!(json.contains("\"id\":1"));
    assert!(json.contains("\"name\":\"doggie\""));

    let roundtrip: types::Pet = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.id, pet.id);
    assert_eq!(roundtrip.name, pet.name);
}

#[test]
fn test_json_serialization_new_pet() {
    let new_pet = types::NewPetStore {
        name: "doggie".to_string(),
        tag: Some("friendly".to_string()),
    };

    let json = serde_json::to_string(&new_pet).expect("failed to serialize");
    assert!(json.contains("\"name\":\"doggie\""));
    assert!(json.contains("\"tag\":\"friendly\""));

    let roundtrip: types::NewPetStore = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.name, new_pet.name);
    assert_eq!(roundtrip.tag, new_pet.tag);
}

#[test]
fn test_json_serialization_error() {
    let error = types::Error {
        code: 404,
        message: "Not found".to_string(),
    };

    let json = serde_json::to_string(&error).expect("failed to serialize");
    assert!(json.contains("\"code\":404"));
    assert!(json.contains("\"message\":\"Not found\""));

    let roundtrip: types::Error = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.code, error.code);
    assert_eq!(roundtrip.message, error.message);
}

#[test]
fn test_full_pet_with_tag() {
    let pet = types::Pet {
        id: 1,
        name: "doggie".to_string(),
        tag: Some("friendly".to_string()),
    };

    let json = serde_json::to_string(&pet).expect("failed to serialize");
    assert!(json.contains("\"tag\":\"friendly\""));

    let roundtrip: types::Pet = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.tag, Some("friendly".to_string()));
}

#[test]
fn test_pet_with_optional_tag() {
    let pet = types::Pet {
        id: 1,
        name: "doggie".to_string(),
        tag: None,
    };

    let json = serde_json::to_string(&pet).expect("failed to serialize");
    assert!(json.contains("\"tag\":null"));

    let roundtrip: types::Pet = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.tag, None);
}

#[test]
fn test_new_pet_with_optional_tag() {
    let new_pet = types::NewPetStore {
        name: "doggie".to_string(),
        tag: None,
    };

    let json = serde_json::to_string(&new_pet).expect("failed to serialize");
    assert!(json.contains("\"tag\":null"));

    let roundtrip: types::NewPetStore = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.tag, None);
}

#[test]
fn test_compare_pets() {
    let pet1 = types::Pet {
        id: 1,
        name: "doggie".to_string(),
        tag: Some("friendly".to_string()),
    };

    let pet2 = types::Pet {
        id: 1,
        name: "doggie".to_string(),
        tag: Some("friendly".to_string()),
    };

    assert_eq!(pet1, pet2);
}

#[test]
fn test_compare_different_pets() {
    let pet1 = types::Pet {
        id: 1,
        name: "doggie".to_string(),
        tag: Some("friendly".to_string()),
    };

    let pet2 = types::Pet {
        id: 2,
        name: "cat".to_string(),
        tag: Some("playful".to_string()),
    };

    assert_ne!(pet1, pet2);
}

#[test]
fn test_cloning_pet() {
    let original = types::Pet {
        id: 1,
        name: "doggie".to_string(),
        tag: Some("friendly".to_string()),
    };

    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn test_cloning_error() {
    let original = types::Error {
        code: 404,
        message: "Not found".to_string(),
    };

    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn test_optional_fields() {
    let pet = types::Pet {
        id: 1,
        name: "doggie".to_string(),
        tag: None,
    };

    let json = serde_json::to_string(&pet).expect("failed to serialize");
    assert!(json.contains("\"tag\":null"));
}

#[test]
fn test_serialize_with_defaults() {
    let pet = types::Pet {
        id: 1,
        name: "doggie".to_string(),
        tag: None,
    };

    let json = serde_json::to_string(&pet).expect("failed to serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("failed to parse");

    assert_eq!(parsed.get("id").unwrap(), &serde_json::json!(1));
    assert_eq!(parsed.get("name").unwrap(), &serde_json::json!("doggie"));
}

#[test]
fn test_error_code_zero() {
    let error = types::Error {
        code: 0,
        message: "OK".to_string(),
    };

    let json = serde_json::to_string(&error).expect("failed to serialize");
    assert!(json.contains("\"code\":0"));

    let roundtrip: types::Error = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.code, error.code);
}

#[test]
fn test_error_empty_message() {
    let error = types::Error {
        code: 500,
        message: "".to_string(),
    };

    let json = serde_json::to_string(&error).expect("failed to serialize");
    assert!(json.contains("\"code\":500"));
    assert!(json.contains("\"message\":\"\""));

    let roundtrip: types::Error = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.code, error.code);
    assert_eq!(roundtrip.message, error.message);
}

#[test]
fn test_pet_id_with_zero() {
    let pet = types::Pet {
        id: 0,
        name: "doggie".to_string(),
        tag: None,
    };

    let json = serde_json::to_string(&pet).expect("failed to serialize");
    assert!(json.contains("\"id\":0"));

    let roundtrip: types::Pet = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.id, pet.id);
}

#[test]
fn test_pet_name_with_special_chars() {
    let pet = types::Pet {
        id: 1,
        name: "dog-gie!".to_string(),
        tag: Some("friendly".to_string()),
    };

    let json = serde_json::to_string(&pet).expect("failed to serialize");
    assert!(json.contains("\"name\":\"dog-gie!\""));

    let roundtrip: types::Pet = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.name, pet.name);
}

#[test]
fn test_error_with_large_code() {
    let error = types::Error {
        code: 999999,
        message: "Custom error".to_string(),
    };

    let json = serde_json::to_string(&error).expect("failed to serialize");
    assert!(json.contains("\"code\":999999"));

    let roundtrip: types::Error = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.code, error.code);
}

#[test]
fn test_pet_with_long_name() {
    let pet = types::Pet {
        id: 1,
        name: "this-is-a-very-long-pet-name-for-testing-purposes".to_string(),
        tag: Some("friendly".to_string()),
    };

    let json = serde_json::to_string(&pet).expect("failed to serialize");
    assert!(json.contains("\"name\":\"this-is-a-very-long-pet-name-for-testing-purposes\""));

    let roundtrip: types::Pet = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.name, pet.name);
}

#[test]
fn test_error_with_unicode_message() {
    let error = types::Error {
        code: 404,
        message: "Not found 🐕".to_string(),
    };

    let json = serde_json::to_string(&error).expect("failed to serialize");
    assert!(json.contains("\"message\":\"Not found 🐕\""));

    let roundtrip: types::Error = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.message, error.message);
}

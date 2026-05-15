use crate::types;

#[test]
fn test_json_serialization_pet() {
    let pet = types::Pet {
        id: "123".to_string(),
        name: "doggie".to_string(),
        species: types::PetSpecies::Dog,
        status: types::PetStatus::Available,
        price: "100.00".to_string(),
        currency: "USD".to_string(),
        created_at: chrono::DateTime::from_timestamp(1736412000, 0).unwrap(),
        updated_at: chrono::DateTime::from_timestamp(1736412000, 0).unwrap(),
        age_months: 12,
        ..Default::default()
    };

    let json = serde_json::to_string(&pet).expect("failed to serialize");
    assert!(json.contains("\"id\":\"123\""));
    assert!(json.contains("\"name\":\"doggie\""));

    let roundtrip: types::Pet = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.id, pet.id);
    assert_eq!(roundtrip.name, pet.name);
}

#[test]
fn test_json_serialization_error() {
    let error = types::Error {
        status: 404,
        title: "Not found".to_string(),
        r#type: "about:blank".to_string(),
        ..Default::default()
    };

    let json = serde_json::to_string(&error).expect("failed to serialize");
    assert!(json.contains("\"status\":404"));
    assert!(json.contains("\"title\":\"Not found\""));

    let roundtrip: types::Error = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(roundtrip.status, error.status);
    assert_eq!(roundtrip.title, error.title);
}

#[test]
fn test_validation() {
    use validator::Validate;
    let pet = types::Pet {
        id: "123".to_string(),
        name: "".to_string(), // Too short, min is 1
        age_months: -1,       // Too small, min is 0
        ..Default::default()
    };
    let result = pet.validate();
    assert!(result.is_err());
}

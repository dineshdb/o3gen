use crate::types::types;

#[test]
fn test_pet_with_required_fields() {
    let pet = types::Pet {
        id: "123".to_string(),
        name: "doggie".to_string(),
        species: "dog".to_string(),
        status: "available".to_string(),
        price: "100.00".to_string(),
        currency: "USD".to_string(),
        created_at: "2023-01-01T00:00:00Z".to_string(),
        updated_at: "2023-01-01T00:00:00Z".to_string(),
        age_months: 12,
        ..Default::default()
    };
    assert_eq!(pet.id, "123");
    assert_eq!(pet.name, "doggie");
    assert_eq!(pet.age_months, 12);
}

#[test]
fn test_error_with_reserved_keyword_field() {
    let error = types::Error {
        status: 404,
        title: "Not found".to_string(),
        r#type: "about:blank".to_string(),
        ..Default::default()
    };
    assert_eq!(error.status, 404);
    assert_eq!(error.title, "Not found");
    assert_eq!(error.r#type, "about:blank");
}

use crate::types::types;

#[test]
fn test_compare_pets() {
    let pet1 = types::Pet {
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

    let pet2 = types::Pet {
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

    assert_eq!(pet1, pet2);
}

#[test]
fn test_compare_different_pets() {
    let pet1 = types::Pet {
        id: "1".to_string(),
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

    let pet2 = types::Pet {
        id: "2".to_string(),
        name: "cat".to_string(),
        species: "cat".to_string(),
        status: "available".to_string(),
        price: "50.00".to_string(),
        currency: "USD".to_string(),
        created_at: "2023-01-01T00:00:00Z".to_string(),
        updated_at: "2023-01-01T00:00:00Z".to_string(),
        age_months: 24,
        ..Default::default()
    };

    assert_ne!(pet1, pet2);
}

#[test]
fn test_cloning_pet() {
    let original = types::Pet {
        id: "1".to_string(),
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

    let cloned = original.clone();
    assert_eq!(original, cloned);
}

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

include!(concat!(env!("OUT_DIR"), "/renamed_types.rs"));
use types::*;

#[test]
fn test_renaming() {
    // User should be renamed to AppUser
    let user = AppUser {
        id: 1,
        name: "Alice".to_string(),
        status: Some(AppStatus::Active),
    };
    assert_eq!(user.name, "Alice");
    assert_eq!(user.status.as_ref().unwrap().as_str(), "active");

    // Role should be renamed to AppRole
    let role = AppRole::from(user);
    match role {
        AppRole::AppUser(_) => (),
        _ => panic!("Expected AppRole::AppUser"),
    }
}

#[test]
fn test_nested_and_subtype_renaming() {
    // Admin should be renamed to SuperUser
    let admin = SuperUser {
        id: Some(1),
        permissions: None,
    };
    let role = AppRole::from(admin);
    match role {
        AppRole::SuperUser(_) => (),
        _ => panic!("Expected AppRole::SuperUser"),
    }

    // ComplexAnyOfSubtype1 should be renamed to SpecialVariantA
    let val_a = SpecialVariantA {
        r#type: Some("A".to_string()),
        value_a: Some(42),
    };
    let complex = ComplexAnyOf::from(val_a);
    match complex {
        ComplexAnyOf::SpecialVariantA(_) => (),
        _ => panic!("Expected ComplexAnyOf::SpecialVariantA"),
    }
}

#[test]
fn test_extra_derives() {
    let user1 = AppUser {
        id: 1,
        name: "Alice".to_string(),
        status: Some(AppStatus::Active),
    };
    let user2 = AppUser {
        id: 1,
        name: "Alice".to_string(),
        status: Some(AppStatus::Active),
    };

    // Test Eq (added via derive_extra)
    assert_eq!(user1, user2);

    // Test Hash (added via derive_extra)
    let mut s1 = DefaultHasher::new();
    user1.hash(&mut s1);
    let h1 = s1.finish();

    let mut s2 = DefaultHasher::new();
    user2.hash(&mut s2);
    let h2 = s2.finish();

    assert_eq!(h1, h2);
}

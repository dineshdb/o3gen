use crate::renamed_types::types as renamed;
use crate::types::types as normal;

#[test]
fn test_basic_object_and_enum() {
    let user = normal::User {
        id: 1,
        name: "John Doe".to_string(),
        status: Some(normal::Status::Active),
    };
    assert_eq!(user.id, 1);
    assert_eq!(normal::Status::Active.as_str(), "active");
}

#[test]
fn test_all_of_inheritance() {
    // Extended inherits id from Base via allOf
    let extended = normal::Extended {
        id: "123".to_string(),
        name: "Test".to_string(),
    };
    assert_eq!(extended.id, "123");
    assert_eq!(extended.name, "Test");
}

#[test]
fn test_any_of_enums() {
    let role = normal::Role::Admin(normal::Admin {
        id: Some(1),
        permissions: Some(vec!["read".to_string()]),
    });
    assert!(matches!(role, normal::Role::Admin(_)));
}

#[test]
fn test_type_renaming() {
    // AppUser is User renamed in config
    let user = renamed::AppUser {
        id: 1,
        name: "John Doe".to_string(),
        status: Some(renamed::AppStatus::Active),
    };
    assert_eq!(user.id, 1);
}

#[test]
fn test_extra_derives() {
    // AppUser has extra derives: Hash, Eq
    use std::collections::HashSet;
    let user = renamed::AppUser {
        id: 1,
        name: "a".to_string(),
        status: None,
    };
    let mut set = HashSet::new();
    set.insert(user.clone());
    assert!(set.contains(&user));
}

#[test]
fn test_json_serialization() {
    let s = normal::Status::Active;
    let json = serde_json::to_string(&s).expect("failed to serialize");
    assert_eq!(json, "\"active\"");
    let s2: normal::Status = serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(s, s2);
}

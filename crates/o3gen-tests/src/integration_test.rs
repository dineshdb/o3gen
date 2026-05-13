use serde_json::json;

include!(concat!(env!("OUT_DIR"), "/types.rs"));
use types::*;

#[test]
fn enum_string() {
    let active = Status::Active;
    assert_eq!(active.as_str(), "active");
    assert_eq!(active.to_string(), "active");

    let deserialized = Status::try_from("pending").unwrap();
    assert_eq!(deserialized, Status::Pending);
}

#[test]
fn enum_json() {
    let deserialized: Status = serde_json::from_str("\"pending\"").unwrap();
    assert_eq!(deserialized, Status::Pending);
    assert_eq!(deserialized.as_str(), "pending");
}

#[test]
fn test_user_struct() {
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        status: Some(Status::Active),
    };
    let json = serde_json::to_value(&user).unwrap();
    assert_eq!(json["id"], 1);
    assert_eq!(json["name"], "Alice");
    assert_eq!(json["status"], "active");
}

#[test]
fn test_role_any_of() {
    let user = User {
        id: 1,
        name: "Bob".to_string(),
        status: None,
    };
    let role_user = Role::from(user.clone());
    if let Role::User(u) = &role_user {
        assert_eq!(u.name, "Bob");
    } else {
        panic!("Expected Role::User");
    }

    let admin = Admin {
        id: Some(2),
        permissions: Some(vec!["read".to_string()]),
    };
    let role_admin = Role::from(admin);
    match &role_admin {
        Role::Admin(a) => assert_eq!(a.id, Some(2)),
        _ => panic!("Expected Role::Admin"),
    }

    let s = RoleSubtype3("guest".to_string());
    let role_string = Role::from(s);
    match &role_string {
        Role::RoleSubtype3(s) => assert_eq!(s.0, "guest"),
        _ => panic!("Expected Role::RoleSubtype3"),
    }
}

#[test]
fn test_complex_any_of() {
    // Test Type A
    let json_a = json!({
        "type": "A",
        "value_a": 42
    });
    let val_a: ComplexAnyOf = serde_json::from_value(json_a).unwrap();
    match val_a {
        ComplexAnyOf::ComplexAnyOfSubtype1(obj) => {
            assert_eq!(obj.value_a, Some(42));
        }
        ComplexAnyOf::ComplexAnyOfSubtype2(_) => panic!("Expected ComplexAnyOfSubtype1"),
    }

    // Test Type B
    let json_b = json!({
        "type": "B",
        "value_b": "hello"
    });
    let val_b: ComplexAnyOf = serde_json::from_value(json_b).unwrap();
    match val_b {
        ComplexAnyOf::ComplexAnyOfSubtype2(obj) => {
            assert_eq!(obj.value_b, Some("hello".to_string()));
        }
        ComplexAnyOf::ComplexAnyOfSubtype1(_) => panic!("Expected ComplexAnyOfSubtype2"),
    }
}

#[test]
fn test_default_values() {
    let role = Role::default();
    // Default should be the first variant, which is User
    match role {
        Role::User(_) => (),
        _ => panic!("Expected Role::User as default"),
    }

    let status = Status::default();
    assert_eq!(status, Status::Active); // First enum variant is default
}

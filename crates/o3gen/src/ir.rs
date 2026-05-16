use heck::ToSnakeCase;
use http::{Method, StatusCode};
use indexmap::IndexMap;

#[derive(Debug, Clone)]
pub struct ApiIr {
    pub types: IndexMap<String, TypeDefinitionIr>,
    pub operations: Vec<OperationIr>,
}

#[derive(Debug, Clone)]
pub enum TypeDefinitionIr {
    Struct(StructIr),
    Enum(EnumIr),
    Alias(AliasIr),
    AnyOf(AnyOfIr),
    Newtype(NewtypeIr),
}

impl TypeDefinitionIr {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Struct(s) => s.name.as_str(),
            Self::Enum(e) => e.name.as_str(),
            Self::Alias(a) => a.name.as_str(),
            Self::AnyOf(a) => a.name.as_str(),
            Self::Newtype(n) => n.name.as_str(),
        }
    }

    pub fn set_name(&mut self, name: String) {
        match self {
            Self::Struct(s) => s.name.set_string(name),
            Self::Enum(e) => e.name.set_string(name),
            Self::Alias(a) => a.name.set_string(name),
            Self::AnyOf(a) => a.name.set_string(name),
            Self::Newtype(n) => n.name.set_string(name),
        }
    }

    #[must_use]
    pub fn is_generated(&self) -> bool {
        match self {
            Self::Struct(s) => s.name.is_generated(),
            Self::Enum(e) => e.name.is_generated(),
            Self::Alias(a) => a.name.is_generated(),
            Self::AnyOf(a) => a.name.is_generated(),
            Self::Newtype(n) => n.name.is_generated(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Name {
    Provided(String),
    Generated(String),
}

impl Name {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Provided(s) | Self::Generated(s) => s,
        }
    }

    pub fn set_string(&mut self, new_name: String) {
        match self {
            Self::Provided(s) | Self::Generated(s) => *s = new_name,
        }
    }

    #[must_use]
    pub fn is_generated(&self) -> bool {
        matches!(self, Self::Generated(_))
    }
}

#[derive(Debug, Clone)]
pub struct StructIr {
    pub name: Name,
    pub fields: Vec<FieldIr>,
    pub derives: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnumIr {
    pub name: Name,
    pub variants: Vec<EnumVariantIr>,
    pub derives: Vec<String>,
    pub rename_all: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewtypeIr {
    pub name: Name,
    pub target: TypeIr,
    pub derives: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FieldIr {
    pub name: String,      // Original name in JSON
    pub rust_name: String, // Snake case identifier
    pub type_info: TypeIr,
    pub required: bool,
    pub validation: Vec<ValidationIr>,
    pub serde_rename: Option<String>,
    pub description: Option<String>,
}

impl FieldIr {
    #[must_use]
    pub fn new(
        name: &str,
        type_info: TypeIr,
        required: bool,
        validation: Vec<ValidationIr>,
        description: Option<String>,
    ) -> Self {
        let rust_name = name.to_snake_case();
        Self {
            name: name.to_string(),
            serde_rename: if name == rust_name {
                None
            } else {
                Some(name.to_string())
            },
            rust_name,
            type_info,
            required,
            validation,
            description,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnumVariantIr {
    pub name: String,
    pub rust_name: String,
    pub value: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AliasIr {
    pub name: Name,
    pub target: TypeIr,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnyOfIr {
    pub name: Name,
    pub variants: Vec<VariantIr>,
    pub derives: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VariantIr {
    pub name: String,
    pub type_info: TypeIr,
}

#[derive(Debug, Clone)]
pub enum TypeIr {
    Reference(String),
    Primitive(PrimitiveType),
    Array(Box<TypeIr>),
    Map(Box<TypeIr>),
    Value,        // serde_json::Value
    Enum(String), // Reference to an enum type definition
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    String,
    Integer,
    Number,
    Boolean,
    Date,
    DateTime,
}

#[derive(Debug, Clone, Copy)]
pub enum ValidationIr {
    Length { min: Option<u64>, max: Option<u64> },
    FloatRange { min: Option<f64>, max: Option<f64> },
    IntRange { min: Option<i64>, max: Option<i64> },
}

#[derive(Debug, Clone)]
pub struct OperationIr {
    pub operation_id: String,
    pub method: Method,
    pub path: String,
    pub parameters: Vec<ParameterIr>,
    pub request_body: Option<TypeIr>,
    pub responses: Vec<ResponseIr>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParameterIr {
    pub name: String,
    pub location: ParameterLocation,
    pub required: bool,
    pub type_info: TypeIr,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

#[derive(Debug, Clone)]
pub struct ResponseIr {
    pub code: StatusCode,
    pub type_info: Option<TypeIr>,
}

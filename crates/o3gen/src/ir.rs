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

#[derive(Debug, Clone)]
pub struct StructIr {
    pub name: String,
    pub fields: Vec<FieldIr>,
    pub derives: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EnumIr {
    pub name: String,
    pub variants: Vec<EnumVariantIr>,
    pub derives: Vec<String>,
    pub rename_all: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewtypeIr {
    pub name: String,
    pub target: TypeIr,
    pub derives: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FieldIr {
    pub name: String,      // Original name in JSON
    pub rust_name: String, // Snake case identifier
    pub type_info: TypeIr,
    pub required: bool,
    pub validation: Vec<ValidationIr>,
    pub serde_rename: Option<String>,
}

impl FieldIr {
    #[must_use]
    pub fn new(
        name: &str,
        type_info: TypeIr,
        required: bool,
        validation: Vec<ValidationIr>,
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
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnumVariantIr {
    pub name: String,
    pub rust_name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct AliasIr {
    pub name: String,
    pub target: TypeIr,
}

#[derive(Debug, Clone)]
pub struct AnyOfIr {
    pub name: String,
    pub variants: Vec<TypeIr>,
    pub derives: Vec<String>,
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
}

#[derive(Debug, Clone)]
pub struct ParameterIr {
    pub name: String,
    pub location: ParameterLocation,
    pub required: bool,
    pub type_info: TypeIr,
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

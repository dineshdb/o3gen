use openapiv3::{OpenAPI, ReferenceOr, Schema, SchemaKind, Type};
use std::collections::{HashMap, HashSet};

use crate::helpers::to_pascal_case;

struct Ctx {
    taken: HashSet<String>,
    fingerprints: HashMap<String, String>,
    rename: HashMap<String, String>,
    pending: Vec<(String, Schema)>,
}

#[allow(clippy::implicit_hasher)]
pub fn flatten(openapi: &mut OpenAPI, rename: &HashMap<String, String>) {
    let Some(components) = openapi.components.as_mut() else {
        return;
    };

    let mut ctx = Ctx {
        taken: components.schemas.keys().cloned().collect(),
        fingerprints: HashMap::new(),
        rename: rename.clone(),
        pending: Vec::new(),
    };

    let schema_names: Vec<String> = components.schemas.keys().cloned().collect();
    for name in &schema_names {
        if let Some(schema_ref) = components.schemas.get_mut(name) {
            walk_schema(name, schema_ref, &mut ctx);
        }
    }

    for (name, schema) in ctx.pending {
        openapi
            .components
            .get_or_insert_with(openapiv3::Components::default)
            .schemas
            .insert(name, ReferenceOr::Item(schema));
    }
}

fn walk_schema(parent: &str, schema_ref: &mut ReferenceOr<Schema>, ctx: &mut Ctx) {
    let schema = match schema_ref {
        ReferenceOr::Reference { .. } => return,
        ReferenceOr::Item(s) => s,
    };

    match &mut schema.schema_kind {
        SchemaKind::Type(Type::Object(obj)) => {
            let prop_names: Vec<String> = obj.properties.keys().cloned().collect();
            for prop_name in prop_names {
                let Some(prop_ref) = obj.properties.get_mut(&prop_name) else {
                    continue;
                };
                extract_boxed(parent, &prop_name, prop_ref, ctx, false);
            }
        }
        SchemaKind::Type(Type::Array(arr)) => {
            if let Some(items) = &mut arr.items {
                extract_boxed(parent, "items", items, ctx, true);
            }
        }
        SchemaKind::AllOf { all_of } => {
            for sub in all_of.iter_mut() {
                walk_schema(parent, sub, ctx);
            }
        }
        SchemaKind::AnyOf { any_of } => {
            for sub in any_of.iter_mut() {
                walk_schema(parent, sub, ctx);
            }
        }
        SchemaKind::OneOf { one_of } => {
            for sub in one_of.iter_mut() {
                walk_schema(parent, sub, ctx);
            }
        }
        _ => {}
    }
}

fn is_complex_kind(kind: &SchemaKind) -> bool {
    matches!(
        kind,
        SchemaKind::Type(Type::Object(_))
            | SchemaKind::AnyOf { .. }
            | SchemaKind::AllOf { .. }
            | SchemaKind::OneOf { .. }
    )
}

fn extract_boxed(
    parent: &str,
    field: &str,
    schema_ref: &mut ReferenceOr<Box<Schema>>,
    ctx: &mut Ctx,
    singularize_field: bool,
) {
    if let ReferenceOr::Reference { .. } = schema_ref {
        return;
    }

    // Recurse into the inline schema first (handle deeply nested inline objects bottom-up)
    if let ReferenceOr::Item(boxed_schema) = schema_ref {
        let schema = (**boxed_schema).clone();
        let mut unwrapped = ReferenceOr::Item(schema);
        walk_schema(parent, &mut unwrapped, ctx);
        if let ReferenceOr::Item(resolved) = &mut unwrapped {
            if let SchemaKind::Type(Type::Object(obj)) = &mut resolved.schema_kind {
                let prop_names: Vec<String> = obj.properties.keys().cloned().collect();
                for prop_name in prop_names {
                    let Some(prop_ref) = obj.properties.get_mut(&prop_name) else {
                        continue;
                    };
                    extract_boxed(parent, &prop_name, prop_ref, ctx, false);
                }
            }
            **boxed_schema = resolved.clone();
        }
    }

    let ReferenceOr::Item(schema) = schema_ref else {
        return;
    };

    if !is_complex_kind(&schema.schema_kind) {
        return;
    }

    // Fingerprint for dedup
    let fp = fingerprint_schema(schema);
    if let Some(existing_name) = ctx.fingerprints.get(&fp) {
        *schema_ref = ReferenceOr::Reference {
            reference: format!("#/components/schemas/{existing_name}"),
        };
        return;
    }

    // Compute name
    let pascal_field = to_pascal_case(field);
    let base = if singularize_field {
        singularize(&pascal_field)
    } else {
        pascal_field
    };
    let candidate = format!("{parent}{base}");
    let final_name = resolve_name(&candidate, &ctx.rename, &ctx.taken);

    // Register
    ctx.taken.insert(final_name.clone());
    ctx.fingerprints.insert(fp, final_name.clone());

    // Replace inline with $ref and queue extraction
    let owned = std::mem::replace(
        schema_ref,
        ReferenceOr::Reference {
            reference: format!("#/components/schemas/{final_name}"),
        },
    );
    let extracted = match owned {
        ReferenceOr::Item(boxed) => *boxed,
        ReferenceOr::Reference { .. } => unreachable!(),
    };
    ctx.pending.push((final_name, extracted));
}

fn fingerprint_schema(schema: &Schema) -> String {
    serde_json::to_string(schema).unwrap_or_default()
}

fn singularize(s: &str) -> String {
    if s.len() < 3 {
        return s.to_string();
    }
    let lower = s.to_ascii_lowercase();
    if lower.ends_with("ies") && s.len() > 3 {
        return format!("{}y", &s[..s.len() - 3]);
    }
    if lower.ends_with("shes")
        || lower.ends_with("ches")
        || lower.ends_with("xes")
        || lower.ends_with("zes")
        || lower.ends_with("sses")
    {
        return s[..s.len() - 2].to_string();
    }
    if lower.ends_with("es") && s.len() > 3 {
        return s[..s.len() - 2].to_string();
    }
    if lower.ends_with('s') && !lower.ends_with("ss") {
        return s[..s.len() - 1].to_string();
    }
    s.to_string()
}

fn resolve_name(
    candidate: &str,
    rename: &HashMap<String, String>,
    taken: &HashSet<String>,
) -> String {
    let renamed = rename
        .get(candidate)
        .cloned()
        .unwrap_or_else(|| candidate.to_string());
    if !taken.contains(&renamed) {
        return renamed;
    }
    let mut i = 2u32;
    loop {
        let attempt = format!("{renamed}{i}");
        if !taken.contains(&attempt) {
            return attempt;
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_singularize() {
        assert_eq!(singularize("Items"), "Item");
        assert_eq!(singularize("Addresses"), "Address");
        assert_eq!(singularize("Properties"), "Property");
        assert_eq!(singularize("Boxes"), "Box");
        assert_eq!(singularize("Matches"), "Match");
        assert_eq!(singularize("Brushes"), "Brush");
        assert_eq!(singularize("Classes"), "Class");
    }
}

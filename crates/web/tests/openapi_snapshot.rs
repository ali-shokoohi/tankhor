//! T099: Snapshot and local meta-schema conformance tests for `docs/openapi.json` (FR-001, FR-CON-01).

use std::path::Path;
use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::{Map, Value};
use testkit::fakes::health_probe::FakeHealthProbe;
use testkit::fakes::store::FakeStore;
use tower::ServiceExt;
use web::api::openapi::{
    OPENAPI_JSON_PATH, SchemaAccessPolicy, build_api_router, render_openapi_json,
};

const MAX_RESPONSE_BYTES: usize = 256 * 1024;
const PINNED_META_SCHEMA: &str = include_str!("fixtures/openapi-meta-schema.json");

fn resolve_ref<'a>(root_schema: &'a Value, reference: &str) -> Result<&'a Value, String> {
    let pointer = reference
        .strip_prefix('#')
        .ok_or_else(|| format!("unsupported external $ref `{reference}`"))?;
    root_schema
        .pointer(pointer)
        .ok_or_else(|| format!("unresolved local $ref `{reference}`"))
}

fn matches_json_type(instance: &Value, type_name: &str) -> bool {
    match type_name {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "integer" => instance.as_i64().is_some() || instance.as_u64().is_some(),
        "number" => instance.is_number(),
        "boolean" => instance.is_boolean(),
        "null" => instance.is_null(),
        _ => false,
    }
}

fn matches_pattern(value: &str, pattern: &str) -> Result<bool, String> {
    match pattern {
        "^3\\.1\\.\\d+$" => Ok(value
            .strip_prefix("3.1.")
            .is_some_and(|patch| !patch.is_empty() && patch.chars().all(|ch| ch.is_ascii_digit()))),
        "^/api/v1/" => Ok(value.starts_with("/api/v1/")),
        "^[1-5][0-9][0-9]$" => Ok(value.len() == 3
            && value.as_bytes()[0].is_ascii_digit()
            && (b'1'..=b'5').contains(&value.as_bytes()[0])
            && value.as_bytes()[1].is_ascii_digit()
            && value.as_bytes()[2].is_ascii_digit()),
        "^[a-z0-9.+-]+/[a-z0-9.+-]+$" => {
            let Some((left, right)) = value.split_once('/') else {
                return Ok(false);
            };
            let valid_token = |part: &str| {
                !part.is_empty()
                    && part.chars().all(|ch| {
                        ch.is_ascii_lowercase()
                            || ch.is_ascii_digit()
                            || matches!(ch, '.' | '+' | '-')
                    })
            };
            Ok(!right.contains('/') && valid_token(left) && valid_token(right))
        }
        "^[A-Za-z0-9_.-]+$" => Ok(!value.is_empty()
            && value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-'))),
        "^#/components/schemas/[A-Za-z0-9_.-]+$" => Ok(value
            .strip_prefix("#/components/schemas/")
            .is_some_and(|name| {
                !name.is_empty()
                    && name
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-'))
            })),
        "^.+$" => Ok(!value.is_empty()),
        other => Err(format!("unsupported meta-schema pattern `{other}`")),
    }
}

fn validate_object(
    root_schema: &Value,
    schema_obj: &Map<String, Value>,
    obj: &Map<String, Value>,
    path: &str,
) -> Result<(), String> {
    if let Some(min_props) = schema_obj.get("minProperties").and_then(Value::as_u64)
        && (obj.len() as u64) < min_props
    {
        return Err(format!(
            "{path}: object has {} properties, fewer than minProperties {min_props}",
            obj.len()
        ));
    }

    if let Some(required) = schema_obj.get("required").and_then(Value::as_array) {
        for req in required {
            let key = req
                .as_str()
                .ok_or_else(|| format!("{path}: non-string required entry"))?;
            if !obj.contains_key(key) {
                return Err(format!("{path}: missing required property `{key}`"));
            }
        }
    }

    let properties = schema_obj.get("properties").and_then(Value::as_object);
    let pattern_properties = schema_obj
        .get("patternProperties")
        .and_then(Value::as_object);
    let additional = schema_obj.get("additionalProperties");

    for (key, prop_value) in obj {
        let child_path = format!("{path}.{key}");
        let mut matched = false;

        if let Some(props) = properties
            && let Some(prop_schema) = props.get(key)
        {
            matched = true;
            validate_against_meta_schema(root_schema, prop_schema, prop_value, &child_path)?;
        }

        if let Some(patterns) = pattern_properties {
            for (pattern, pat_schema) in patterns {
                if matches_pattern(key, pattern)? {
                    matched = true;
                    validate_against_meta_schema(root_schema, pat_schema, prop_value, &child_path)?;
                }
            }
        }

        if !matched {
            match additional {
                Some(Value::Bool(false)) => {
                    return Err(format!(
                        "{path}: unexpected additional property `{key}` rejected by meta-schema"
                    ));
                }
                Some(Value::Object(_)) => {
                    validate_against_meta_schema(
                        root_schema,
                        additional.expect("checked Some"),
                        prop_value,
                        &child_path,
                    )?;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn validate_against_meta_schema(
    root_schema: &Value,
    schema: &Value,
    instance: &Value,
    path: &str,
) -> Result<(), String> {
    let schema_obj = schema
        .as_object()
        .ok_or_else(|| format!("schema node at {path} must be an object"))?;

    if let Some(reference) = schema_obj.get("$ref").and_then(Value::as_str) {
        let resolved = resolve_ref(root_schema, reference)?;
        return validate_against_meta_schema(root_schema, resolved, instance, path);
    }

    if let Some(any_of) = schema_obj.get("anyOf").and_then(Value::as_array) {
        let mut matched_branch = false;
        for branch in any_of {
            if validate_against_meta_schema(root_schema, branch, instance, path).is_ok() {
                matched_branch = true;
                break;
            }
        }
        if !matched_branch {
            return Err(format!("{path}: value did not match anyOf branches"));
        }
    }

    if let Some(expected_type) = schema_obj.get("type") {
        let valid = match expected_type {
            Value::String(name) => matches_json_type(instance, name),
            Value::Array(names) => names
                .iter()
                .filter_map(Value::as_str)
                .any(|name| matches_json_type(instance, name)),
            _ => false,
        };
        if !valid {
            return Err(format!(
                "{path}: value `{instance}` does not match expected type `{expected_type}`"
            ));
        }
    }

    if let Some(text) = instance.as_str() {
        if let Some(min_len) = schema_obj.get("minLength").and_then(Value::as_u64)
            && (text.len() as u64) < min_len
        {
            return Err(format!(
                "{path}: string `{text}` shorter than minLength {min_len}"
            ));
        }
        if let Some(pattern) = schema_obj.get("pattern").and_then(Value::as_str)
            && !matches_pattern(text, pattern)?
        {
            return Err(format!(
                "{path}: string `{text}` does not match pattern `{pattern}`"
            ));
        }
    }

    if let Some(arr) = instance.as_array() {
        if let Some(min_items) = schema_obj.get("minItems").and_then(Value::as_u64)
            && (arr.len() as u64) < min_items
        {
            return Err(format!("{path}: array shorter than minItems {min_items}"));
        }
        if let Some(item_schema) = schema_obj.get("items") {
            for (index, item) in arr.iter().enumerate() {
                validate_against_meta_schema(
                    root_schema,
                    item_schema,
                    item,
                    &format!("{path}[{index}]"),
                )?;
            }
        }
    }

    if let Some(obj) = instance.as_object() {
        validate_object(root_schema, schema_obj, obj, path)?;
    }

    Ok(())
}

#[tokio::test]
async fn openapi_snapshot_matches_router() {
    let meta_schema: Value =
        serde_json::from_str(PINNED_META_SCHEMA).expect("pinned local meta-schema must parse");
    assert_eq!(
        meta_schema["$id"],
        "https://spec.openapis.org/oas/3.1/schema/2022-10-07"
    );

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root directory");
    let committed_path = repo_root.join("docs/openapi.json");
    assert!(
        committed_path.is_file(),
        "docs/openapi.json must be generated via `cargo run -p app -- openapi` and committed"
    );

    let committed_raw =
        std::fs::read_to_string(&committed_path).expect("docs/openapi.json must be readable UTF-8");
    let expected_offline = render_openapi_json().expect("offline OpenAPI rendering must succeed");
    assert_eq!(
        committed_raw, expected_offline,
        "docs/openapi.json drifted from router-generated OpenAPI specification"
    );

    let (router, _doc) = build_api_router(
        Arc::new(FakeHealthProbe::new(FakeStore::new())),
        SchemaAccessPolicy::NonProductionOpen,
    );
    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(OPENAPI_JSON_PATH)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("openapi response");
    assert_eq!(response.status(), StatusCode::OK);
    let served_bytes = to_bytes(response.into_body(), MAX_RESPONSE_BYTES)
        .await
        .expect("body bytes");
    let served_json: Value = serde_json::from_slice(&served_bytes).expect("served JSON");
    let committed_json: Value = serde_json::from_str(&committed_raw).expect("committed JSON");
    assert_eq!(served_json, committed_json);

    validate_against_meta_schema(&meta_schema, &meta_schema, &committed_json, "$")
        .expect("committed OpenAPI 3.1 document must conform to pinned local meta-schema");

    // Negative check: malformed/non-v1 path or invalid openapi version must fail meta-schema check.
    let mut drifted = committed_json.clone();
    drifted["paths"]["/api/health"] = drifted["paths"]["/api/v1/health"].clone();
    assert!(
        validate_against_meta_schema(&meta_schema, &meta_schema, &drifted, "$").is_err(),
        "meta-schema must reject non-/api/v1 path entries"
    );

    // Every internal component $ref in the committed document must resolve cleanly.
    let mut stack = vec![&committed_json];
    while let Some(node) = stack.pop() {
        match node {
            Value::Object(map) => {
                if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                    resolve_ref(&committed_json, reference)
                        .expect("every $ref in docs/openapi.json must resolve");
                }
                stack.extend(map.values());
            }
            Value::Array(items) => stack.extend(items.iter()),
            _ => {}
        }
    }
}

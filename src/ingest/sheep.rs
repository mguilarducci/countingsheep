//! The validated `Sheep` (a CloudEvents v1.0.2 event) and its pure validator.

use serde_json::{Map, Value};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// A usage event that has passed validation. Derives below also count as
/// "uses" of every field, so unused optional fields don't trip `dead_code`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sheep {
    pub id: String,
    pub source: String,
    pub r#type: String,
    pub specversion: String,
    pub subject: Option<String>,
    pub time: Option<String>,
    pub data: Option<Value>,
    pub datacontenttype: Option<String>,
    pub dataschema: Option<String>,
}

/// A present, non-empty string is required.
fn required_string(
    obj: &Map<String, Value>,
    key: &str,
    errors: &mut Vec<String>,
) -> Option<String> {
    match obj.get(key) {
        None | Some(Value::Null) => {
            errors.push(format!("{key} is required"));
            None
        }
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::String(_)) => {
            errors.push(format!("{key} must not be empty"));
            None
        }
        Some(_) => {
            errors.push(format!("{key} must be a string"));
            None
        }
    }
}

/// If present and non-null, must be a string.
fn optional_string(
    obj: &Map<String, Value>,
    key: &str,
    errors: &mut Vec<String>,
) -> Option<String> {
    match obj.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => {
            errors.push(format!("{key} must be a string"));
            None
        }
    }
}

/// Validate a raw JSON value against the CloudEvents v1.0.2 contract.
/// Collects *all* failures rather than stopping at the first.
pub(crate) fn validate(value: Value) -> Result<Sheep, Vec<String>> {
    let Some(obj) = value.as_object() else {
        return Err(vec!["body must be a JSON object".to_string()]);
    };

    let mut errors = Vec::new();

    let id = required_string(obj, "id", &mut errors);
    let source = required_string(obj, "source", &mut errors);
    let kind = required_string(obj, "type", &mut errors);
    let specversion = required_string(obj, "specversion", &mut errors);
    if let Some(sv) = &specversion
        && sv != "1.0"
    {
        errors.push(format!("specversion must be \"1.0\", got \"{sv}\""));
    }

    let subject = optional_string(obj, "subject", &mut errors);
    let datacontenttype = optional_string(obj, "datacontenttype", &mut errors);
    let dataschema = optional_string(obj, "dataschema", &mut errors);

    let time = optional_string(obj, "time", &mut errors);
    if let Some(t) = &time
        && OffsetDateTime::parse(t, &Rfc3339).is_err()
    {
        errors.push(format!("time must be RFC 3339, got \"{t}\""));
    }

    let data = obj.get("data").filter(|v| !v.is_null()).cloned();

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(Sheep {
        id: id.unwrap(),
        source: source.unwrap(),
        r#type: kind.unwrap(),
        specversion: specversion.unwrap(),
        subject,
        time,
        data,
        datacontenttype,
        dataschema,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid() -> Value {
        json!({ "id": "a-1", "source": "/svc", "type": "usage.created", "specversion": "1.0" })
    }

    #[test]
    fn accepts_minimal_valid() {
        assert!(validate(valid()).is_ok());
    }

    #[test]
    fn accepts_full_and_unknown_extension() {
        let v = json!({
            "id": "a-1", "source": "/svc", "type": "usage.created", "specversion": "1.0",
            "subject": "tenant-9", "time": "2026-06-26T10:00:00Z",
            "datacontenttype": "application/json", "dataschema": "https://x/s",
            "data": { "tokens": 42 }, "tenantid": "extension-ok"
        });
        let s = validate(v).unwrap();
        assert_eq!(s.subject.as_deref(), Some("tenant-9"));
        assert_eq!(s.data, Some(json!({ "tokens": 42 })));
    }

    #[test]
    fn rejects_missing_and_empty_required() {
        for key in ["id", "source", "type", "specversion"] {
            let mut missing = valid();
            missing.as_object_mut().unwrap().remove(key);
            assert!(validate(missing).is_err(), "missing {key} should fail");

            let mut empty = valid();
            empty[key] = json!("");
            assert!(validate(empty).is_err(), "empty {key} should fail");
        }
    }

    #[test]
    fn rejects_wrong_specversion() {
        let mut v = valid();
        v["specversion"] = json!("0.3");
        let errs = validate(v).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("specversion")));
    }

    #[test]
    fn time_is_optional_but_must_be_rfc3339_when_present() {
        let mut absent = valid();
        absent.as_object_mut().unwrap().remove("time");
        assert!(validate(absent).is_ok());

        let mut good = valid();
        good["time"] = json!("2026-06-26T10:00:00Z");
        assert!(validate(good).is_ok());

        let mut bad = valid();
        bad["time"] = json!("not-a-date");
        assert!(
            validate(bad)
                .unwrap_err()
                .iter()
                .any(|e| e.contains("RFC 3339"))
        );
    }

    #[test]
    fn reports_all_errors_at_once() {
        let errs = validate(json!({ "specversion": "1.0" })).unwrap_err();
        assert_eq!(errs.len(), 3, "expected id + source + type, got {errs:?}");
    }

    #[test]
    fn rejects_non_object_body() {
        assert!(validate(json!("just a string")).is_err());
    }
}

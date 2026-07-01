//! The validated `Sheep` (a CloudEvents v1.0.2 event) and its pure validator.

use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

/// A usage event that has passed validation. Derives below also count as
/// "uses" of every field, so unused optional fields don't trip `dead_code`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sheep {
    pub id: String,
    pub source: String,
    pub r#type: String,
    pub specversion: String,
    pub subject: String,
    pub time: Option<OffsetDateTime>,
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

/// If present and non-null, must be a non-empty string. Per CloudEvents
/// v1.0.2, the optional string attributes carried here (`subject`,
/// `datacontenttype`, `dataschema`) MUST be non-empty when present — a
/// present-but-empty optional is malformed, not absent.
fn optional_string(
    obj: &Map<String, Value>,
    key: &str,
    errors: &mut Vec<String>,
) -> Option<String> {
    match obj.get(key) {
        None | Some(Value::Null) => None,
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

/// True for an absolute URI per RFC 3986 §4.3: a valid `scheme` followed by
/// `:`, no fragment, and no raw whitespace/control characters. Intentionally
/// scheme-keyed (not authority-based) so `urn:` and `mailto:` URIs pass.
fn is_absolute_uri(s: &str) -> bool {
    let Some((scheme, _rest)) = s.split_once(':') else {
        return false;
    };
    let mut scheme_chars = scheme.chars();
    let valid_scheme = matches!(scheme_chars.next(), Some(c) if c.is_ascii_alphabetic())
        && scheme_chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid_scheme && !s.contains('#') && !s.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// If present and non-null, must be a non-empty absolute URI.
fn optional_uri(obj: &Map<String, Value>, key: &str, errors: &mut Vec<String>) -> Option<String> {
    let value = optional_string(obj, key, errors)?;
    if is_absolute_uri(&value) {
        Some(value)
    } else {
        errors.push(format!("{key} must be an absolute URI"));
        None
    }
}

/// True for an RFC 2046 media type: `type "/" subtype` optionally followed by
/// `;`-separated parameters (which are not inspected). Both `type` and
/// `subtype` must be non-empty RFC 2045 tokens — printable ASCII excluding
/// space, control characters, and `tspecials` (`()<>@,;:\"/[]?=`).
fn is_media_type(s: &str) -> bool {
    let essence = s.split(';').next().unwrap_or(s).trim();
    let Some((kind, subtype)) = essence.split_once('/') else {
        return false;
    };
    let is_token = |part: &str| {
        !part.is_empty()
            && part.chars().all(|c| {
                c.is_ascii_graphic()
                    && !matches!(
                        c,
                        '(' | ')'
                            | '<'
                            | '>'
                            | '@'
                            | ','
                            | ';'
                            | ':'
                            | '\\'
                            | '"'
                            | '/'
                            | '['
                            | ']'
                            | '?'
                            | '='
                    )
            })
    };
    is_token(kind) && is_token(subtype)
}

/// If present and non-null, must be a non-empty RFC 2046 media type.
fn optional_media_type(
    obj: &Map<String, Value>,
    key: &str,
    errors: &mut Vec<String>,
) -> Option<String> {
    let value = optional_string(obj, key, errors)?;
    if is_media_type(&value) {
        Some(value)
    } else {
        errors.push(format!("{key} must be a media type"));
        None
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

    let subject = required_string(obj, "subject", &mut errors);
    let datacontenttype = optional_media_type(obj, "datacontenttype", &mut errors);
    let dataschema = optional_uri(obj, "dataschema", &mut errors);

    // Parse and *keep* the time as a UTC `OffsetDateTime`, so a consistent
    // representation is structural rather than convention. An offset-bearing
    // time is normalized to the equivalent UTC instant; an unparseable one is
    // rejected. (A missing time is defaulted later, at the handler edge, where
    // the clock lives — `validate` stays pure.)
    let time =
        optional_string(obj, "time", &mut errors).and_then(|t| {
            match OffsetDateTime::parse(&t, &Rfc3339) {
                Ok(dt) => Some(dt.to_offset(UtcOffset::UTC)),
                Err(_) => {
                    errors.push(format!("time must be RFC 3339, got \"{t}\""));
                    None
                }
            }
        });

    let data = obj.get("data").filter(|v| !v.is_null()).cloned();

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(Sheep {
        id: id.unwrap(),
        source: source.unwrap(),
        r#type: kind.unwrap(),
        specversion: specversion.unwrap(),
        subject: subject.unwrap(),
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
        json!({ "id": "a-1", "source": "/svc", "type": "usage.created",
                "specversion": "1.0", "subject": "customer-1" })
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
        assert_eq!(s.subject, "tenant-9");
        assert_eq!(s.data, Some(json!({ "tokens": 42 })));
    }

    #[test]
    fn rejects_missing_and_empty_required() {
        for key in ["id", "source", "type", "specversion", "subject"] {
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
    fn time_is_kept_as_an_offsetdatetime_normalized_to_utc() {
        use time::UtcOffset;
        use time::macros::datetime;

        // An offset-bearing time is kept as the equivalent instant, with the
        // stored offset normalized to UTC (D2). The instant equality alone
        // would pass without normalization — OffsetDateTime compares instants —
        // so the offset assertion is what actually pins "stored in UTC".
        let mut offset = valid();
        offset["time"] = json!("2026-06-26T12:00:00+02:00");
        let t = validate(offset).unwrap().time.expect("time is kept");
        assert_eq!(t, datetime!(2026-06-26 10:00:00 UTC), "same instant");
        assert_eq!(
            t.offset(),
            UtcOffset::UTC,
            "stored offset normalized to UTC"
        );

        // Fractional seconds survive the round-trip.
        let mut frac = valid();
        frac["time"] = json!("2026-06-26T10:00:00.5Z");
        let t = validate(frac).unwrap().time.expect("time is kept");
        assert_eq!(
            t,
            datetime!(2026-06-26 10:00:00.5 UTC),
            "fractional preserved"
        );

        // Absent time stays absent (defaulting happens later, at the edge).
        let mut absent = valid();
        absent.as_object_mut().unwrap().remove("time");
        assert_eq!(validate(absent).unwrap().time, None);
    }

    #[test]
    fn time_boundary_values_are_handled() {
        use time::macros::datetime;

        // `-00:00` (RFC 3339 "unknown offset") is valid and accepted.
        let mut unknown_offset = valid();
        unknown_offset["time"] = json!("2026-06-26T10:00:00-00:00");
        assert!(
            validate(unknown_offset).is_ok(),
            "-00:00 should be accepted"
        );

        // Leap second `:60`: the time crate does NOT reject it — it accepts the
        // value and clamps to the last representable instant of the day. We pin
        // that real behavior (verified, not assumed) so a dependency bump that
        // changes it is caught. We do not model true leap seconds.
        let mut leap = valid();
        leap["time"] = json!("2026-12-31T23:59:60Z");
        let t = validate(leap).unwrap().time.expect("leap second is kept");
        assert_eq!(t, datetime!(2026-12-31 23:59:59.999_999_999 UTC));

        // A future-dated time is accepted as-is: we record the gap, we don't
        // police clock skew (out of scope).
        let mut future = valid();
        future["time"] = json!("2099-01-01T00:00:00Z");
        assert!(validate(future).is_ok(), "future time should be accepted");
    }

    #[test]
    fn time_with_wrong_json_type_is_rejected_not_defaulted() {
        // A present-but-malformed time must surface an error, never silently
        // fall through to the "now" default.
        for bad in [json!(1_234_567_890), json!(true), json!("")] {
            let mut v = valid();
            v["time"] = bad.clone();
            let errs = validate(v).unwrap_err();
            assert!(
                errs.iter().any(|e| e.contains("time")),
                "{bad:?} time should be rejected, got {errs:?}"
            );
        }
    }

    #[test]
    fn rejects_empty_optional_attributes() {
        // CloudEvents v1.0.2: subject and dataschema MUST be a non-empty
        // string/URI when present; datacontenttype must be a valid RFC 2046
        // media type (an empty string is not). A present-but-empty optional is
        // malformed, not absent.
        for key in ["dataschema", "datacontenttype"] {
            let mut v = valid();
            v[key] = json!("");
            let errs = validate(v).unwrap_err();
            assert!(
                errs.iter().any(|e| e.contains(key) && e.contains("empty")),
                "empty {key} should be rejected as non-empty, got {errs:?}"
            );
        }
    }

    #[test]
    fn absent_optional_attributes_are_accepted() {
        // Optional governs presence: absent is fine (no error), unlike a
        // present-but-empty value.
        assert!(validate(valid()).is_ok());
    }

    #[test]
    fn reports_all_errors_at_once() {
        let errs = validate(json!({ "specversion": "1.0" })).unwrap_err();
        assert_eq!(
            errs.len(),
            4,
            "expected id + source + type + subject, got {errs:?}"
        );
    }

    #[test]
    fn rejects_non_object_body() {
        assert!(validate(json!("just a string")).is_err());
    }

    #[test]
    fn dataschema_must_be_absolute_uri_when_present() {
        // CloudEvents v1.0.2 types `dataschema` as URI (RFC 3986 §4.3,
        // absolute URI): a scheme is mandatory. A bare word or a string with
        // spaces is non-empty but not a URI, and must be rejected.
        for bad in ["not a uri", "relativeword", "/relative/path", "schema#frag"] {
            let mut v = valid();
            v["dataschema"] = json!(bad);
            let errs = validate(v).unwrap_err();
            assert!(
                errs.iter()
                    .any(|e| e.contains("dataschema") && e.contains("URI")),
                "{bad:?} should be rejected as a non-URI, got {errs:?}"
            );
        }
    }

    #[test]
    fn datacontenttype_must_be_media_type_when_present() {
        // CloudEvents v1.0.2 types `datacontenttype` as an RFC 2046 media type:
        // `type/subtype`. A non-empty string that is not shaped as a media type
        // (no slash, a bare word, or whitespace in a token) must be rejected.
        for bad in ["garbage value", "application", "/json", "application/"] {
            let mut v = valid();
            v["datacontenttype"] = json!(bad);
            let errs = validate(v).unwrap_err();
            assert!(
                errs.iter()
                    .any(|e| e.contains("datacontenttype") && e.contains("media type")),
                "{bad:?} should be rejected as a non-media-type, got {errs:?}"
            );
        }
    }

    #[test]
    fn datacontenttype_accepts_varied_media_types() {
        // type/subtype, with optional parameters, must pass.
        for good in [
            "application/json",
            "text/plain; charset=utf-8",
            "application/cloudevents+json",
            "application/vnd.api+json",
        ] {
            let mut v = valid();
            v["datacontenttype"] = json!(good);
            assert!(validate(v).is_ok(), "{good:?} should be accepted");
        }
    }

    #[test]
    fn dataschema_accepts_varied_absolute_uri_schemes() {
        // Not just http(s): URNs and mailto are valid absolute URIs and must
        // pass, so the check keys on the scheme, not on an HTTP authority.
        for good in [
            "https://example.com/schema.json",
            "urn:uuid:6e8bc430-9c3a-11d9-9669-0800200c9a66",
            "mailto:schemas@example.com",
            "ftp://h/p",
        ] {
            let mut v = valid();
            v["dataschema"] = json!(good);
            assert!(validate(v).is_ok(), "{good:?} should be accepted");
        }
    }
}

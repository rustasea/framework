/// Unit tests for the rule dispatcher.
///
/// The pre-existing rule tests are preserved verbatim so behaviour is pinned;
/// the new auth rules (`string`, `confirmed`, `password`, `unique`,
/// `current_password`) are covered below.
use super::*;
use crate::context::MemoryContext;
use crate::failure::RuleError;
use crate::password::PasswordPolicy;
use serde_json::json;

/// Required + min length enforce presence and size.
#[test]
fn required_and_min_length_rules() {
    let rules = Rules::new().field("name", "required|min:3").clone();
    assert!(rules.validate(&json!({ "name": "Ada" })).is_ok());
    assert!(rules.validate(&json!({ "name": "ab" })).is_err());
    assert!(rules.validate(&json!({})).is_err());
}

/// `to_value` serializes a payload through the validation facade.
#[test]
fn to_value_serializes_payload() {
    let value = to_value(json!({ "name": "Ada" })).expect("payload serializes");
    assert_eq!(value["name"].as_str(), Some("Ada"));
}

/// Strict `in` distinguishes int from string.
#[test]
fn strict_in_rule_rejects_string_int() {
    let rules = Rules::new().field("identifier", "in_array:1,2,3").clone();
    assert!(rules.validate(&json!({ "identifier": 1 })).is_ok());
    let bag = rules.validate(&json!({ "identifier": "1" })).unwrap_err();
    assert!(bag.get("identifier").iter().any(|e| e.code == "in_array"));
}

/// `in:` accepts a bare-word list, still comparing type and value strictly.
#[test]
fn in_rule_accepts_bare_words() {
    let rules = Rules::new().field("role", "in:admin,editor").clone();
    assert!(rules.validate(&json!({ "role": "admin" })).is_ok());
    assert!(rules.validate(&json!({ "role": "editor" })).is_ok());
    let bag = rules.validate(&json!({ "role": "viewer" })).unwrap_err();
    assert!(bag.get("role").iter().any(|e| e.code == "in_array"));
    let bag = rules.validate(&json!({ "role": "Admin" })).unwrap_err();
    assert!(bag.get("role").iter().any(|e| e.code == "in_array"));

    // Quoted numbers stay strings; bare numbers stay numbers.
    let rules = Rules::new().field("code", "in:\"1\",two").clone();
    assert!(rules.validate(&json!({ "code": "1" })).is_ok());
    assert!(rules.validate(&json!({ "code": "two" })).is_ok());
    assert!(rules.validate(&json!({ "code": 1 })).is_err());
}

/// Optional fields skip rules when absent; required still fires.
#[test]
fn optional_absent_fields_pass() {
    let rules = Rules::new().field("role", "contains_strict:admin").clone();
    assert!(rules.validate(&json!({})).is_ok());
    assert!(rules.validate(&json!({ "role": null })).is_ok());
}

/// `strict_contains` / `strict_in_array` rule names are accepted and
/// enforce strict type+case semantics (FS-M3-05).
#[test]
fn strict_alias_rule_names_are_accepted() {
    let rules = Rules::new().field("role", "strict_contains:admin").clone();
    assert!(rules.validate(&json!({ "role": "admin" })).is_ok());
    let bag = rules.validate(&json!({ "role": "Admin" })).unwrap_err();
    assert!(bag.get("role").iter().any(|e| e.code == "contains_strict"));

    let rules = Rules::new()
        .field("identifier", "strict_in_array:1,2,3")
        .clone();
    assert!(rules.validate(&json!({ "identifier": 2 })).is_ok());
    let bag = rules.validate(&json!({ "identifier": "2" })).unwrap_err();
    assert!(bag.get("identifier").iter().any(|e| e.code == "in_array"));
}

/// `string` accepts JSON strings and rejects every other JSON type.
#[test]
fn string_rule_checks_json_type() {
    let rules = Rules::new().field("name", "required|string").clone();
    assert!(rules.validate(&json!({ "name": "Ada" })).is_ok());
    let bag = rules.validate(&json!({ "name": 42 })).unwrap_err();
    assert!(bag.get("name").iter().any(|e| e.code == "string"));
    let bag = rules.validate(&json!({ "name": true })).unwrap_err();
    assert!(bag.get("name").iter().any(|e| e.code == "string"));
}

/// `timezone` accepts valid IANA names and rejects unknown ones.
#[test]
fn timezone_rule_checks_iana_name() {
    let rules = Rules::new().field("timezone", "required|timezone").clone();
    assert!(rules
        .validate(&json!({ "timezone": "Asia/Jakarta" }))
        .is_ok());
    // A documented short alias is accepted.
    assert!(rules.validate(&json!({ "timezone": "PST" })).is_ok());
    // An unknown name fails with the `timezone` code.
    let bag = rules
        .validate(&json!({ "timezone": "Not/AZone" }))
        .unwrap_err();
    assert!(bag.get("timezone").iter().any(|e| e.code == "timezone"));
    // A non-string value fails too.
    let bag = rules.validate(&json!({ "timezone": 42 })).unwrap_err();
    assert!(bag.get("timezone").iter().any(|e| e.code == "timezone"));
}

/// `confirmed` passes when the sibling confirmation matches and fails on
/// mismatch or when the confirmation key is absent.
#[test]
fn confirmed_rule_compares_sibling_key() {
    let rules = Rules::new().field("password", "required|confirmed").clone();
    assert!(rules
        .validate(&json!({ "password": "secret", "password_confirmation": "secret" }))
        .is_ok());

    let bag = rules
        .validate(&json!({ "password": "secret", "password_confirmation": "nope" }))
        .unwrap_err();
    assert!(bag.get("password").iter().any(|e| e.code == "confirmed"));

    let bag = rules
        .validate(&json!({ "password": "secret" }))
        .unwrap_err();
    assert!(bag.get("password").iter().any(|e| e.code == "confirmed"));
}

/// `password` enforces the configured policy, one message per violated rule.
#[test]
fn password_rule_reports_each_violation() {
    let rules = Rules::new()
        .password_policy(PasswordPolicy::production())
        .field("password", "required|string|password")
        .clone();

    assert!(rules
        .validate(&json!({ "password": "Sup3rSecret!" }))
        .is_ok());

    // Too short and all lowercase: missing min + mixed case.
    let bag = rules.validate(&json!({ "password": "short" })).unwrap_err();
    let codes: Vec<&str> = bag
        .get("password")
        .iter()
        .map(|e| e.code.as_str())
        .collect();
    assert!(codes.contains(&"password_min"));
    assert!(codes.contains(&"password_mixed_case"));
    assert!(codes.contains(&"password_numbers"));
    assert!(codes.contains(&"password_symbols"));

    // Long enough but no letters at all: missing letters + mixed case.
    let bag = rules
        .validate(&json!({ "password": "123456789012" }))
        .unwrap_err();
    let codes: Vec<&str> = bag
        .get("password")
        .iter()
        .map(|e| e.code.as_str())
        .collect();
    assert!(codes.contains(&"password_letters"));
    assert!(codes.contains(&"password_mixed_case"));

    // All lowercase + digit + symbol: still missing mixed case.
    let bag = rules
        .validate(&json!({ "password": "sup3rsecret!" }))
        .unwrap_err();
    assert!(bag
        .get("password")
        .iter()
        .any(|e| e.code == "password_mixed_case"));
}

/// Default policy is min 12 only, matching the scaffold's `MIN_LENGTH`.
#[test]
fn default_password_policy_is_min_twelve() {
    assert_eq!(PasswordPolicy::default(), PasswordPolicy::new().min(12));
    let rules = Rules::new().field("password", "required|password").clone();
    assert!(rules
        .validate(&json!({ "password": "twelvechars!" }))
        .is_ok());
    assert!(rules.validate(&json!({ "password": "short" })).is_err());
}

/// Enabling `uncompromised` fails closed rather than silently passing.
#[test]
fn uncompromised_policy_fails_closed() {
    let rules = Rules::new()
        .password_policy(PasswordPolicy::new().min(4).uncompromised(true))
        .field("password", "required|password")
        .clone();
    let failure = rules
        .validate_checked(&json!({ "password": "goodpass" }), &MemoryContext::new())
        .unwrap_err();
    assert!(failure.is_rule_error());
}

/// `unique` passes when context reports no conflict and fails on conflict.
#[test]
fn unique_rule_uses_context() {
    let rules = Rules::new()
        .field("email", "required|email|unique:users,email")
        .clone();
    let taken = MemoryContext::new().with_unique("users", "email", "ada@example.com");
    assert!(rules
        .validate_with(&json!({ "email": "grace@example.com" }), &taken)
        .is_ok());
    let bag = rules
        .validate_with(&json!({ "email": "ada@example.com" }), &taken)
        .unwrap_err();
    assert!(bag.get("email").iter().any(|e| e.code == "unique"));
}

/// `unique` self-exclusion: the owner may re-submit their own value.
#[test]
fn unique_rule_ignores_own_row() {
    let rules = Rules::new()
        .field("email", "required|unique:users,email,42")
        .clone();
    let ctx = MemoryContext::new().with_unique_owned("users", "email", "ada@example.com", "42");
    // Same owner id: allowed.
    assert!(rules
        .validate_with(&json!({ "email": "ada@example.com" }), &ctx)
        .is_ok());
    // Different ignore id: still a conflict.
    let other = Rules::new()
        .field("email", "required|unique:users,email,99")
        .clone();
    assert!(other
        .validate_with(&json!({ "email": "ada@example.com" }), &ctx)
        .is_err());
}

/// `unique` with `NoContext` (the default `validate`) fails closed with a
/// typed missing-context error, not a silent pass and not a generic failure.
#[test]
fn unique_without_context_fails_closed() {
    let rules = Rules::new()
        .field("email", "required|unique:users,email")
        .clone();
    let failure = rules
        .validate_checked(
            &json!({ "email": "ada@example.com" }),
            &crate::context::NoContext,
        )
        .unwrap_err();
    match failure.rule_error() {
        Some(RuleError::MissingContext { field, rule }) => {
            assert_eq!(field, "email");
            assert_eq!(rule, "unique");
        }
        other => panic!("expected MissingContext, got {other:?}"),
    }
    // The historical signature still surfaces the typed error in the bag.
    let bag = rules
        .validate(&json!({ "email": "ada@example.com" }))
        .unwrap_err();
    assert!(bag.get("email").iter().any(|e| e.code == "missing_context"));
}

/// `unique` without table/column is an unsupported configuration.
#[test]
fn unique_without_params_is_unsupported() {
    let rules = Rules::new().field("email", "required|unique").clone();
    let failure = rules
        .validate_checked(
            &json!({ "email": "ada@example.com" }),
            &MemoryContext::new(),
        )
        .unwrap_err();
    assert!(matches!(
        failure.rule_error(),
        Some(RuleError::Unsupported { .. })
    ));
}

/// `current_password` passes on a matching context and fails on mismatch.
#[test]
fn current_password_rule_uses_context() {
    let rules = Rules::new()
        .field("current_password", "required|string|current_password")
        .clone();
    let ctx = MemoryContext::new().with_current_password("old-secret");
    assert!(rules
        .validate_with(&json!({ "current_password": "old-secret" }), &ctx)
        .is_ok());
    let bag = rules
        .validate_with(&json!({ "current_password": "wrong" }), &ctx)
        .unwrap_err();
    assert!(bag
        .get("current_password")
        .iter()
        .any(|e| e.code == "current_password"));
}

/// `current_password` with `NoContext` fails closed with a typed error.
#[test]
fn current_password_without_context_fails_closed() {
    let rules = Rules::new()
        .field("current_password", "required|current_password")
        .clone();
    let failure = rules
        .validate_checked(
            &json!({ "current_password": "whatever" }),
            &crate::context::NoContext,
        )
        .unwrap_err();
    match failure.rule_error() {
        Some(RuleError::MissingContext { field, rule }) => {
            assert_eq!(field, "current_password");
            assert_eq!(rule, "current_password");
        }
        other => panic!("expected MissingContext, got {other:?}"),
    }
}

/// `validate` stays non-breaking: it still returns `Result<(), ErrorBag>`.
#[test]
fn validate_signature_is_non_breaking() {
    let rules = Rules::new().field("name", "required|string").clone();
    let result: Result<(), crate::error_bag::ErrorBag> = rules.validate(&json!({ "name": "Ada" }));
    assert!(result.is_ok());
}

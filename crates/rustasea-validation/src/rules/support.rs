/// Rule evaluation: the dispatcher and its pure helpers.
///
/// Split out of `mod.rs` to keep every file well under the 500-line cap. The
/// public surface lives in [`super`].
use serde_json::Value;

use crate::context::ValidationContext;
use crate::error_bag::ValidationError;
use crate::failure::RuleError;
use crate::password::PasswordPolicy;
use crate::strict::{contains_strict, doesnt_contain, in_array_strict};

/// Outcome of evaluating one rule on one field.
pub(crate) enum RuleFailure {
    /// An ordinary validation failure (bad value).
    Invalid(ValidationError),
    /// A rule that could not be evaluated (fail-closed).
    Context(RuleError),
}

impl From<ValidationError> for RuleFailure {
    fn from(err: ValidationError) -> Self {
        RuleFailure::Invalid(err)
    }
}

/// Apply a single rule string to a field value, yielding zero or more failures.
///
/// Most rules produce at most one failure; `password` may produce one per
/// unmet requirement.
pub(crate) fn apply_rule(
    field: &str,
    value: Option<&Value>,
    rule: &str,
    whole: &Value,
    ctx: &dyn ValidationContext,
    policy: &PasswordPolicy,
) -> Vec<RuleFailure> {
    let (name, arg) = match rule.split_once(':') {
        Some((n, a)) => (n, Some(a)),
        None => (rule, None),
    };

    // `required` needs raw presence, not the dereferenced value.
    if name == "required" {
        return if value.is_none() || value == Some(&Value::Null) {
            vec![ValidationError::new("required", format!("The {field} field is required.")).into()]
        } else {
            Vec::new()
        };
    }

    // `confirmed` needs the sibling `<field>_confirmation` key from the whole
    // payload, so it runs before the absent/null short-circuit: Laravel still
    // reports a mismatch when the confirmation key is missing.
    if name == "confirmed" {
        return confirmed(field, value, whole);
    }

    let present = match value {
        Some(Value::Null) | None => {
            // Optional fields: skip non-required rules when absent/null.
            return Vec::new();
        }
        Some(v) => v,
    };

    let label = humanize(field);
    let result: Result<(), RuleFailure> = match name {
        "string" => {
            if present.is_string() {
                Ok(())
            } else {
                Err(ValidationError::new("string", format!("The {label} must be a string.")).into())
            }
        }
        "email" => {
            use validator::ValidateEmail;
            let valid = present
                .as_str()
                .map(|s| s.validate_email())
                .unwrap_or(false);
            if valid {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "email",
                    format!("The {label} must be a valid email address."),
                )
                .into())
            }
        }
        "timezone" => {
            let valid = present
                .as_str()
                .map(|s| rustasea_timezone::validate(s).is_ok())
                .unwrap_or(false);
            if valid {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "timezone",
                    format!("The {label} must be a valid timezone."),
                )
                .into())
            }
        }
        "min" => {
            let n = parse_arg::<u64>(arg).unwrap_or(0);
            let len = value_len(present);
            if len >= n {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "min",
                    format!("The {label} must be at least {n} characters."),
                )
                .into())
            }
        }
        "max" => {
            let n = parse_arg::<u64>(arg).unwrap_or(u64::MAX);
            let len = value_len(present);
            if len <= n {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "max",
                    format!("The {label} must not exceed {n} characters."),
                )
                .into())
            }
        }
        "in_array" | "in" | "strict_in_array" => {
            // Each item is read like a `contains` needle: JSON when it parses
            // (`1` is a number, `"1"` a string), else the raw text, so a
            // bare-word list such as `in:admin,editor` allows those strings.
            let allow = Value::Array(
                parse_list(arg)
                    .iter()
                    .map(|item| parse_needle(Some(item)))
                    .collect(),
            );
            if in_array_strict(&allow, present) {
                Ok(())
            } else {
                Err(
                    ValidationError::new("in_array", format!("The selected {label} is invalid."))
                        .into(),
                )
            }
        }
        "contains" | "contains_strict" | "strict_contains" => {
            let needle = parse_needle(arg);
            if contains_strict(present, &needle) {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "contains_strict",
                    format!("The {label} must contain {arg:?} strictly."),
                )
                .into())
            }
        }
        "doesnt_contain" => {
            let needle = parse_needle(arg);
            if doesnt_contain(present, &needle) {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "doesnt_contain",
                    format!("The {label} must not contain {arg:?}."),
                )
                .into())
            }
        }
        "password" => return password(field, present, policy),
        "unique" => return unique(field, present, arg, ctx),
        "current_password" => return current_password(field, present, ctx),
        _ => Err(ValidationError::new(
            "unknown_rule",
            format!("Unknown validation rule {rule:?}."),
        )
        .into()),
    };

    match result {
        Ok(()) => Vec::new(),
        Err(failure) => vec![failure],
    }
}

/// `confirmed` — `X` must equal `X_confirmation` in the same payload.
///
/// Emits a distinct message when the confirmation key is absent so the client
/// can tell "missing confirmation" from "confirmation does not match".
fn confirmed(field: &str, value: Option<&Value>, whole: &Value) -> Vec<RuleFailure> {
    let label = humanize(field);
    let confirmation_key = format!("{field}_confirmation");
    let Some(confirmation) = whole.get(&confirmation_key) else {
        return vec![ValidationError::new(
            "confirmed",
            format!("The {label} confirmation is missing."),
        )
        .into()];
    };
    if value == Some(confirmation) {
        Vec::new()
    } else {
        vec![ValidationError::new(
            "confirmed",
            format!("The {label} confirmation does not match."),
        )
        .into()]
    }
}

/// `password` — evaluate the value against the configured policy.
///
/// One failure per unmet requirement. A policy that enables `uncompromised`
/// fails closed with a typed unsupported error, because no breach-list checker
/// is available.
fn password(field: &str, present: &Value, policy: &PasswordPolicy) -> Vec<RuleFailure> {
    let Some(candidate) = present.as_str() else {
        return vec![ValidationError::new(
            "password",
            format!("The {} must be a string.", humanize(field)),
        )
        .into()];
    };
    if policy.uncompromised {
        return vec![RuleFailure::Context(RuleError::Unsupported {
            field: field.to_string(),
            rule: "password".to_string(),
            detail: "the `uncompromised` check requires a breach-list checker that this crate \
                     does not provide"
                .to_string(),
        })];
    }
    policy
        .violations(candidate)
        .into_iter()
        .map(|v| ValidationError::new(v.code, v.message).into())
        .collect()
}

/// `unique:table,column[,ignore_id]` — delegate the conflict check to context.
fn unique(
    field: &str,
    present: &Value,
    arg: Option<&str>,
    ctx: &dyn ValidationContext,
) -> Vec<RuleFailure> {
    let params = parse_list(arg);
    let (Some(table), Some(column)) = (params.first(), params.get(1)) else {
        return vec![RuleFailure::Context(RuleError::Unsupported {
            field: field.to_string(),
            rule: "unique".to_string(),
            detail: "the `unique` rule requires `unique:table,column[,ignore_id]`".to_string(),
        })];
    };
    let ignore_id = params.get(2).map(String::as_str);
    let Some(value) = present.as_str() else {
        return vec![ValidationError::new(
            "unique",
            format!("The {} must be a string.", humanize(field)),
        )
        .into()];
    };
    match ctx.is_unique(table, column, value, ignore_id) {
        Some(true) => Vec::new(),
        Some(false) => vec![ValidationError::new(
            "unique",
            format!("The {} has already been taken.", humanize(field)),
        )
        .into()],
        // Fail closed: unknown is not the same as available.
        None => vec![RuleFailure::Context(RuleError::MissingContext {
            field: field.to_string(),
            rule: "unique".to_string(),
        })],
    }
}

/// `current_password` — verify the value against the authenticated user.
fn current_password(field: &str, present: &Value, ctx: &dyn ValidationContext) -> Vec<RuleFailure> {
    let Some(value) = present.as_str() else {
        return vec![ValidationError::new(
            "current_password",
            format!("The {} must be a string.", humanize(field)),
        )
        .into()];
    };
    match ctx.current_password_matches(value) {
        Some(true) => Vec::new(),
        Some(false) => vec![ValidationError::new(
            "current_password",
            "The provided password does not match your current password.".to_string(),
        )
        .into()],
        None => vec![RuleFailure::Context(RuleError::MissingContext {
            field: field.to_string(),
            rule: "current_password".to_string(),
        })],
    }
}

/// Length of a JSON value treated as text or collection.
fn value_len(v: &Value) -> u64 {
    match v {
        Value::String(s) => s.chars().count() as u64,
        Value::Array(a) => a.len() as u64,
        Value::Object(o) => o.len() as u64,
        Value::Number(n) => n.to_string().len() as u64,
        Value::Bool(_) | Value::Null => 0,
    }
}

/// Parse `min:3` / `max:255` arguments.
fn parse_arg<T: std::str::FromStr>(arg: Option<&str>) -> Option<T> {
    arg.and_then(|a| a.trim().parse().ok())
}

/// Parse `in:a,b,c` / `unique:users,email,42` into individual string items.
fn parse_list(arg: Option<&str>) -> Vec<String> {
    arg.map(|a| {
        a.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

/// Parse a scalar needle from a rule argument (JSON if possible, else raw).
fn parse_needle(arg: Option<&str>) -> Value {
    match arg {
        Some(a) => serde_json::from_str(a).unwrap_or_else(|_| Value::String(a.to_string())),
        None => Value::Null,
    }
}

/// Field label for messages (snake_case -> words).
fn humanize(field: &str) -> String {
    field.replace(['_', '.'], " ").to_string()
}

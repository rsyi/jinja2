//! Extended Python method compatibility for Jinja2 parity.
//!
//! This module wraps minijinja-contrib's `pycompat::unknown_method_callback`
//! and adds additional methods that pycompat doesn't cover:
//!
//! - List mutation: `append`, `extend`, `insert`, `pop`, `remove`, `copy`,
//!   `sort`, `reverse`, `index`, `clear`
//! - Dict mutation: `update`, `pop`, `copy`, `setdefault`, `clear`
//! - Extra string: `center`, `ljust`, `rjust`, `zfill`
//!
//! Note: minijinja values are immutable, so mutation methods (append, extend,
//! insert, remove, sort, reverse, update, clear) are accepted but operate as
//! no-ops, returning `""`. This matches the behavior of dbt's Rust-based
//! Jinja engines and prevents templates from erroring on common Jinja2/Python
//! patterns like `{% do items.append(x) %}`.

use minijinja::value::{from_args, Value, ValueKind};
use minijinja::{Error, ErrorKind, State};

/// Extended unknown method callback with full Jinja2/Python method parity.
///
/// Delegates to minijinja-contrib's pycompat for known methods, then handles
/// additional methods that pycompat doesn't cover.
pub fn unknown_method_callback(
    state: &State,
    value: &Value,
    method: &str,
    args: &[Value],
) -> Result<Value, Error> {
    // Try pycompat first
    match minijinja_contrib::pycompat::unknown_method_callback(state, value, method, args) {
        Ok(v) => return Ok(v),
        Err(e) if e.kind() == ErrorKind::UnknownMethod => {
            // Fall through to our extensions
        }
        Err(e) => return Err(e),
    }

    // Handle by value kind
    match value.kind() {
        ValueKind::String => extended_string_methods(value, method, args),
        ValueKind::Seq => seq_methods(value, method, args),
        ValueKind::Map => map_methods(value, method, args),
        _ => {
            // For undefined/chainable values, return empty string to avoid errors
            if value.is_undefined() {
                Ok(Value::from(""))
            } else {
                Err(Error::from(ErrorKind::UnknownMethod))
            }
        }
    }
}

fn extended_string_methods(value: &Value, method: &str, args: &[Value]) -> Result<Value, Error> {
    let Some(s) = value.as_str() else {
        return Err(Error::from(ErrorKind::UnknownMethod));
    };

    match method {
        "center" => {
            let (width, fillchar): (usize, Option<&str>) = from_args(args)?;
            let fill = fillchar.and_then(|s| s.chars().next()).unwrap_or(' ');
            let len = s.chars().count();
            if len >= width {
                Ok(Value::from(s))
            } else {
                let total_pad = width - len;
                let left_pad = total_pad / 2;
                let right_pad = total_pad - left_pad;
                let mut result = String::with_capacity(width);
                for _ in 0..left_pad {
                    result.push(fill);
                }
                result.push_str(s);
                for _ in 0..right_pad {
                    result.push(fill);
                }
                Ok(Value::from(result))
            }
        }
        "ljust" => {
            let (width, fillchar): (usize, Option<&str>) = from_args(args)?;
            let fill = fillchar.and_then(|s| s.chars().next()).unwrap_or(' ');
            let len = s.chars().count();
            if len >= width {
                Ok(Value::from(s))
            } else {
                let mut result = String::from(s);
                for _ in 0..(width - len) {
                    result.push(fill);
                }
                Ok(Value::from(result))
            }
        }
        "rjust" => {
            let (width, fillchar): (usize, Option<&str>) = from_args(args)?;
            let fill = fillchar.and_then(|s| s.chars().next()).unwrap_or(' ');
            let len = s.chars().count();
            if len >= width {
                Ok(Value::from(s))
            } else {
                let mut result = String::with_capacity(width);
                for _ in 0..(width - len) {
                    result.push(fill);
                }
                result.push_str(s);
                Ok(Value::from(result))
            }
        }
        "zfill" => {
            let (width,): (usize,) = from_args(args)?;
            let len = s.chars().count();
            if len >= width {
                Ok(Value::from(s))
            } else {
                let (sign, num_part) = if s.starts_with('-') || s.starts_with('+') {
                    (&s[..1], &s[1..])
                } else {
                    ("", s)
                };
                let pad = width - len;
                let mut result = String::with_capacity(width);
                result.push_str(sign);
                for _ in 0..pad {
                    result.push('0');
                }
                result.push_str(num_part);
                Ok(Value::from(result))
            }
        }
        _ => Err(Error::from(ErrorKind::UnknownMethod)),
    }
}

fn seq_methods(value: &Value, method: &str, args: &[Value]) -> Result<Value, Error> {
    match method {
        // Mutation methods - accepted as no-ops (minijinja values are immutable)
        // Returns "" so templates using {% do list.append(x) %} don't error.
        "append" | "extend" | "insert" | "remove" | "clear" | "sort" | "reverse" => {
            let _ = args; // Accept any args
            Ok(Value::from(""))
        }
        "pop" => {
            // pop() returns a default value if provided, or UNDEFINED
            if args.len() >= 2 {
                Ok(args[1].clone())
            } else {
                Ok(Value::UNDEFINED)
            }
        }
        "copy" => {
            // Return the value itself (it's immutable anyway)
            Ok(value.clone())
        }
        "index" => {
            let (needle,): (&Value,) = from_args(args)?;
            if let Ok(iter) = value.try_iter() {
                for (idx, item) in iter.enumerate() {
                    if &item == needle {
                        return Ok(Value::from(idx));
                    }
                }
            }
            Err(Error::new(ErrorKind::InvalidOperation, "value not in list"))
        }
        _ => Err(Error::from(ErrorKind::UnknownMethod)),
    }
}

fn map_methods(value: &Value, method: &str, args: &[Value]) -> Result<Value, Error> {
    match method {
        // Mutation methods - no-ops
        "update" | "clear" => {
            let _ = args;
            Ok(Value::from(""))
        }
        "pop" => {
            // dict.pop(key[, default]) - try to get value, fall back to default
            let key = args.first().ok_or_else(|| {
                Error::new(
                    ErrorKind::MissingArgument,
                    "pop() requires at least 1 argument",
                )
            })?;
            match value.get_item(key) {
                Ok(v) if !v.is_undefined() => Ok(v),
                _ => {
                    if args.len() >= 2 {
                        Ok(args[1].clone())
                    } else {
                        Ok(Value::UNDEFINED)
                    }
                }
            }
        }
        "copy" => Ok(value.clone()),
        "setdefault" => {
            // dict.setdefault(key, default) - return value if exists, else default
            let key = args.first().ok_or_else(|| {
                Error::new(
                    ErrorKind::MissingArgument,
                    "setdefault() requires at least 1 argument",
                )
            })?;
            match value.get_item(key) {
                Ok(v) if !v.is_undefined() => Ok(v),
                _ => Ok(args.get(1).cloned().unwrap_or(Value::from(()))),
            }
        }
        _ => Err(Error::from(ErrorKind::UnknownMethod)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_center() {
        let v = Value::from("hello");
        let result = extended_string_methods(&v, "center", &[Value::from(20i32)]).unwrap();
        assert_eq!(result.to_string(), "       hello        ");
    }

    #[test]
    fn test_string_ljust() {
        let v = Value::from("hello");
        let result = extended_string_methods(&v, "ljust", &[Value::from(10i32)]).unwrap();
        assert_eq!(result.to_string(), "hello     ");
    }

    #[test]
    fn test_string_rjust() {
        let v = Value::from("hello");
        let result = extended_string_methods(&v, "rjust", &[Value::from(10i32)]).unwrap();
        assert_eq!(result.to_string(), "     hello");
    }

    #[test]
    fn test_string_zfill() {
        let v = Value::from("42");
        let result = extended_string_methods(&v, "zfill", &[Value::from(5i32)]).unwrap();
        assert_eq!(result.to_string(), "00042");

        let v = Value::from("-42");
        let result = extended_string_methods(&v, "zfill", &[Value::from(5i32)]).unwrap();
        assert_eq!(result.to_string(), "-0042");
    }

    #[test]
    fn test_list_append_noop() {
        let v = Value::from(vec![Value::from(1), Value::from(2)]);
        let result = seq_methods(&v, "append", &[Value::from(3)]).unwrap();
        assert_eq!(result.to_string(), "");
    }

    #[test]
    fn test_list_copy() {
        let v = Value::from(vec![Value::from(1), Value::from(2)]);
        let result = seq_methods(&v, "copy", &[]).unwrap();
        assert_eq!(result.to_string(), v.to_string());
    }

    #[test]
    fn test_dict_pop_with_default() {
        let v = Value::from_serialize(&serde_json::json!({"a": 1}));
        let result = map_methods(&v, "pop", &[Value::from("b"), Value::from(42)]).unwrap();
        assert_eq!(result.to_string(), "42");
    }

    #[test]
    fn test_dict_setdefault() {
        let v = Value::from_serialize(&serde_json::json!({"a": 1}));
        // Key exists
        let result = map_methods(&v, "setdefault", &[Value::from("a"), Value::from(99)]).unwrap();
        assert_eq!(result.to_string(), "1");
        // Key doesn't exist
        let result = map_methods(&v, "setdefault", &[Value::from("b"), Value::from(99)]).unwrap();
        assert_eq!(result.to_string(), "99");
    }
}

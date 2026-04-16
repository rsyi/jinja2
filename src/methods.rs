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

use std::sync::RwLock;
use std::fmt;
use std::sync::Arc;

use indexmap::IndexMap;
use minijinja::value::{from_args, Enumerator, Object, ObjectRepr, Value, ValueKind};
use minijinja::{Error, ErrorKind, State};

/// A mutable list that supports `append`, `extend`, etc. in Jinja templates.
///
/// Unlike minijinja's built-in immutable sequences, `MutableList` truly
/// mutates in place when `.append()` is called, so the common Jinja2/dbt
/// pattern `{% set cols = [...] %} {{ cols.append(item) }}` works correctly
/// even across scopes (e.g., inside `{% if %}` blocks).
#[derive(Debug)]
pub struct MutableList {
    items: RwLock<Vec<Value>>,
}

impl MutableList {
    /// Create a new MutableList from a Vec of Values.
    pub fn new(items: Vec<Value>) -> Self {
        Self {
            items: RwLock::new(items),
        }
    }

    /// Create a Value wrapping a new MutableList.
    pub fn from_values(items: Vec<Value>) -> Value {
        Value::from_object(Self::new(items))
    }

    /// Create a MutableList Value from an iterable Value.
    pub fn from_value(val: &Value) -> Value {
        let items: Vec<Value> = val.try_iter().map(|i| i.collect()).unwrap_or_default();
        Self::from_values(items)
    }
}

impl fmt::Display for MutableList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let items = self.items.read().unwrap();
        write!(f, "[")?;
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{item}")?;
        }
        write!(f, "]")
    }
}

impl Object for MutableList {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Seq
    }

    fn get_value(self: &Arc<Self>, index: &Value) -> Option<Value> {
        let items = self.items.read().unwrap();
        if let Some(idx) = index.as_usize() {
            items.get(idx).cloned()
        } else {
            None
        }
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        let items = self.items.read().unwrap().clone();
        Enumerator::Values(items)
    }

    fn enumerator_len(self: &Arc<Self>) -> Option<usize> {
        Some(self.items.read().unwrap().len())
    }

    fn call_method(
        self: &Arc<Self>,
        _state: &minijinja::State,
        method: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        match method {
            "append" => {
                if let Some(item) = args.first() {
                    self.items.write().unwrap().push(item.clone());
                }
                Ok(Value::from(""))
            }
            "extend" => {
                if let Some(iterable) = args.first() {
                    if let Ok(iter) = iterable.try_iter() {
                        let mut items = self.items.write().unwrap();
                        for item in iter {
                            items.push(item);
                        }
                    }
                }
                Ok(Value::from(""))
            }
            "insert" => {
                if args.len() >= 2 {
                    if let Some(idx) = args[0].as_usize() {
                        let mut items = self.items.write().unwrap();
                        let idx = idx.min(items.len());
                        items.insert(idx, args[1].clone());
                    }
                }
                Ok(Value::from(""))
            }
            "pop" => {
                let mut items = self.items.write().unwrap();
                if let Some(idx_val) = args.first() {
                    if let Some(idx) = idx_val.as_usize() {
                        if idx < items.len() {
                            return Ok(items.remove(idx));
                        }
                    }
                }
                if !items.is_empty() {
                    Ok(items.pop().unwrap())
                } else {
                    Ok(Value::UNDEFINED)
                }
            }
            "remove" => {
                if let Some(needle) = args.first() {
                    let mut items = self.items.write().unwrap();
                    if let Some(pos) = items.iter().position(|x| x == needle) {
                        items.remove(pos);
                    }
                }
                Ok(Value::from(""))
            }
            "clear" => {
                self.items.write().unwrap().clear();
                Ok(Value::from(""))
            }
            "copy" => {
                let items = self.items.read().unwrap().clone();
                Ok(MutableList::from_values(items))
            }
            "reverse" => {
                self.items.write().unwrap().reverse();
                Ok(Value::from(""))
            }
            "sort" => {
                // Basic sort by string representation
                self.items.write().unwrap().sort_by(|a, b| {
                    a.to_string().cmp(&b.to_string())
                });
                Ok(Value::from(""))
            }
            "index" => {
                let (needle,): (&Value,) = from_args(args)?;
                let items = self.items.read().unwrap();
                for (idx, item) in items.iter().enumerate() {
                    if item == needle {
                        return Ok(Value::from(idx));
                    }
                }
                Err(Error::new(ErrorKind::InvalidOperation, "value not in list"))
            }
            _ => Err(Error::from(ErrorKind::UnknownMethod)),
        }
    }
}

/// Global function to create a MutableList from a list literal.
/// Usage in templates: `{% set columns = _mklist([...]) %}`
pub fn mklist(args: &[Value]) -> Result<Value, Error> {
    let items: Vec<Value> = if let Some(val) = args.first() {
        val.try_iter().map(|i| i.collect()).unwrap_or_default()
    } else {
        Vec::new()
    };
    Ok(MutableList::from_values(items))
}

/// A mutable dictionary that supports `update`, `pop`, etc. in Jinja templates.
///
/// Unlike minijinja's built-in immutable maps, `MutableDict` truly mutates
/// in place when `.update()` or `.pop()` is called, so common Jinja2/dbt
/// patterns like `{% do config.update({"key": "val"}) %}` work correctly
/// even across scopes.
#[derive(Debug)]
pub struct MutableDict {
    items: RwLock<IndexMap<String, Value>>,
}

impl MutableDict {
    /// Create a new MutableDict from an IndexMap.
    pub fn new(items: IndexMap<String, Value>) -> Self {
        Self {
            items: RwLock::new(items),
        }
    }

    /// Create a Value wrapping a new MutableDict.
    pub fn from_index_map(items: IndexMap<String, Value>) -> Value {
        Value::from_object(Self::new(items))
    }

    /// Create a MutableDict Value from a map-like Value.
    pub fn from_value(val: &Value) -> Value {
        let mut map = IndexMap::new();
        if let Ok(keys) = val.try_iter() {
            for key in keys {
                if let Some(k) = key.as_str() {
                    let v = val.get_item(&key).unwrap_or(Value::UNDEFINED);
                    map.insert(k.to_string(), v);
                }
            }
        }
        Self::from_index_map(map)
    }
}

impl fmt::Display for MutableDict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let items = self.items.read().unwrap();
        write!(f, "{{")?;
        for (i, (key, value)) in items.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "'{key}': {value}")?;
        }
        write!(f, "}}")
    }
}

impl Object for MutableDict {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let items = self.items.read().unwrap();
        if let Some(k) = key.as_str() {
            items.get(k).cloned()
        } else {
            None
        }
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        let items = self.items.read().unwrap();
        let keys: Vec<Value> = items.keys().map(|k| Value::from(k.clone())).collect();
        Enumerator::Values(keys)
    }

    fn enumerator_len(self: &Arc<Self>) -> Option<usize> {
        Some(self.items.read().unwrap().len())
    }

    fn call_method(
        self: &Arc<Self>,
        _state: &State,
        method: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        match method {
            "update" => {
                if let Some(other) = args.first() {
                    if other.kind() == ValueKind::Map {
                        // Merge from another map-like value
                        let mut items = self.items.write().unwrap();
                        if let Ok(keys) = other.try_iter() {
                            for key in keys {
                                if let Some(k) = key.as_str() {
                                    let v = other.get_item(&key).unwrap_or(Value::UNDEFINED);
                                    items.insert(k.to_string(), v);
                                }
                            }
                        }
                    }
                }
                Ok(Value::from(""))
            }
            "pop" => {
                let key = args.first().ok_or_else(|| {
                    Error::new(
                        ErrorKind::MissingArgument,
                        "pop() requires at least 1 argument",
                    )
                })?;
                if let Some(k) = key.as_str() {
                    let mut items = self.items.write().unwrap();
                    if let Some(v) = items.shift_remove(k) {
                        return Ok(v);
                    }
                }
                if args.len() >= 2 {
                    Ok(args[1].clone())
                } else {
                    Ok(Value::UNDEFINED)
                }
            }
            "get" => {
                let key = args.first().ok_or_else(|| {
                    Error::new(
                        ErrorKind::MissingArgument,
                        "get() requires at least 1 argument",
                    )
                })?;
                if let Some(k) = key.as_str() {
                    let items = self.items.read().unwrap();
                    if let Some(v) = items.get(k) {
                        return Ok(v.clone());
                    }
                }
                Ok(args.get(1).cloned().unwrap_or(Value::from(())))
            }
            "setdefault" => {
                let key = args.first().ok_or_else(|| {
                    Error::new(
                        ErrorKind::MissingArgument,
                        "setdefault() requires at least 1 argument",
                    )
                })?;
                if let Some(k) = key.as_str() {
                    let mut items = self.items.write().unwrap();
                    if let Some(v) = items.get(k) {
                        return Ok(v.clone());
                    }
                    let default = args.get(1).cloned().unwrap_or(Value::from(()));
                    items.insert(k.to_string(), default.clone());
                    return Ok(default);
                }
                Ok(Value::from(()))
            }
            "keys" => {
                let items = self.items.read().unwrap();
                let keys: Vec<Value> = items.keys().map(|k| Value::from(k.clone())).collect();
                Ok(Value::from(keys))
            }
            "values" => {
                let items = self.items.read().unwrap();
                let vals: Vec<Value> = items.values().cloned().collect();
                Ok(Value::from(vals))
            }
            "items" => {
                let items = self.items.read().unwrap();
                let pairs: Vec<Value> = items
                    .iter()
                    .map(|(k, v)| Value::from(vec![Value::from(k.clone()), v.clone()]))
                    .collect();
                Ok(Value::from(pairs))
            }
            "clear" => {
                self.items.write().unwrap().clear();
                Ok(Value::from(""))
            }
            "copy" => {
                let items = self.items.read().unwrap().clone();
                Ok(MutableDict::from_index_map(items))
            }
            _ => Err(Error::from(ErrorKind::UnknownMethod)),
        }
    }
}

/// Global function to create a MutableDict from a dict literal.
/// Usage in templates: `{% set config = _mkdict({"key": "val"}) %}`
pub fn mkdict(args: &[Value]) -> Result<Value, Error> {
    let mut map = IndexMap::new();
    if let Some(val) = args.first() {
        if let Ok(keys) = val.try_iter() {
            for key in keys {
                if let Some(k) = key.as_str() {
                    let v = val.get_item(&key).unwrap_or(Value::UNDEFINED);
                    map.insert(k.to_string(), v);
                }
            }
        }
    }
    Ok(MutableDict::from_index_map(map))
}

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

    #[test]
    fn test_mutable_list_append() {
        use minijinja::{Environment, context};
        let mut env = Environment::new();
        crate::add_jinja2_compat(&mut env);
        env.add_template("test", r#"
{%- set cols = _mklist([1, 2]) -%}
{%- if true -%}
  {{ cols.append(3) }}
{%- endif -%}
len={{ cols|length }}
"#).unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!{}).unwrap();
        eprintln!("MutableList append: '{}'", result.trim());
        assert!(result.contains("len=3"), "Expected len=3, got: {}", result);
    }

    #[test]
    fn test_mutable_list_iteration() {
        use minijinja::{Environment, context};
        let mut env = Environment::new();
        crate::add_jinja2_compat(&mut env);
        env.add_template("test", r#"
{%- set cols = _mklist([{"name": "a"}, {"name": "b"}]) -%}
{%- for col in cols -%}{{ col.name }}{% if not loop.last %},{% endif %}{%- endfor -%}
"#).unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!{}).unwrap();
        eprintln!("MutableList iter: '{}'", result.trim());
        assert_eq!(result.trim(), "a,b");
    }

    #[test]
    fn test_mutable_dict_update() {
        use minijinja::{Environment, context};
        let mut env = Environment::new();
        crate::add_jinja2_compat(&mut env);
        env.add_template("test", r#"
{%- set d = _mkdict({"a": 1}) -%}
{{ d.update({"b": 2, "c": 3}) -}}
a={{ d.a }},b={{ d.b }},c={{ d.c }},len={{ d|length }}
"#).unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!{}).unwrap();
        eprintln!("MutableDict update: '{}'", result.trim());
        assert!(result.contains("a=1"), "Expected a=1, got: {}", result);
        assert!(result.contains("b=2"), "Expected b=2, got: {}", result);
        assert!(result.contains("c=3"), "Expected c=3, got: {}", result);
        assert!(result.contains("len=3"), "Expected len=3, got: {}", result);
    }

    #[test]
    fn test_mutable_dict_iteration() {
        use minijinja::{Environment, context};
        let mut env = Environment::new();
        crate::add_jinja2_compat(&mut env);
        env.add_template("test", r#"
{%- set d = _mkdict({"name": "alice", "age": "30"}) -%}
{%- for k, v in d|items -%}{{ k }}={{ v }}{% if not loop.last %},{% endif %}{%- endfor -%}
"#).unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!{}).unwrap();
        eprintln!("MutableDict iter: '{}'", result.trim());
        assert!(result.contains("name=alice"), "Expected name=alice, got: {}", result);
        assert!(result.contains("age=30"), "Expected age=30, got: {}", result);
    }
}

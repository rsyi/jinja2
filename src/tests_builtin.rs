//! Additional test functions for Jinja2 parity.
//!
//! Minijinja already provides most Jinja2 tests. This module adds the
//! remaining ones.

use minijinja::value::Value;

/// Test if a value is callable.
///
/// In Jinja2, `callable()` checks if the value can be called as a function.
/// In minijinja, this corresponds to objects that implement the call protocol.
///
/// ```jinja
/// {% if func is callable %}...{% endif %}
/// ```
pub fn is_callable(value: &Value) -> bool {
    // In minijinja, callable values are Object-backed values that implement call.
    // We check if it's an object — minijinja functions, macros, cyclers, joiners
    // are all objects.
    value.as_object().is_some()
}

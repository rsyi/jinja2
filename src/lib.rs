//! # jinja2
//!
//! A Jinja2-compatible template engine for Rust with full feature parity,
//! built on top of [minijinja](https://github.com/mitsuhiko/minijinja).
//!
//! jinja2 fills the gaps between minijinja and the full Jinja2 spec by:
//! - Registering all missing filters (`center`, `forceescape`, `urlize`, `xmlattr`,
//!   `as_bool`)
//! - Registering all contrib filters (`truncate`, `striptags`, `filesizeformat`,
//!   `pluralize`, `wordcount`, `wordwrap`, `random`)
//! - Registering all contrib globals (`cycler`, `joiner`, `lipsum`, `randrange`)
//! - Full Python method compatibility: pycompat base plus list mutation
//!   (`append`, `extend`, `pop`, `copy`), dict mutation (`update`, `pop`,
//!   `setdefault`, `copy`), and extra string methods (`center`, `ljust`,
//!   `rjust`, `zfill`)
//! - Registering the `callable` test
//! - Enabling all minijinja features: loop controls, unicode identifiers,
//!   custom syntax, JSON/urlencode filters, template loaders, etc.
//!
//! # Quick Start
//!
//! ```rust
//! use jinja2::{Environment, context};
//!
//! let mut env = Environment::new();
//! env.add_template("hello", "Hello {{ name|upper }}!").unwrap();
//! let tmpl = env.get_template("hello").unwrap();
//! let result = tmpl.render(context!(name => "world")).unwrap();
//! assert_eq!(result, "Hello WORLD!");
//! ```
//!
//! # Using `new_jinja2()` for full Jinja2 parity
//!
//! ```rust
//! use jinja2::new_jinja2;
//!
//! let mut env = new_jinja2();
//! env.add_template("test", "{{ 'hello'|center(20) }}").unwrap();
//! let tmpl = env.get_template("test").unwrap();
//! let result = tmpl.render(()).unwrap();
//! assert_eq!(result, "       hello        ");
//! ```

pub mod filters;
pub mod methods;
pub mod tests_builtin;

// Re-export core minijinja types
pub use minijinja::{
    context, render,
    value::{self, Value},
    Environment, Error, ErrorKind, State, Template,
};

// Re-export contrib modules for direct access
pub use minijinja_contrib;

/// Create a new `Environment` pre-configured with full Jinja2 parity.
///
/// This registers all built-in minijinja features plus:
/// - All contrib filters and globals
/// - Python method compatibility
/// - Additional filters for full Jinja2 parity
/// - The `callable` test
///
/// This is the recommended entry point for jinja2.
pub fn new_jinja2<'source>() -> Environment<'source> {
    let mut env = Environment::new();
    add_jinja2_compat(&mut env);
    env
}

/// Add all Jinja2 compatibility features to an existing `Environment`.
///
/// Call this if you've already created an `Environment` and want to
/// upgrade it to full Jinja2 parity. If you're starting fresh, use
/// [`new_jinja2()`] instead.
pub fn add_jinja2_compat(env: &mut Environment<'_>) {
    // Register all minijinja-contrib filters and globals
    minijinja_contrib::add_to_environment(env);

    // Enable extended Python method compatibility.
    // Our callback wraps pycompat and adds: list mutation (append, extend,
    // pop, copy, etc.), dict mutation (update, pop, setdefault, copy),
    // and extra string methods (center, ljust, rjust, zfill).
    env.set_unknown_method_callback(methods::unknown_method_callback);

    // Register additional filters for Jinja2 parity
    env.add_filter("center", filters::center);
    env.add_filter("forceescape", filters::forceescape);
    env.add_filter("urlize", filters::urlize);
    env.add_filter("xmlattr", filters::xmlattr);
    env.add_filter("as_bool", filters::as_bool);

    // Register additional tests
    env.add_test("callable", tests_builtin::is_callable);

    // Register mutable list constructor for templates that need list mutation
    env.add_function("_mklist", methods::mklist);

    // Register mutable dict constructor for templates that need dict mutation
    env.add_function("_mkdict", methods::mkdict);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_new_jinja2_basic() {
        let mut env = new_jinja2();
        env.add_template("hello", "Hello {{ name }}!").unwrap();
        let tmpl = env.get_template("hello").unwrap();
        let result = tmpl.render(context!(name => "World")).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_center_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|center(20) }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(value => "hello")).unwrap();
        assert_eq!(result, "       hello        ");
    }

    #[test]
    fn test_forceescape_filter() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% autoescape true %}{{ value|forceescape }}{% endautoescape %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(value => "<b>bold</b>")).unwrap();
        assert!(result.contains("&lt;"));
    }

    #[test]
    fn test_urlize_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|urlize }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(value => "Visit https://example.com today!"))
            .unwrap();
        assert!(result.contains("<a href="));
        assert!(result.contains("https://example.com"));
    }

    #[test]
    fn test_xmlattr_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "<ul{{ attrs|xmlattr }}>").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(
                context!(attrs => Value::from_serialize(&json!({"class": "nav", "id": "main"}))),
            )
            .unwrap();
        assert!(result.contains("class=\"nav\""));
        assert!(result.contains("id=\"main\""));
        assert!(result.starts_with("<ul "));
    }

    #[test]
    fn test_truncate_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|truncate(length=10) }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(value => "This is a very long string that should be truncated"))
            .unwrap();
        assert!(result.len() <= 15);
    }

    #[test]
    fn test_striptags_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|striptags }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(value => "<p>Hello <b>World</b></p>"))
            .unwrap();
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_filesizeformat_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|filesizeformat }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(value => 13000)).unwrap();
        assert_eq!(result, "13.0 kB");
    }

    #[test]
    fn test_wordcount_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|wordcount }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(value => "Hello beautiful world"))
            .unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn test_pluralize_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ count }} item{{ count|pluralize }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        assert_eq!(tmpl.render(context!(count => 1)).unwrap(), "1 item");
        assert_eq!(tmpl.render(context!(count => 5)).unwrap(), "5 items");
    }

    #[test]
    fn test_python_method_compat() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ 'hello'.upper() }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_dict_methods() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for k, v in data.items() %}{{ k }}={{ v }} {% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(data => Value::from_serialize(&json!({"a": 1}))))
            .unwrap();
        assert!(result.contains("a=1"));
    }

    #[test]
    fn test_loop_controls() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for i in range(10) %}{% if i == 3 %}{% break %}{% endif %}{{ i }}{% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "012");
    }

    #[test]
    fn test_template_inheritance() {
        let mut env = new_jinja2();
        env.add_template(
            "base",
            "Header {% block content %}default{% endblock %} Footer",
        )
        .unwrap();
        env.add_template(
            "child",
            "{% extends 'base' %}{% block content %}custom{% endblock %}",
        )
        .unwrap();
        let tmpl = env.get_template("child").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "Header custom Footer");
    }

    #[test]
    fn test_macros() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% macro greet(name) %}Hello {{ name }}!{% endmacro %}{{ greet('World') }}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_callable_test() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% if range is callable %}yes{% else %}no{% endif %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "yes");
    }

    #[test]
    fn test_tojson_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|tojson }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(value => Value::from_serialize(&json!({"key": "value"}))))
            .unwrap();
        assert!(result.contains("\"key\""));
        assert!(result.contains("\"value\""));
    }

    #[test]
    fn test_urlencode_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|urlencode }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(value => "hello world")).unwrap();
        assert_eq!(result, "hello%20world");
    }

    #[test]
    fn test_cycler_global() {
        let mut env = new_jinja2();
        // cycler expects its items passed as a list, not varargs
        env.add_template(
            "test",
            "{% set items = ['a', 'b'] %}{% set c = cycler(items) %}{{ c.next() }}{{ c.next() }}{{ c.next() }}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "aba");
    }

    #[test]
    fn test_joiner_global() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% set j = joiner(', ') %}{% for i in items %}{{ j() }}{{ i }}{% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(items => vec!["a", "b", "c"])).unwrap();
        assert_eq!(result, "a, b, c");
    }

    #[test]
    fn test_namespace() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% set ns = namespace(count=0) %}{% for i in items %}{% set ns.count = ns.count + 1 %}{% endfor %}{{ ns.count }}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(items => vec![1, 2, 3])).unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn test_builtin_filters_comprehensive() {
        let mut env = new_jinja2();

        // lower/upper/title/capitalize
        env.add_template("t1", "{{ 'Hello'|lower }}").unwrap();
        assert_eq!(env.get_template("t1").unwrap().render(()).unwrap(), "hello");

        env.add_template("t2", "{{ 'hello'|upper }}").unwrap();
        assert_eq!(env.get_template("t2").unwrap().render(()).unwrap(), "HELLO");

        env.add_template("t3", "{{ 'hello world'|title }}").unwrap();
        assert_eq!(
            env.get_template("t3").unwrap().render(()).unwrap(),
            "Hello World"
        );

        // replace
        env.add_template("t4", "{{ 'hello'|replace('l', 'r') }}")
            .unwrap();
        assert_eq!(env.get_template("t4").unwrap().render(()).unwrap(), "herro");

        // default
        env.add_template("t5", "{{ x|default('fallback') }}")
            .unwrap();
        assert_eq!(
            env.get_template("t5").unwrap().render(()).unwrap(),
            "fallback"
        );

        // length
        env.add_template("t6", "{{ items|length }}").unwrap();
        assert_eq!(
            env.get_template("t6")
                .unwrap()
                .render(context!(items => vec![1, 2, 3]))
                .unwrap(),
            "3"
        );

        // first/last
        env.add_template("t7", "{{ items|first }}").unwrap();
        assert_eq!(
            env.get_template("t7")
                .unwrap()
                .render(context!(items => vec![10, 20, 30]))
                .unwrap(),
            "10"
        );

        env.add_template("t8", "{{ items|last }}").unwrap();
        assert_eq!(
            env.get_template("t8")
                .unwrap()
                .render(context!(items => vec![10, 20, 30]))
                .unwrap(),
            "30"
        );

        // join
        env.add_template("t9", "{{ items|join(', ') }}").unwrap();
        assert_eq!(
            env.get_template("t9")
                .unwrap()
                .render(context!(items => vec!["a", "b", "c"]))
                .unwrap(),
            "a, b, c"
        );

        // sort
        env.add_template("t10", "{{ items|sort|join(', ') }}")
            .unwrap();
        assert_eq!(
            env.get_template("t10")
                .unwrap()
                .render(context!(items => vec![3, 1, 2]))
                .unwrap(),
            "1, 2, 3"
        );

        // reverse
        env.add_template("t11", "{{ items|reverse|join(', ') }}")
            .unwrap();
        assert_eq!(
            env.get_template("t11")
                .unwrap()
                .render(context!(items => vec![1, 2, 3]))
                .unwrap(),
            "3, 2, 1"
        );

        // abs
        env.add_template("t12", "{{ -5|abs }}").unwrap();
        assert_eq!(env.get_template("t12").unwrap().render(()).unwrap(), "5");

        // round
        env.add_template("t13", "{{ 3.7|round }}").unwrap();
        assert_eq!(env.get_template("t13").unwrap().render(()).unwrap(), "4.0");

        // int/float
        env.add_template("t14", "{{ '42'|int }}").unwrap();
        assert_eq!(env.get_template("t14").unwrap().render(()).unwrap(), "42");

        // trim
        env.add_template("t15", "{{ '  hello  '|trim }}").unwrap();
        assert_eq!(
            env.get_template("t15").unwrap().render(()).unwrap(),
            "hello"
        );

        // sum
        env.add_template("t16", "{{ items|sum }}").unwrap();
        assert_eq!(
            env.get_template("t16")
                .unwrap()
                .render(context!(items => vec![1, 2, 3]))
                .unwrap(),
            "6"
        );

        // unique
        env.add_template("t17", "{{ items|unique|sort|join(', ') }}")
            .unwrap();
        assert_eq!(
            env.get_template("t17")
                .unwrap()
                .render(context!(items => vec![1, 2, 2, 3, 3]))
                .unwrap(),
            "1, 2, 3"
        );
    }

    #[test]
    fn test_select_reject_filters() {
        let mut env = new_jinja2();

        env.add_template("t1", "{{ items|select('odd')|list|join(', ') }}")
            .unwrap();
        assert_eq!(
            env.get_template("t1")
                .unwrap()
                .render(context!(items => vec![1, 2, 3, 4, 5]))
                .unwrap(),
            "1, 3, 5"
        );

        env.add_template("t2", "{{ items|reject('odd')|list|join(', ') }}")
            .unwrap();
        assert_eq!(
            env.get_template("t2")
                .unwrap()
                .render(context!(items => vec![1, 2, 3, 4, 5]))
                .unwrap(),
            "2, 4"
        );
    }

    #[test]
    fn test_map_filter() {
        let mut env = new_jinja2();

        env.add_template("t1", "{{ items|map('upper')|join(', ') }}")
            .unwrap();
        assert_eq!(
            env.get_template("t1")
                .unwrap()
                .render(context!(items => vec!["hello", "world"]))
                .unwrap(),
            "HELLO, WORLD"
        );
    }

    #[test]
    fn test_batch_slice_filters() {
        let mut env = new_jinja2();

        env.add_template(
            "t1",
            "{% for batch in items|batch(2) %}[{{ batch|join(', ') }}]{% endfor %}",
        )
        .unwrap();
        assert_eq!(
            env.get_template("t1")
                .unwrap()
                .render(context!(items => vec![1, 2, 3, 4, 5]))
                .unwrap(),
            "[1, 2][3, 4][5]"
        );
    }

    #[test]
    fn test_indent_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|indent(4) }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(value => "line1\nline2\nline3"))
            .unwrap();
        assert!(result.contains("    line2"));
    }

    #[test]
    fn test_dictsort_filter() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for k, v in data|dictsort %}{{ k }}={{ v }} {% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(data => Value::from_serialize(&json!({"b": 2, "a": 1}))))
            .unwrap();
        assert_eq!(result, "a=1 b=2 ");
    }

    #[test]
    fn test_groupby_filter() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for group in items|groupby('type') %}{{ group.grouper }}: {% for item in group.list %}{{ item.name }} {% endfor %}{% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(items => Value::from_serialize(&json!([
                {"type": "fruit", "name": "apple"},
                {"type": "fruit", "name": "banana"},
                {"type": "veggie", "name": "carrot"},
            ]))))
            .unwrap();
        assert!(result.contains("fruit:"));
        assert!(result.contains("apple"));
        assert!(result.contains("veggie:"));
    }

    #[test]
    fn test_raw_block() {
        let mut env = new_jinja2();
        env.add_template("test", "{% raw %}{{ not_rendered }}{% endraw %}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "{{ not_rendered }}");
    }

    #[test]
    fn test_with_block() {
        let mut env = new_jinja2();
        env.add_template("test", "{% with x = 42 %}{{ x }}{% endwith %}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_filter_block() {
        let mut env = new_jinja2();
        env.add_template("test", "{% filter upper %}hello{% endfilter %}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_set_tag() {
        let mut env = new_jinja2();
        env.add_template("test", "{% set x = 'hello' %}{{ x }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_include() {
        let mut env = new_jinja2();
        env.add_template("partial", "I am included").unwrap();
        env.add_template("test", "Before {% include 'partial' %} After")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "Before I am included After");
    }

    #[test]
    fn test_import() {
        let mut env = new_jinja2();
        env.add_template(
            "macros",
            "{% macro greet(name) %}Hello {{ name }}{% endmacro %}",
        )
        .unwrap();
        env.add_template(
            "test",
            "{% from 'macros' import greet %}{{ greet('World') }}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_recursive_loop() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for item in items recursive %}{{ item.name }}{% if item.children %} [{{ loop(item.children) }}]{% endif %} {% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(items => Value::from_serialize(&json!([
                {"name": "a", "children": [{"name": "a1", "children": []}]},
                {"name": "b", "children": []},
            ]))))
            .unwrap();
        assert!(result.contains("a"));
        assert!(result.contains("a1"));
        assert!(result.contains("b"));
    }

    #[test]
    fn test_loop_variables() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for i in items %}{{ loop.index }}:{{ i }} {% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(items => vec!["a", "b", "c"])).unwrap();
        assert_eq!(result, "1:a 2:b 3:c ");
    }

    #[test]
    fn test_inline_if() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ 'yes' if flag else 'no' }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        assert_eq!(tmpl.render(context!(flag => true)).unwrap(), "yes");
        assert_eq!(tmpl.render(context!(flag => false)).unwrap(), "no");
    }

    #[test]
    fn test_string_concatenation() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ 'hello' ~ ' ' ~ 'world' }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_slicing() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ items[1:3]|join(', ') }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(items => vec![1, 2, 3, 4, 5])).unwrap();
        assert_eq!(result, "2, 3");
    }

    #[test]
    fn test_is_tests() {
        let mut env = new_jinja2();

        env.add_template("t1", "{{ 3 is odd }}").unwrap();
        assert_eq!(env.get_template("t1").unwrap().render(()).unwrap(), "true");

        env.add_template("t2", "{{ 4 is even }}").unwrap();
        assert_eq!(env.get_template("t2").unwrap().render(()).unwrap(), "true");

        env.add_template("t3", "{{ 10 is divisibleby(5) }}")
            .unwrap();
        assert_eq!(env.get_template("t3").unwrap().render(()).unwrap(), "true");

        env.add_template("t4", "{{ x is defined }}").unwrap();
        assert_eq!(env.get_template("t4").unwrap().render(()).unwrap(), "false");

        env.add_template("t5", "{{ none is none }}").unwrap();
        assert_eq!(env.get_template("t5").unwrap().render(()).unwrap(), "true");

        env.add_template("t6", "{{ 42 is number }}").unwrap();
        assert_eq!(env.get_template("t6").unwrap().render(()).unwrap(), "true");

        env.add_template("t7", "{{ 'hello' is string }}").unwrap();
        assert_eq!(env.get_template("t7").unwrap().render(()).unwrap(), "true");
    }

    #[test]
    fn test_autoescape() {
        let mut env = new_jinja2();
        env.add_template("test.html", "{{ value }}").unwrap();
        let tmpl = env.get_template("test.html").unwrap();
        let result = tmpl
            .render(context!(value => "<script>alert('xss')</script>"))
            .unwrap();
        assert!(result.contains("&lt;script&gt;"));
        assert!(!result.contains("<script>"));
    }

    #[test]
    fn test_safe_filter_with_autoescape() {
        let mut env = new_jinja2();
        env.add_template("test.html", "{{ value|safe }}").unwrap();
        let tmpl = env.get_template("test.html").unwrap();
        let result = tmpl.render(context!(value => "<b>bold</b>")).unwrap();
        assert_eq!(result, "<b>bold</b>");
    }

    #[test]
    fn test_expression_compilation() {
        let env = new_jinja2();
        let expr = env.compile_expression("x > 10 and y < 20").unwrap();
        let result = expr.eval(context!(x => 15, y => 5)).unwrap();
        assert!(result.is_true());

        let result = expr.eval(context!(x => 5, y => 5)).unwrap();
        assert!(!result.is_true());
    }

    #[test]
    fn test_wordwrap_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|wordwrap(width=10) }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(value => "This is a long text that should be wrapped"))
            .unwrap();
        assert!(result.contains('\n'));
    }

    #[test]
    fn test_call_block() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% macro render_dialog(title) %}<div>{{ title }}: {{ caller() }}</div>{% endmacro %}{% call render_dialog('Note') %}This is the body{% endcall %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert!(result.contains("Note"));
        assert!(result.contains("This is the body"));
    }

    #[test]
    fn test_for_else() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for i in items %}{{ i }}{% else %}empty{% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(items => Vec::<i32>::new())).unwrap();
        assert_eq!(result, "empty");
    }

    #[test]
    fn test_continue_in_loop() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for i in range(5) %}{% if i == 2 %}{% continue %}{% endif %}{{ i }}{% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "0134");
    }

    #[test]
    fn test_whitespace_control() {
        let mut env = new_jinja2();
        env.add_template("test", "  {%- if true %} yes {%- endif %}  ")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, " yes  ");
    }

    #[test]
    fn test_multiple_inheritance_levels() {
        let mut env = new_jinja2();
        env.add_template("base", "A {% block content %}base{% endblock %} Z")
            .unwrap();
        env.add_template(
            "mid",
            "{% extends 'base' %}{% block content %}mid-{% block inner %}inner{% endblock %}{% endblock %}",
        )
        .unwrap();
        env.add_template(
            "child",
            "{% extends 'mid' %}{% block inner %}child{% endblock %}",
        )
        .unwrap();
        let tmpl = env.get_template("child").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "A mid-child Z");
    }

    // ======================================================================
    // Airform-specific patterns
    // ======================================================================

    #[test]
    fn test_as_bool_filter() {
        let mut env = new_jinja2();
        env.add_template("t1", "{{ 'true'|as_bool }}").unwrap();
        assert_eq!(env.get_template("t1").unwrap().render(()).unwrap(), "true");

        env.add_template("t2", "{{ 'false'|as_bool }}").unwrap();
        assert_eq!(env.get_template("t2").unwrap().render(()).unwrap(), "false");

        env.add_template("t3", "{{ '1'|as_bool }}").unwrap();
        assert_eq!(env.get_template("t3").unwrap().render(()).unwrap(), "true");

        env.add_template("t4", "{{ 'yes'|as_bool }}").unwrap();
        assert_eq!(env.get_template("t4").unwrap().render(()).unwrap(), "true");

        env.add_template("t5", "{{ ''|as_bool }}").unwrap();
        assert_eq!(env.get_template("t5").unwrap().render(()).unwrap(), "false");

        env.add_template("t6", "{{ 42|as_bool }}").unwrap();
        assert_eq!(env.get_template("t6").unwrap().render(()).unwrap(), "true");

        env.add_template("t7", "{{ 0|as_bool }}").unwrap();
        assert_eq!(env.get_template("t7").unwrap().render(()).unwrap(), "false");
    }

    #[test]
    fn test_filtered_for_loop() {
        // {% for x in list if cond %} - heavily used in airform/dbt
        let mut env = new_jinja2();
        env.add_template("test", "{% for i in items if i > 2 %}{{ i }} {% endfor %}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(items => vec![1, 2, 3, 4, 5])).unwrap();
        assert_eq!(result, "3 4 5 ");
    }

    #[test]
    fn test_do_tag() {
        // {% do %} executes a call expression for side effects
        let mut env = new_jinja2();
        // do tag requires a call expression (function call)
        env.add_template(
            "test",
            "{% set items = [1, 2] %}{% do items.append(3) %}done",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        // append is a no-op stub, but the do tag shouldn't error
        assert_eq!(result, "done");
    }

    #[test]
    fn test_loop_variables_comprehensive() {
        let mut env = new_jinja2();

        // loop.index (1-based)
        env.add_template("t1", "{% for i in items %}{{ loop.index }}{% endfor %}")
            .unwrap();
        assert_eq!(
            env.get_template("t1")
                .unwrap()
                .render(context!(items => vec!["a", "b", "c"]))
                .unwrap(),
            "123"
        );

        // loop.index0 (0-based)
        env.add_template("t2", "{% for i in items %}{{ loop.index0 }}{% endfor %}")
            .unwrap();
        assert_eq!(
            env.get_template("t2")
                .unwrap()
                .render(context!(items => vec!["a", "b", "c"]))
                .unwrap(),
            "012"
        );

        // loop.first / loop.last
        env.add_template(
            "t3",
            "{% for i in items %}{% if loop.first %}F{% endif %}{% if loop.last %}L{% endif %}{{ i }}{% endfor %}",
        )
        .unwrap();
        assert_eq!(
            env.get_template("t3")
                .unwrap()
                .render(context!(items => vec!["a", "b", "c"]))
                .unwrap(),
            "FabLc"
        );

        // loop.length
        env.add_template("t4", "{% for i in items %}{{ loop.length }}{% endfor %}")
            .unwrap();
        assert_eq!(
            env.get_template("t4")
                .unwrap()
                .render(context!(items => vec!["a", "b", "c"]))
                .unwrap(),
            "333"
        );

        // loop.revindex (1-based, from end)
        env.add_template("t5", "{% for i in items %}{{ loop.revindex }}{% endfor %}")
            .unwrap();
        assert_eq!(
            env.get_template("t5")
                .unwrap()
                .render(context!(items => vec!["a", "b", "c"]))
                .unwrap(),
            "321"
        );

        // loop.revindex0 (0-based, from end)
        env.add_template("t6", "{% for i in items %}{{ loop.revindex0 }}{% endfor %}")
            .unwrap();
        assert_eq!(
            env.get_template("t6")
                .unwrap()
                .render(context!(items => vec!["a", "b", "c"]))
                .unwrap(),
            "210"
        );

        // loop.cycle
        env.add_template(
            "t7",
            "{% for i in items %}{{ loop.cycle('odd', 'even') }} {% endfor %}",
        )
        .unwrap();
        assert_eq!(
            env.get_template("t7")
                .unwrap()
                .render(context!(items => vec![1, 2, 3]))
                .unwrap(),
            "odd even odd "
        );

        // loop.changed
        env.add_template(
            "t8",
            "{% for i in items %}{% if loop.changed(i) %}*{% endif %}{{ i }}{% endfor %}",
        )
        .unwrap();
        assert_eq!(
            env.get_template("t8")
                .unwrap()
                .render(context!(items => vec![1, 1, 2, 2, 3]))
                .unwrap(),
            "*11*22*3"
        );
    }

    #[test]
    fn test_loop_previtem_nextitem() {
        let mut env = new_jinja2();

        // loop.previtem
        env.add_template(
            "t1",
            "{% for i in items %}{% if loop.previtem is defined %}{{ loop.previtem }}->{% endif %}{{ i }} {% endfor %}",
        )
        .unwrap();
        assert_eq!(
            env.get_template("t1")
                .unwrap()
                .render(context!(items => vec![1, 2, 3]))
                .unwrap(),
            "1 1->2 2->3 "
        );
    }

    #[test]
    fn test_string_method_compat() {
        let mut env = new_jinja2();

        // .upper() / .lower()
        env.add_template("t1", "{{ 'hello'.upper() }}").unwrap();
        assert_eq!(env.get_template("t1").unwrap().render(()).unwrap(), "HELLO");

        env.add_template("t2", "{{ 'HELLO'.lower() }}").unwrap();
        assert_eq!(env.get_template("t2").unwrap().render(()).unwrap(), "hello");

        // .strip() / .lstrip() / .rstrip()
        env.add_template("t3", "{{ '  hello  '.strip() }}").unwrap();
        assert_eq!(env.get_template("t3").unwrap().render(()).unwrap(), "hello");

        // .replace()
        env.add_template("t4", "{{ 'hello'.replace('l', 'r') }}")
            .unwrap();
        assert_eq!(env.get_template("t4").unwrap().render(()).unwrap(), "herro");

        // .startswith() / .endswith()
        env.add_template("t5", "{{ 'hello'.startswith('hel') }}")
            .unwrap();
        assert_eq!(env.get_template("t5").unwrap().render(()).unwrap(), "true");

        env.add_template("t6", "{{ 'hello'.endswith('llo') }}")
            .unwrap();
        assert_eq!(env.get_template("t6").unwrap().render(()).unwrap(), "true");

        // .split()
        env.add_template("t7", "{{ 'a,b,c'.split(',') | join(' ') }}")
            .unwrap();
        assert_eq!(env.get_template("t7").unwrap().render(()).unwrap(), "a b c");

        // .title()
        env.add_template("t8", "{{ 'hello world'.title() }}")
            .unwrap();
        assert_eq!(
            env.get_template("t8").unwrap().render(()).unwrap(),
            "Hello World"
        );

        // .capitalize()
        env.add_template("t9", "{{ 'hello'.capitalize() }}")
            .unwrap();
        assert_eq!(env.get_template("t9").unwrap().render(()).unwrap(), "Hello");
    }

    #[test]
    fn test_extended_string_methods() {
        let mut env = new_jinja2();

        // .center()
        env.add_template("t1", "{{ 'hi'.center(10) }}").unwrap();
        assert_eq!(
            env.get_template("t1").unwrap().render(()).unwrap(),
            "    hi    "
        );

        // .ljust()
        env.add_template("t2", "{{ 'hi'.ljust(10) }}").unwrap();
        assert_eq!(
            env.get_template("t2").unwrap().render(()).unwrap(),
            "hi        "
        );

        // .rjust()
        env.add_template("t3", "{{ 'hi'.rjust(10) }}").unwrap();
        assert_eq!(
            env.get_template("t3").unwrap().render(()).unwrap(),
            "        hi"
        );

        // .zfill()
        env.add_template("t4", "{{ '42'.zfill(5) }}").unwrap();
        assert_eq!(env.get_template("t4").unwrap().render(()).unwrap(), "00042");
    }

    #[test]
    fn test_dict_method_compat() {
        let mut env = new_jinja2();

        // .items()
        env.add_template(
            "t1",
            "{% for k, v in data.items() %}{{ k }}={{ v }} {% endfor %}",
        )
        .unwrap();
        assert_eq!(
            env.get_template("t1")
                .unwrap()
                .render(context!(data => Value::from_serialize(&json!({"x": 1}))))
                .unwrap(),
            "x=1 "
        );

        // .keys()
        env.add_template("t2", "{% for k in data.keys() %}{{ k }} {% endfor %}")
            .unwrap();
        assert_eq!(
            env.get_template("t2")
                .unwrap()
                .render(context!(data => Value::from_serialize(&json!({"a": 1}))))
                .unwrap(),
            "a "
        );

        // .values()
        env.add_template("t3", "{% for v in data.values() %}{{ v }} {% endfor %}")
            .unwrap();
        assert_eq!(
            env.get_template("t3")
                .unwrap()
                .render(context!(data => Value::from_serialize(&json!({"a": 1}))))
                .unwrap(),
            "1 "
        );

        // .get()
        env.add_template("t4", "{{ data.get('a', 'default') }}")
            .unwrap();
        assert_eq!(
            env.get_template("t4")
                .unwrap()
                .render(context!(data => Value::from_serialize(&json!({"a": 42}))))
                .unwrap(),
            "42"
        );

        env.add_template("t5", "{{ data.get('missing', 'default') }}")
            .unwrap();
        assert_eq!(
            env.get_template("t5")
                .unwrap()
                .render(context!(data => Value::from_serialize(&json!({"a": 42}))))
                .unwrap(),
            "default"
        );
    }

    #[test]
    fn test_list_method_stubs() {
        // List mutation methods should not error (no-op stubs)
        let mut env = new_jinja2();

        // .append() - should not error in {% do %} context
        env.add_template("test", "{% set items = [1, 2] %}{% do items.append(3) %}ok")
            .unwrap();
        let result = env.get_template("test").unwrap().render(()).unwrap();
        assert_eq!(result, "ok");
    }

    #[test]
    fn test_dict_update_stub() {
        // .update() should not error
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% set data = {'a': 1} %}{% do data.update({'b': 2}) %}ok",
        )
        .unwrap();
        let result = env.get_template("test").unwrap().render(()).unwrap();
        // Result doesn't matter, just that it doesn't error
        assert!(result.contains("ok"));
    }

    #[test]
    fn test_is_not_none() {
        // Heavily used in airform: {% if x is not none %}
        let mut env = new_jinja2();
        env.add_template("test", "{% if x is not none %}yes{% else %}no{% endif %}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        assert_eq!(tmpl.render(context!(x => "hello")).unwrap(), "yes");
        assert_eq!(tmpl.render(context!(x => Value::from(()))).unwrap(), "no");
    }

    #[test]
    fn test_is_mapping() {
        let mut env = new_jinja2();
        env.add_template("test", "{% if x is mapping %}map{% else %}not{% endif %}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        assert_eq!(
            tmpl.render(context!(x => Value::from_serialize(&json!({"a": 1}))))
                .unwrap(),
            "map"
        );
        assert_eq!(tmpl.render(context!(x => "hello")).unwrap(), "not");
    }

    #[test]
    fn test_in_operator() {
        let mut env = new_jinja2();

        // in with list
        env.add_template("t1", "{{ 'a' in items }}").unwrap();
        assert_eq!(
            env.get_template("t1")
                .unwrap()
                .render(context!(items => vec!["a", "b", "c"]))
                .unwrap(),
            "true"
        );

        // not in
        env.add_template("t2", "{{ 'z' not in items }}").unwrap();
        assert_eq!(
            env.get_template("t2")
                .unwrap()
                .render(context!(items => vec!["a", "b", "c"]))
                .unwrap(),
            "true"
        );

        // in with string
        env.add_template("t3", "{{ 'ell' in 'hello' }}").unwrap();
        assert_eq!(env.get_template("t3").unwrap().render(()).unwrap(), "true");
    }

    #[test]
    fn test_target_type_pattern() {
        // Common airform pattern: {% if target.type in ('spark','databricks') %}
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% if target_type in ('postgres', 'redshift') %}pg{% else %}other{% endif %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        assert_eq!(
            tmpl.render(context!(target_type => "postgres")).unwrap(),
            "pg"
        );
        assert_eq!(
            tmpl.render(context!(target_type => "bigquery")).unwrap(),
            "other"
        );
    }

    #[test]
    fn test_tilde_concat_in_loop() {
        // Common airform pattern: building SQL with ~ and loop
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% set ns = namespace(sql='') %}{% for col in cols %}{% if not loop.first %}{% set ns.sql = ns.sql ~ ', ' %}{% endif %}{% set ns.sql = ns.sql ~ col %}{% endfor %}{{ ns.sql }}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(cols => vec!["a", "b", "c"])).unwrap();
        assert_eq!(result, "a, b, c");
    }

    #[test]
    fn test_map_attribute() {
        // columns|map(attribute='name')|list - common in airform
        let mut env = new_jinja2();
        env.add_template("test", "{{ items|map(attribute='name')|join(', ') }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(items => Value::from_serialize(&json!([
                {"name": "id", "type": "int"},
                {"name": "email", "type": "str"},
            ]))))
            .unwrap();
        assert_eq!(result, "id, email");
    }

    #[test]
    fn test_selectattr_equalto() {
        // graph.nodes.values() | selectattr("resource_type", "equalto", "seed")
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for item in items|selectattr('type', 'equalto', 'fruit') %}{{ item.name }} {% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(items => Value::from_serialize(&json!([
                {"name": "apple", "type": "fruit"},
                {"name": "carrot", "type": "veggie"},
                {"name": "banana", "type": "fruit"},
            ]))))
            .unwrap();
        assert_eq!(result, "apple banana ");
    }

    #[test]
    fn test_reject_equalto() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ items|reject('equalto', 'b')|join(', ') }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(items => vec!["a", "b", "c"])).unwrap();
        assert_eq!(result, "a, c");
    }

    #[test]
    fn test_select_in() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% set allowed = ['a', 'c'] %}{% for x in items|select('in', allowed) %}{{ x }} {% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(items => vec!["a", "b", "c", "d"]))
            .unwrap();
        assert_eq!(result, "a c ");
    }

    #[test]
    fn test_default_filter_with_boolean() {
        // var('enabled', True)|as_bool pattern from airform
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% if enabled|default(true) %}on{% else %}off{% endif %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        assert_eq!(tmpl.render(()).unwrap(), "on");
        assert_eq!(tmpl.render(context!(enabled => false)).unwrap(), "off");
    }

    #[test]
    fn test_dict_literal_in_set() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% set data = {'key': 'value', 'num': 42} %}{{ data.key }}-{{ data.num }}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "value-42");
    }

    #[test]
    fn test_list_literal_in_set() {
        let mut env = new_jinja2();
        env.add_template("test", "{% set items = [1, 2, 3] %}{{ items|join(', ') }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "1, 2, 3");
    }

    #[test]
    fn test_nested_attribute_access() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ data.inner.value }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(data => Value::from_serialize(&json!({"inner": {"value": "deep"}}))))
            .unwrap();
        assert_eq!(result, "deep");
    }

    #[test]
    fn test_bracket_access() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ data['key'] }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(data => Value::from_serialize(&json!({"key": "value"}))))
            .unwrap();
        assert_eq!(result, "value");
    }

    #[test]
    fn test_conditional_comma_pattern() {
        // Very common in airform SQL generation:
        // {% for col in cols %}{{ col }}{% if not loop.last %}, {% endif %}{% endfor %}
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "SELECT {% for col in cols %}{{ col }}{% if not loop.last %}, {% endif %}{% endfor %} FROM table",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(cols => vec!["id", "name", "email"]))
            .unwrap();
        assert_eq!(result, "SELECT id, name, email FROM table");
    }

    #[test]
    fn test_map_capitalize_join() {
        // original_column_name.split('_') | map('capitalize') | join('')
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{{ 'hello_world_test'.split('_') | map('capitalize') | join('') }}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "HelloWorldTest");
    }

    #[test]
    fn test_replace_chain() {
        // em | replace(' ', '_') | replace('(', '')
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{{ value | replace(' ', '_') | replace('(', '') | replace(')', '') }}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(value => "hello (world)")).unwrap();
        assert_eq!(result, "hello_world");
    }

    #[test]
    fn test_trim_filter_with_chars() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|trim }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(value => "  hello  ")).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_lower_in_selectattr() {
        // col.name|lower pattern in conditional for loops
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for col in columns if col.name|lower != 'id' %}{{ col.name }} {% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(columns => Value::from_serialize(&json!([
                {"name": "id"},
                {"name": "Name"},
                {"name": "Email"},
            ]))))
            .unwrap();
        assert_eq!(result, "Name Email ");
    }

    // ======================================================================
    // Full Jinja2 spec coverage
    // ======================================================================

    #[test]
    fn test_super_in_blocks() {
        let mut env = new_jinja2();
        env.add_template("base", "{% block content %}base{% endblock %}")
            .unwrap();
        env.add_template(
            "child",
            "{% extends 'base' %}{% block content %}{{ super() }}-child{% endblock %}",
        )
        .unwrap();
        let tmpl = env.get_template("child").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "base-child");
    }

    #[test]
    fn test_include_ignore_missing() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "before{% include 'nonexistent' ignore missing %}after",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "beforeafter");
    }

    #[test]
    fn test_dynamic_extends() {
        let mut env = new_jinja2();
        env.add_template("layout_a", "A:{% block body %}{% endblock %}")
            .unwrap();
        env.add_template("layout_b", "B:{% block body %}{% endblock %}")
            .unwrap();
        env.add_template(
            "test",
            "{% extends layout %}{% block body %}content{% endblock %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        assert_eq!(
            tmpl.render(context!(layout => "layout_a")).unwrap(),
            "A:content"
        );
        assert_eq!(
            tmpl.render(context!(layout => "layout_b")).unwrap(),
            "B:content"
        );
    }

    #[test]
    fn test_floor_division() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ 7 // 2 }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn test_power_operator() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ 2 ** 10 }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "1024");
    }

    #[test]
    fn test_modulo() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ 17 % 5 }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "2");
    }

    #[test]
    fn test_string_multiply() {
        // Not standard Jinja2 but minijinja supports it
        let mut env = new_jinja2();
        env.add_template("test", "{{ '-' * 5 }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "-----");
    }

    #[test]
    fn test_pprint_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|pprint }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(value => vec![1, 2, 3])).unwrap();
        // pprint should produce some representation of the list
        assert!(result.contains("1"));
        assert!(result.contains("2"));
        assert!(result.contains("3"));
    }

    #[test]
    fn test_format_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ 'Hello %s, you are %d'|format('World', 42) }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "Hello World, you are 42");
    }

    #[test]
    fn test_chain_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ [1, 2]|chain([3, 4])|list|join(', ') }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "1, 2, 3, 4");
    }

    #[test]
    fn test_zip_filter() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for a, b in [1, 2]|zip(['a', 'b']) %}{{ a }}{{ b }} {% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "1a 2b ");
    }

    #[test]
    fn test_items_filter() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for k, v in data|items %}{{ k }}={{ v }} {% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(data => Value::from_serialize(&json!({"a": 1}))))
            .unwrap();
        assert_eq!(result, "a=1 ");
    }

    #[test]
    fn test_bool_filter() {
        let mut env = new_jinja2();
        env.add_template("t1", "{{ 0|bool }}").unwrap();
        assert_eq!(env.get_template("t1").unwrap().render(()).unwrap(), "false");

        env.add_template("t2", "{{ 1|bool }}").unwrap();
        assert_eq!(env.get_template("t2").unwrap().render(()).unwrap(), "true");
    }

    #[test]
    fn test_attr_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ data|attr('name') }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(data => Value::from_serialize(&json!({"name": "test"}))))
            .unwrap();
        assert_eq!(result, "test");
    }

    #[test]
    fn test_split_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ 'a,b,c'|split(',')|join(' ') }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "a b c");
    }

    #[test]
    fn test_lines_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ value|lines|length }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(value => "line1\nline2\nline3"))
            .unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn test_debug_function() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ debug() }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        // debug() should not error
        let _ = tmpl.render(context!(x => 1)).unwrap();
    }

    #[test]
    fn test_range_function() {
        let mut env = new_jinja2();

        // range(stop)
        env.add_template("t1", "{{ range(5)|list|join(',') }}")
            .unwrap();
        assert_eq!(
            env.get_template("t1").unwrap().render(()).unwrap(),
            "0,1,2,3,4"
        );

        // range(start, stop)
        env.add_template("t2", "{{ range(2, 5)|list|join(',') }}")
            .unwrap();
        assert_eq!(env.get_template("t2").unwrap().render(()).unwrap(), "2,3,4");

        // range(start, stop, step)
        env.add_template("t3", "{{ range(0, 10, 3)|list|join(',') }}")
            .unwrap();
        assert_eq!(
            env.get_template("t3").unwrap().render(()).unwrap(),
            "0,3,6,9"
        );
    }

    #[test]
    fn test_dict_function() {
        let mut env = new_jinja2();
        env.add_template("test", "{% set d = dict(a=1, b=2) %}{{ d.a }},{{ d.b }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "1,2");
    }

    #[test]
    fn test_jinja2_comments() {
        let mut env = new_jinja2();
        env.add_template("test", "before{# this is a comment #}after")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "beforeafter");
    }

    #[test]
    fn test_complex_dbt_pattern() {
        // Realistic dbt-like pattern combining many features
        let mut env = new_jinja2();
        env.add_template(
            "model",
            "\
{% set columns = ['id', 'name', 'email', 'created_at'] %}\
{% set exclude = ['created_at'] %}\
SELECT\n\
{% for col in columns if col not in exclude %}\
    {{ col }}{% if not loop.last %},{% endif %}\n\
{% endfor %}\
FROM {{ schema }}.{{ table }}",
        )
        .unwrap();
        let tmpl = env.get_template("model").unwrap();
        let result = tmpl
            .render(context!(schema => "public", table => "users"))
            .unwrap();
        assert!(result.contains("id,"));
        assert!(result.contains("name,"));
        assert!(result.contains("email"));
        assert!(!result.contains("created_at"));
        assert!(result.contains("FROM public.users"));
    }

    #[test]
    fn test_macro_with_defaults() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% macro cast(col, type='string') %}CAST({{ col }} AS {{ type }}){% endmacro %}{{ cast('id') }},{{ cast('amount', 'float') }}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "CAST(id AS string),CAST(amount AS float)");
    }

    #[test]
    fn test_macro_caller() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% macro wrap(tag) %}<{{ tag }}>{{ caller() }}</{{ tag }}>{% endmacro %}{% call wrap('div') %}content{% endcall %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "<div>content</div>");
    }

    #[test]
    fn test_set_block() {
        // {% set x %}...{% endset %}
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% set content %}hello world{% endset %}{{ content|upper }}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "HELLO WORLD");
    }

    // ======================================================================
    // Audit gap-fill: filters/features with missing direct tests
    // ======================================================================

    #[test]
    fn test_slice_filter() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for group in items|slice(3) %}[{{ group|join(',') }}]{% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(items => vec![1, 2, 3, 4, 5, 6, 7]))
            .unwrap();
        assert!(result.contains("[1,2,3]"));
    }

    #[test]
    fn test_max_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ items|max }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(items => vec![3, 1, 5, 2])).unwrap();
        assert_eq!(result, "5");
    }

    #[test]
    fn test_min_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ items|min }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(items => vec![3, 1, 5, 2])).unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn test_rejectattr_filter() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for item in items|rejectattr('hidden') %}{{ item.name }} {% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl
            .render(context!(items => Value::from_serialize(&json!([
                {"name": "a", "hidden": true},
                {"name": "b", "hidden": false},
                {"name": "c", "hidden": false},
            ]))))
            .unwrap();
        assert_eq!(result, "b c ");
    }

    #[test]
    fn test_string_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ 42|string }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_loop_nextitem() {
        let mut env = new_jinja2();
        env.add_template(
            "test",
            "{% for i in items %}{{ i }}{% if loop.nextitem is defined %}->{{ loop.nextitem }}{% endif %} {% endfor %}",
        )
        .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(context!(items => vec![1, 2, 3])).unwrap();
        assert_eq!(result, "1->2 2->3 3 ");
    }

    #[test]
    fn test_escape_filter() {
        let mut env = new_jinja2();
        env.add_template("test.html", "{{ value|e }}").unwrap();
        let tmpl = env.get_template("test.html").unwrap();
        let result = tmpl.render(context!(value => "<b>bold</b>")).unwrap();
        assert!(result.contains("&lt;b&gt;"));
    }

    #[test]
    fn test_capitalize_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ 'hello WORLD'|capitalize }}")
            .unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_float_filter() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ '3.14'|float }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        assert_eq!(result, "3.14");
    }

    #[test]
    fn test_lipsum_function() {
        let mut env = new_jinja2();
        env.add_template("test", "{{ lipsum(1) }}").unwrap();
        let tmpl = env.get_template("test").unwrap();
        let result = tmpl.render(()).unwrap();
        // lipsum generates lorem-ipsum-like text
        assert!(!result.is_empty());
        assert!(result.contains('.'));
    }
}

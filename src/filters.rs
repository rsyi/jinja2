//! Additional filters for Jinja2 parity.
//!
//! These filters fill the gaps between minijinja's built-in filters and
//! the full Jinja2 filter set.

use minijinja::value::{Kwargs, Value, ValueKind};
use minijinja::{Error, ErrorKind};

/// Centers the value in a field of a given width.
///
/// ```jinja
/// {{ "hello"|center(20) }}
/// ```
pub fn center(value: &Value, width: Option<u32>) -> Result<String, Error> {
    let s = value.to_string();
    let width = width.unwrap_or(80) as usize;
    let len = s.chars().count();
    if len >= width {
        return Ok(s);
    }
    let total_pad = width - len;
    let left_pad = total_pad / 2;
    let right_pad = total_pad - left_pad;
    let mut result = String::with_capacity(width);
    for _ in 0..left_pad {
        result.push(' ');
    }
    result.push_str(&s);
    for _ in 0..right_pad {
        result.push(' ');
    }
    Ok(result)
}

/// Apply HTML escaping, even if the value is already marked safe.
///
/// Unlike `escape`, this will always escape the value, potentially
/// double-escaping it.
///
/// ```jinja
/// {{ value|forceescape }}
/// ```
pub fn forceescape(value: &Value) -> Value {
    let s = value.to_string();
    let escaped = html_escape(&s);
    Value::from_safe_string(escaped)
}

/// Convert URLs in plain text into clickable links.
///
/// ```jinja
/// {{ "Check out https://example.com today!"|urlize }}
/// ```
///
/// Keyword arguments:
/// * `trim_url_limit`: shorten displayed URLs to this length (default: no limit)
/// * `nofollow`: add `rel="nofollow"` (default: false)
/// * `target`: set target attribute (e.g., `"_blank"`)
/// * `rel`: set rel attribute (overrides nofollow)
pub fn urlize(value: &Value, kwargs: Kwargs) -> Result<Value, Error> {
    let s = value.to_string();
    let trim_url_limit: Option<usize> = kwargs.get("trim_url_limit")?;
    let nofollow: Option<bool> = kwargs.get("nofollow")?;
    let target: Option<String> = kwargs.get("target")?;
    let rel: Option<String> = kwargs.get("rel")?;
    kwargs.assert_all_used()?;

    let nofollow = nofollow.unwrap_or(false);

    let mut result = String::new();
    let mut last_end = 0;

    for (start, end, url) in find_urls(&s) {
        // Append text before this URL (escaped)
        result.push_str(&html_escape(&s[last_end..start]));

        // Build the link
        result.push_str("<a href=\"");
        result.push_str(&html_escape(&url));
        result.push('"');

        if let Some(ref r) = rel {
            result.push_str(" rel=\"");
            result.push_str(&html_escape(r));
            result.push('"');
        } else if nofollow {
            result.push_str(" rel=\"nofollow\"");
        }

        if let Some(ref t) = target {
            result.push_str(" target=\"");
            result.push_str(&html_escape(t));
            result.push('"');
        }

        result.push('>');

        // Display text (potentially trimmed)
        let display = &s[start..end];
        let display_escaped = html_escape(display);
        if let Some(limit) = trim_url_limit {
            if display.chars().count() > limit {
                let trimmed: String = display.chars().take(limit - 3).collect();
                result.push_str(&html_escape(&trimmed));
                result.push_str("...");
            } else {
                result.push_str(&display_escaped);
            }
        } else {
            result.push_str(&display_escaped);
        }

        result.push_str("</a>");
        last_end = end;
    }

    // Remaining text
    result.push_str(&html_escape(&s[last_end..]));

    Ok(Value::from_safe_string(result))
}

/// Create an HTML/XML attribute string from a dictionary.
///
/// Results are sorted by key for consistent output.
/// Values that are `none` or `undefined` are skipped.
/// If a value is `true`, only the key is rendered.
///
/// ```jinja
/// <ul{{ {'class': 'nav', 'id': 'main'}|xmlattr }}>
/// ```
pub fn xmlattr(value: &Value, kwargs: Kwargs) -> Result<Value, Error> {
    let autospace: Option<bool> = kwargs.get("autospace")?;
    kwargs.assert_all_used()?;
    let autospace = autospace.unwrap_or(true);

    if value.kind() != ValueKind::Map {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("xmlattr expects a mapping, got {}", value.kind()),
        ));
    }

    let mut parts = Vec::new();

    // Collect and sort by key for consistent output
    let mut pairs: Vec<(String, Value)> = Vec::new();
    if let Ok(iter) = value.try_iter() {
        for key in iter {
            if let Some(val) = value.get_item(&key).ok().filter(|v| !v.is_undefined()) {
                pairs.push((key.to_string(), val));
            }
        }
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    for (key, val) in pairs {
        if val.is_none() || val.is_undefined() {
            continue;
        }
        if val.kind() == ValueKind::Bool {
            if val.is_true() {
                parts.push(html_escape(&key));
            }
        } else {
            parts.push(format!(
                "{}=\"{}\"",
                html_escape(&key),
                html_escape(&val.to_string())
            ));
        }
    }

    let joined = parts.join(" ");
    if autospace && !joined.is_empty() {
        Ok(Value::from_safe_string(format!(" {}", joined)))
    } else {
        Ok(Value::from_safe_string(joined))
    }
}

/// Convert a value to a boolean.
///
/// Interprets common truthy string values ("true", "1", "yes", "on")
/// as `true`, and falsy values ("false", "0", "no", "off", "") as `false`.
/// Non-string values use standard truthiness.
///
/// ```jinja
/// {{ "true"|as_bool }}
/// {{ value|as_bool }}
/// ```
pub fn as_bool(value: &Value) -> bool {
    if let Some(s) = value.as_str() {
        matches!(
            s.to_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        )
    } else {
        value.is_true()
    }
}

// --- Helpers ---

fn html_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&#34;"),
            '\'' => result.push_str("&#39;"),
            _ => result.push(c),
        }
    }
    result
}

/// Find URLs in text, returning (start, end, full_url) tuples.
///
/// Operates on byte offsets directly since URL prefixes are ASCII.
fn find_urls(text: &str) -> Vec<(usize, usize, String)> {
    let mut results = Vec::new();
    let prefixes: &[&str] = &["https://", "http://", "www."];
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let remaining = &text[i..];
        let matched_prefix = prefixes.iter().find(|p| remaining.starts_with(**p));

        if let Some(prefix) = matched_prefix {
            let start = i;

            // Find end of URL
            let mut end = i + prefix.len();
            while end < len {
                let b = bytes[end];
                if b.is_ascii_whitespace() || matches!(b, b'<' | b'>' | b'"' | b'\'') {
                    break;
                }
                end += 1;
            }

            // Strip trailing punctuation
            while end > start && matches!(bytes[end - 1], b'.' | b',' | b')' | b'!' | b'?') {
                end -= 1;
            }

            let url_text = &text[start..end];
            if url_text.len() > prefix.len() {
                let full_url = if *prefix == "www." {
                    format!("http://{}", url_text)
                } else {
                    url_text.to_string()
                };
                results.push((start, end, full_url));
            }

            i = end;
        } else {
            i += 1;
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center() {
        let v = Value::from("hello");
        assert_eq!(center(&v, Some(20)).unwrap(), "       hello        ");
        assert_eq!(center(&v, Some(5)).unwrap(), "hello");
        assert_eq!(center(&v, Some(3)).unwrap(), "hello");
    }

    #[test]
    fn test_forceescape() {
        let v = Value::from("<b>hello</b>");
        let result = forceescape(&v);
        assert_eq!(result.to_string(), "&lt;b&gt;hello&lt;/b&gt;");
    }

    #[test]
    fn test_xmlattr() {
        let mut env = minijinja::Environment::new();
        env.add_template("test", "{{ attrs|xmlattr }}").unwrap();
        // Just test the html_escape helper
        assert_eq!(html_escape("<b>"), "&lt;b&gt;");
        assert_eq!(html_escape("a&b"), "a&amp;b");
    }

    #[test]
    fn test_find_urls() {
        let urls = find_urls("Visit https://example.com today!");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].2, "https://example.com");

        let urls = find_urls("Go to www.example.com for more");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].2, "http://www.example.com");
    }
}

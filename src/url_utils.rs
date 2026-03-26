use url::Url;

/// Splits a query string into tokens by whitespace.
pub fn tokenize(s: &str) -> Vec<&str> {
    s.split_whitespace().filter(|t| !t.is_empty()).collect()
}

/// Checks if `s` starts with `prefix`, or starts with `prefix` after trimming
/// leading non-alphanumeric characters.
pub fn leading_boundary_starts_with(s: &str, prefix: &str) -> bool {
    if s.starts_with(prefix) {
        return true;
    }
    let trimmed = s.trim_start_matches(|c: char| !c.is_alphanumeric());
    trimmed.starts_with(prefix)
}

/// Returns URL string without scheme, www. prefix, and trailing '/'.
/// e.g. "https://www.example.com/path/" → "example.com/path"
pub fn naked_string(url: &Url) -> String {
    let host = url.host_str().unwrap_or("");
    let host_no_www = if host.starts_with("www.") {
        &host[4..]
    } else {
        host
    };

    let path = url.path();
    let path_trimmed = if path == "/" { "" } else { path.trim_end_matches('/') };

    let mut result = String::with_capacity(host_no_www.len() + path_trimmed.len() + 32);
    result.push_str(host_no_www);
    result.push_str(path_trimmed);

    if let Some(q) = url.query() {
        result.push('?');
        result.push_str(q);
    }
    if let Some(f) = url.fragment() {
        result.push('#');
        result.push_str(f);
    }

    result
}

/// Returns true if the URL is a root URL (no meaningful path, query, fragment).
pub fn is_root(url: &Url) -> bool {
    let path = url.path();
    (path.is_empty() || path == "/")
        && url.query().is_none()
        && url.fragment().is_none()
        && url.username().is_empty()
        && url.password().is_none()
}

/// Returns the host without "www." prefix.
pub fn host_without_www(url: &Url) -> String {
    let host = url.host_str().unwrap_or("");
    if host.starts_with("www.") {
        host[4..].to_string()
    } else {
        host.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // tokenize tests
    #[test]
    fn test_tokenize_splits_by_whitespace() {
        assert_eq!(tokenize("testing query tokens"), vec!["testing", "query", "tokens"]);
    }

    #[test]
    fn test_tokenize_handles_tabs_and_newlines() {
        assert_eq!(tokenize("testing\tquery\ttokens"), vec!["testing", "query", "tokens"]);
        assert_eq!(tokenize("testing\nquery\ntokens"), vec!["testing", "query", "tokens"]);
    }

    #[test]
    fn test_tokenize_empty_and_whitespace_only() {
        assert_eq!(tokenize(""), Vec::<&str>::new());
        assert_eq!(tokenize("  \t\n\t\t \t \t  \n\n\n "), Vec::<&str>::new());
    }

    // leading_boundary_starts_with tests
    #[test]
    fn test_leading_boundary_exact_start() {
        assert!(leading_boundary_starts_with("cats and dogs", "cats"));
    }

    #[test]
    fn test_leading_boundary_with_quotes() {
        assert!(leading_boundary_starts_with("\"cats and dogs\"", "cats"));
        assert!(leading_boundary_starts_with("\"cats and dogs\"", "\"cats"));
        assert!(leading_boundary_starts_with("\"cats and dogs\"", "\""));
    }

    #[test]
    fn test_leading_boundary_unicode_quotes() {
        assert!(leading_boundary_starts_with(
            "«Рукописи не горят»: первый",
            "Рукописи"
        ));
        assert!(leading_boundary_starts_with(
            "«Рукописи не горят»: первый",
            "«"
        ));
    }

    #[test]
    fn test_leading_boundary_no_match() {
        assert!(!leading_boundary_starts_with("cats and dogs", "dogs"));
    }

    // naked_string tests
    #[test]
    fn test_naked_string_strips_scheme_and_www() {
        let url = Url::parse("https://www.example.com/path").unwrap();
        assert_eq!(naked_string(&url), "example.com/path");
    }

    #[test]
    fn test_naked_string_strips_trailing_slash() {
        let url = Url::parse("https://www.example.com/").unwrap();
        assert_eq!(naked_string(&url), "example.com");
    }

    #[test]
    fn test_naked_string_no_www() {
        let url = Url::parse("https://example.com/path").unwrap();
        assert_eq!(naked_string(&url), "example.com/path");
    }

    #[test]
    fn test_naked_string_preserves_query_and_fragment() {
        let url = Url::parse("https://example.com/path?q=1#frag").unwrap();
        assert_eq!(naked_string(&url), "example.com/path?q=1#frag");
    }

    // is_root tests
    #[test]
    fn test_is_root_true_for_root_url() {
        assert!(is_root(&Url::parse("https://example.com").unwrap()));
        assert!(is_root(&Url::parse("https://example.com/").unwrap()));
    }

    #[test]
    fn test_is_root_false_for_path() {
        assert!(!is_root(&Url::parse("https://example.com/path").unwrap()));
    }

    #[test]
    fn test_is_root_false_for_query() {
        assert!(!is_root(&Url::parse("https://example.com?q=1").unwrap()));
    }

    #[test]
    fn test_is_root_false_for_fragment() {
        assert!(!is_root(&Url::parse("https://example.com#frag").unwrap()));
    }

    // host_without_www tests
    #[test]
    fn test_host_without_www() {
        let url = Url::parse("https://www.example.com").unwrap();
        assert_eq!(host_without_www(&url), "example.com");
    }

    #[test]
    fn test_host_without_www_no_www() {
        let url = Url::parse("https://example.com").unwrap();
        assert_eq!(host_without_www(&url), "example.com");
    }
}

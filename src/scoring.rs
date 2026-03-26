use url::Url;
use crate::url_utils;

/// Scores a suggestion based on query match against title and URL.
/// Direct port of Swift ScoringService.score().
pub fn score(
    title: Option<&str>,
    url: &Url,
    visit_count: i64,
    lowercased_query: &str,
    query_tokens: &[&str],
) -> i64 {
    let lowercased_title = title.map(|t| t.to_lowercase()).unwrap_or_default();
    let query_count = lowercased_query.chars().count();
    let domain = url_utils::host_without_www(url);
    let naked = url_utils::naked_string(url);

    let mut s: i64 = 0;

    if naked.starts_with(lowercased_query) {
        s += 300;
        if url_utils::is_root(url) {
            s += 2000;
        }
    } else if url_utils::leading_boundary_starts_with(&lowercased_title, lowercased_query) {
        s += 200;
        if url_utils::is_root(url) {
            s += 2000;
        }
    } else if query_count > 2 && domain.contains(lowercased_query) {
        s += 150;
    } else if query_count > 2 && lowercased_title.contains(&format!(" {lowercased_query}")) {
        s += 100;
    } else if query_tokens.len() > 1 {
        let mut matches_all = true;
        for token in query_tokens {
            let in_title = url_utils::leading_boundary_starts_with(&lowercased_title, token)
                || lowercased_title.contains(&format!(" {token}"));
            let in_url = naked.starts_with(token);
            if !in_title && !in_url {
                matches_all = false;
                break;
            }
        }

        if matches_all {
            s += 10;
            if naked.starts_with(query_tokens[0]) {
                s += 70;
            } else if url_utils::leading_boundary_starts_with(&lowercased_title, query_tokens[0]) {
                s += 50;
            }
        }
    }

    if s > 0 {
        s <<= 10;
        s += visit_count;
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_score(title: &str, url: &str, visit_count: i64, query: &str) -> i64 {
        let lower = query.to_lowercase();
        let tokens = url_utils::tokenize(&lower);
        let parsed = Url::parse(url).unwrap();
        score(Some(title), &parsed, visit_count, &lower, &tokens)
    }

    #[test]
    fn test_url_match_scores_300() {
        let s = make_score("Test case website", "https://www.testcase.com/notroot", 100, "testcase.com/no");
        assert!(s > 0, "URL match should score > 0, got {s}");
    }

    #[test]
    fn test_title_match_from_beginning_scores_higher() {
        let s1 = make_score("Test case website", "https://www.website.com", 100, "test");
        let s2 = make_score("Case test website 2", "https://www.website2.com", 100, "test");
        assert!(s1 > s2, "Title start match ({s1}) should beat mid-title ({s2})");
    }

    #[test]
    fn test_domain_match_from_beginning_scores_higher() {
        let s1 = make_score("Website", "https://www.test.com", 100, "test");
        let s2 = make_score("Website 2", "https://www.websitetest.com", 100, "test");
        assert!(s1 > s2, "Domain start ({s1}) should beat domain-contains ({s2})");
    }

    #[test]
    fn test_more_visits_scores_higher() {
        let s1 = make_score("Website", "https://www.website.com", 100, "website");
        let s2 = make_score("Website 2", "https://www.website2.com", 101, "website");
        assert!(s1 < s2, "More visits ({s2}) should beat fewer ({s1})");
    }

    #[test]
    fn test_root_url_gets_bonus() {
        let s1 = make_score("Test", "https://www.test.com", 0, "test");
        let s2 = make_score("Test", "https://www.test.com/path", 0, "test");
        assert!(s1 > s2, "Root URL ({s1}) should beat non-root ({s2})");
    }

    #[test]
    fn test_no_match_scores_zero() {
        let s = make_score("Completely unrelated", "https://www.other.com", 0, "zzzzz");
        assert_eq!(s, 0);
    }

    #[test]
    fn test_domain_contains_query_longer_than_2() {
        let s = make_score("Other title", "https://www.website.com/path", 0, "web");
        assert!(s > 0, "Domain contains 'web' should score > 0, got {s}");
    }

    #[test]
    fn test_domain_contains_query_2_or_less_no_match() {
        let s = make_score("Other title", "https://www.notawebsite.com/path", 0, "we");
        assert_eq!(s, 0, "Short query should not match via domain-contains");
    }

    #[test]
    fn test_title_word_match() {
        let s = make_score("Big duck pond", "https://www.other.com/path", 0, "duck");
        assert!(s > 0, "Title word match should score > 0, got {s}");
    }

    #[test]
    fn test_tokenized_match_all_tokens() {
        let s = make_score("Cats and Dogs", "https://www.other.com", 0, "cats dogs");
        assert!(s > 0, "All-token match should score > 0, got {s}");
    }

    #[test]
    fn test_tokenized_match_first_token_url_boost() {
        let s1 = make_score("other test page", "https://www.test.com/page", 0, "test page");
        let s2 = make_score("test page other", "https://www.other.com/page", 0, "page test");
        assert!(s1 > s2, "URL-start first token ({s1}) should beat non-URL-start ({s2})");
    }

    #[test]
    fn test_non_alphanumeric_title_start() {
        let s = make_score("\"Cats and Dogs\"", "https://www.testcase.com/notroot", 0, "cats");
        assert!(s > 0, "Quoted title start should match, got {s}");
    }

    #[test]
    fn test_unicode_title_start() {
        let s = make_score(
            "«Рукописи не горят»: первый",
            "https://www.testcase.com/notroot",
            0,
            "рукописи",
        );
        assert!(s > 0, "Unicode title boundary start should match, got {s}");
    }

    #[test]
    fn test_visit_count_shift() {
        // Score 300 << 10 = 307200, + visitCount 100 = 307300
        let s = make_score("Test", "https://www.testcase.com/notroot", 100, "testcase.com/no");
        assert_eq!(s, 307300);
    }

    #[test]
    fn test_root_url_bonus_with_shift() {
        // Score (300 + 2000) << 10 = 2355200
        let s = make_score("Test", "https://www.test.com", 0, "test.com");
        assert_eq!(s, 2355200);
    }
}

use std::collections::{HashMap, HashSet};
use url::Url;

use crate::scoring;
use crate::types::*;
use crate::url_utils;

pub const MAX_SUGGESTIONS: usize = 12;
pub const MAX_DDG_MOBILE: usize = 5;
pub const MAX_TOP_HITS: usize = 2;
const MIN_IN_SUGGESTION_GROUP: usize = 5;

// Quality ranking matching Swift's ScoredSuggestion.Kind.quality
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Phrase,
    Website,
    InternalPage,
    HistoryEntry,
    BrowserTab,
    Bookmark,
    Favorite,
}

impl Kind {
    fn quality(self) -> i32 {
        match self {
            Kind::Phrase => 1,
            Kind::Website | Kind::InternalPage => 2,
            Kind::HistoryEntry => 3,
            Kind::BrowserTab => 4,
            Kind::Bookmark => 5,
            Kind::Favorite => 6,
        }
    }
}

#[derive(Debug, Clone)]
struct ScoredSuggestion {
    kind: Kind,
    url: Url,
    title: Option<String>,
    score: i64,
    visit_count: i64,
    failed_to_load: bool,
    tab_id: Option<String>,
    is_favorite: bool,
}

impl ScoredSuggestion {
    fn to_output(&self) -> SuggestionOutput {
        match self.kind {
            Kind::Favorite | Kind::Bookmark => SuggestionOutput::Bookmark {
                title: self.title.clone().unwrap_or_default(),
                url: self.url.to_string(),
                is_favorite: self.kind == Kind::Favorite,
                score: self.score,
            },
            Kind::HistoryEntry => SuggestionOutput::HistoryEntry {
                title: self.title.clone(),
                url: self.url.to_string(),
                score: self.score,
            },
            Kind::BrowserTab => SuggestionOutput::OpenTab {
                title: self.title.clone().unwrap_or_default(),
                url: self.url.to_string(),
                tab_id: self.tab_id.clone(),
                score: self.score,
            },
            Kind::InternalPage => SuggestionOutput::InternalPage {
                title: self.title.clone().unwrap_or_default(),
                url: self.url.to_string(),
                score: self.score,
            },
            Kind::Website => SuggestionOutput::Website {
                url: self.url.to_string(),
            },
            Kind::Phrase => SuggestionOutput::Phrase {
                phrase: self.title.clone().unwrap_or_default(),
            },
        }
    }
}

fn parse_url_lenient(s: &str) -> Option<Url> {
    Url::parse(s).ok().or_else(|| Url::parse(&format!("http://{}", s)).ok())
}

fn score_item(
    kind: Kind,
    url: &Url,
    title: Option<&str>,
    visit_count: i64,
    failed_to_load: bool,
    tab_id: Option<String>,
    is_favorite: bool,
    lowercased_query: &str,
    query_tokens: &[&str],
) -> Option<ScoredSuggestion> {
    let s = scoring::score(title, url, visit_count, lowercased_query, query_tokens);
    if s == 0 {
        return None;
    }
    Some(ScoredSuggestion {
        kind,
        url: url.clone(),
        title: title.map(|t| t.to_string()),
        score: s,
        visit_count,
        failed_to_load,
        tab_id,
        is_favorite,
    })
}

/// Extracts DDG suggestions from the API result.
fn ddg_suggestions(api_result: &Option<Vec<ApiSuggestion>>) -> Vec<SuggestionOutput> {
    let items = match api_result {
        Some(items) => items,
        None => return vec![],
    };
    items
        .iter()
        .filter_map(|item| {
            if item.is_nav == Some(true) {
                let url_str = format!("http://{}", &item.phrase);
                let url = Url::parse(&url_str).ok()?;
                Some(SuggestionOutput::Website {
                    url: url.to_string(),
                })
            } else {
                Some(SuggestionOutput::Phrase {
                    phrase: item.phrase.clone(),
                })
            }
        })
        .collect()
}

/// Extracts DDG domain suggestions (website-only) as ScoredSuggestion tuples.
fn ddg_domain_suggestions(
    api_result: &Option<Vec<ApiSuggestion>>,
) -> Vec<(ScoredSuggestion, HashSet<Kind>)> {
    let items = match api_result {
        Some(items) => items,
        None => return vec![],
    };
    items
        .iter()
        .filter_map(|item| {
            if item.is_nav != Some(true) {
                return None;
            }
            let url_str = format!("http://{}", &item.phrase);
            let url = Url::parse(&url_str).ok()?;
            let scored = ScoredSuggestion {
                kind: Kind::Website,
                url: url.clone(),
                title: Some(url.to_string()),
                score: 0,
                visit_count: 0,
                failed_to_load: false,
                tab_id: None,
                is_favorite: false,
            };
            let mut kinds = HashSet::new();
            kinds.insert(Kind::Website);
            Some((scored, kinds))
        })
        .collect()
}

/// Removes duplicates by naked URL string, keeping the highest quality entry.
fn remove_duplicates(
    suggestions: &[ScoredSuggestion],
) -> Vec<(ScoredSuggestion, HashSet<Kind>)> {
    let mut ordered_keys: Vec<String> = Vec::new();
    let mut seen_keys: HashSet<String> = HashSet::new();
    let mut grouped: HashMap<String, Vec<&ScoredSuggestion>> = HashMap::new();

    for s in suggestions {
        let key = url_utils::naked_string(&s.url);
        if seen_keys.insert(key.clone()) {
            ordered_keys.push(key.clone());
        }
        grouped.entry(key).or_default().push(s);
    }

    let mut result = Vec::new();
    for key in &ordered_keys {
        let group = match grouped.get(key) {
            Some(g) => g,
            None => continue,
        };
        // Pick the entry with the highest quality
        let best = group
            .iter()
            .max_by_key(|s| s.kind.quality())
            .unwrap();

        let suggestion_kinds: HashSet<Kind> = group.iter().map(|s| s.kind).collect();
        let visit_count: i64 = group
            .iter()
            .filter(|s| s.kind == Kind::HistoryEntry)
            .map(|s| s.visit_count)
            .sum();
        let tab_id = group
            .iter()
            .find(|s| s.kind == Kind::BrowserTab)
            .and_then(|s| s.tab_id.clone());
        let max_score = group.iter().map(|s| s.score).max().unwrap_or(0);

        let mut suggestion = (*best).clone();
        suggestion.score = max_score;
        suggestion.visit_count = visit_count;
        suggestion.tab_id = tab_id;

        result.push((suggestion, suggestion_kinds));
    }
    result
}

fn is_top_hit(scored: &ScoredSuggestion, kinds: &HashSet<Kind>, platform: &Platform) -> bool {
    let mut allowed: Vec<Kind> = vec![Kind::Website, Kind::Favorite, Kind::HistoryEntry];
    if matches!(platform, Platform::Mobile) {
        allowed.push(Kind::Bookmark);
    }
    if !kinds.iter().any(|k| allowed.contains(k)) {
        return false;
    }

    if *kinds == HashSet::from([Kind::HistoryEntry]) {
        return !scored.failed_to_load
            && (scored.visit_count > 3 || url_utils::is_root(&scored.url));
    }
    if *kinds == HashSet::from([Kind::BrowserTab]) {
        return false;
    }
    true
}

fn handle_top_hits_open_tab_case(
    top_hits_deduped: &[(ScoredSuggestion, HashSet<Kind>)],
) -> Vec<ScoredSuggestion> {
    let mut result: Vec<ScoredSuggestion> = top_hits_deduped.iter().map(|(s, _)| s.clone()).collect();

    if let Some((top_hit_suggestion, top_hit_kinds)) = top_hits_deduped.first() {
        let has_browser_tab = top_hit_kinds.contains(&Kind::BrowserTab);
        let has_navigational = top_hit_kinds.contains(&Kind::HistoryEntry)
            || top_hit_kinds.contains(&Kind::Bookmark)
            || top_hit_kinds.contains(&Kind::Favorite);

        if has_browser_tab && has_navigational {
            let new_kind = if top_hit_suggestion.kind == Kind::BrowserTab {
                top_hit_kinds
                    .iter()
                    .filter(|k| **k != Kind::BrowserTab)
                    .max_by_key(|k| k.quality())
                    .copied()
                    .unwrap_or(Kind::BrowserTab)
            } else {
                Kind::BrowserTab
            };

            let mut new_suggestion = top_hit_suggestion.clone();
            new_suggestion.kind = new_kind;
            let insertion_index = if new_kind == Kind::BrowserTab { 1 } else { 0 };
            result.insert(insertion_index, new_suggestion);

            if result.len() > MAX_TOP_HITS {
                result.truncate(MAX_TOP_HITS);
            }
        }
    }

    result
}

fn suggestion_output_matches(a: &SuggestionOutput, b: &SuggestionOutput) -> bool {
    match (a, b) {
        (SuggestionOutput::Website { url: u1 }, SuggestionOutput::Website { url: u2 }) => u1 == u2,
        (SuggestionOutput::Bookmark { url: u1, .. }, SuggestionOutput::Bookmark { url: u2, .. }) => {
            u1 == u2
        }
        (
            SuggestionOutput::HistoryEntry { url: u1, .. },
            SuggestionOutput::HistoryEntry { url: u2, .. },
        ) => u1 == u2,
        (
            SuggestionOutput::HistoryEntry { url: u1, .. },
            SuggestionOutput::Bookmark { url: u2, .. },
        )
        | (
            SuggestionOutput::Bookmark { url: u1, .. },
            SuggestionOutput::HistoryEntry { url: u2, .. },
        ) => u1 == u2,
        (SuggestionOutput::Phrase { phrase: p1 }, SuggestionOutput::Phrase { phrase: p2 }) => {
            p1 == p2
        }
        (
            SuggestionOutput::OpenTab { url: u1, .. },
            SuggestionOutput::OpenTab { url: u2, .. },
        ) => u1 == u2,
        _ => a == b,
    }
}

fn top_hits_contains(top_hits: &[SuggestionOutput], suggestion: &SuggestionOutput) -> bool {
    top_hits.iter().any(|th| suggestion_output_matches(th, suggestion))
}

/// Checks if a suggestion output's URL matches any top hit URL (by naked string comparison).
fn top_hits_contains_url(top_hits: &[SuggestionOutput], url_str: &str) -> bool {
    let target_naked = if let Ok(u) = Url::parse(url_str) {
        url_utils::naked_string(&u)
    } else {
        return false;
    };
    for th in top_hits {
        let th_url = match th {
            SuggestionOutput::Website { url, .. }
            | SuggestionOutput::Bookmark { url, .. }
            | SuggestionOutput::HistoryEntry { url, .. }
            | SuggestionOutput::OpenTab { url, .. }
            | SuggestionOutput::InternalPage { url, .. } => url,
            _ => continue,
        };
        if let Ok(u) = Url::parse(th_url) {
            if url_utils::naked_string(&u) == target_naked {
                return true;
            }
        }
    }
    false
}

pub fn process(input: &ProcessInput) -> ProcessOutput {
    let lower_query = input.query.trim().to_lowercase();
    let query_tokens: Vec<&str> = url_utils::tokenize(&lower_query);

    if lower_query.is_empty() {
        return ProcessOutput {
            top_hits: vec![],
            ddg_suggestions: vec![],
            local_suggestions: vec![],
            can_be_autocompleted: false,
        };
    }

    // STEP 1: Get DDG suggestions
    let ddg_all = ddg_suggestions(&input.api_result);

    // STEP 2: DDG domain suggestions
    let ddg_domains = ddg_domain_suggestions(&input.api_result);

    // STEP 3: Score all local sources
    let mut all_scored: Vec<ScoredSuggestion> = Vec::new();

    for bm in &input.bookmarks {
        if let Some(url) = parse_url_lenient(&bm.url) {
            let kind = if bm.is_favorite {
                Kind::Favorite
            } else {
                Kind::Bookmark
            };
            if let Some(s) = score_item(
                kind,
                &url,
                Some(&bm.title),
                0,
                false,
                None,
                bm.is_favorite,
                &lower_query,
                &query_tokens,
            ) {
                all_scored.push(s);
            }
        }
    }

    for tab in &input.open_tabs {
        if let Some(url) = parse_url_lenient(&tab.url) {
            if let Some(s) = score_item(
                Kind::BrowserTab,
                &url,
                Some(&tab.title),
                0,
                false,
                tab.tab_id.clone(),
                false,
                &lower_query,
                &query_tokens,
            ) {
                all_scored.push(s);
            }
        }
    }

    for h in &input.history {
        if let Some(url) = parse_url_lenient(&h.url) {
            if let Some(s) = score_item(
                Kind::HistoryEntry,
                &url,
                h.title.as_deref(),
                h.number_of_visits,
                h.failed_to_load,
                None,
                false,
                &lower_query,
                &query_tokens,
            ) {
                all_scored.push(s);
            }
        }
    }

    for ip in &input.internal_pages {
        if let Some(url) = parse_url_lenient(&ip.url) {
            if let Some(s) = score_item(
                Kind::InternalPage,
                &url,
                Some(&ip.title),
                0,
                false,
                None,
                false,
                &lower_query,
                &query_tokens,
            ) {
                all_scored.push(s);
            }
        }
    }

    // Sort by score descending, take top 100
    all_scored.sort_by(|a, b| b.score.cmp(&a.score));
    all_scored.truncate(100);

    // STEP 4: Deduplicate
    let deduped = remove_duplicates(&all_scored);

    // STEP 5: Combine navigational suggestions (deduped local sorted by score + DDG domains)
    let mut deduped_navigational: Vec<(ScoredSuggestion, HashSet<Kind>)> = deduped.clone();
    deduped_navigational.sort_by(|a, b| b.0.score.cmp(&a.0.score));
    deduped_navigational.extend(ddg_domains);

    // STEP 6: Find Top Hits
    let top_hits_deduped: Vec<(ScoredSuggestion, HashSet<Kind>)> = deduped_navigational
        .iter()
        .filter(|(s, kinds)| is_top_hit(s, kinds, &input.platform))
        .take(MAX_TOP_HITS)
        .cloned()
        .collect();

    // STEP 7: Handle open tab special case
    let final_top_hits_scored = handle_top_hits_open_tab_case(&top_hits_deduped);

    // STEP 8: Prepare final Top Hits
    let top_hits: Vec<SuggestionOutput> = final_top_hits_scored
        .iter()
        .map(|s| s.to_output())
        .collect();

    // STEP 9: Calculate remaining count
    let count_for_local = {
        let a = MAX_SUGGESTIONS.saturating_sub(top_hits.len() + MIN_IN_SUGGESTION_GROUP);
        let b = (lower_query.len() + 1).saturating_sub(top_hits.len());
        a.min(b)
    };

    // STEP 10: Build history, bookmarks, and open tabs suggestions
    let navigational_kinds: HashSet<Kind> = [
        Kind::HistoryEntry,
        Kind::Bookmark,
        Kind::Favorite,
        Kind::BrowserTab,
        Kind::InternalPage,
    ]
    .into_iter()
    .collect();

    let local_suggestions: Vec<SuggestionOutput> = deduped_navigational
        .iter()
        .filter(|(s, kinds)| {
            if !kinds.iter().any(|k| navigational_kinds.contains(k)) {
                return false;
            }
            let output = s.to_output();
            !top_hits_contains(&top_hits, &output)
        })
        .take(count_for_local)
        .map(|(s, _)| s.to_output())
        .collect();

    // STEP 11: Filter DDG suggestions that are already in top hits
    let max_ddg = MAX_SUGGESTIONS.saturating_sub(top_hits.len() + local_suggestions.len());
    let ddg_filtered: Vec<SuggestionOutput> = ddg_all
        .into_iter()
        .filter(|s| !top_hits_contains(&top_hits, s))
        .take(max_ddg)
        .collect();

    // STEP 12: Apply mobile limit
    let ddg_final = limit_suggestions_to_display(ddg_filtered, &input.platform);

    ProcessOutput {
        top_hits,
        ddg_suggestions: ddg_final,
        local_suggestions,
        can_be_autocompleted: false,
    }
}

fn limit_suggestions_to_display(
    suggestions: Vec<SuggestionOutput>,
    platform: &Platform,
) -> Vec<SuggestionOutput> {
    match platform {
        Platform::Desktop => suggestions,
        Platform::Mobile => suggestions.into_iter().take(MAX_DDG_MOBILE).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(query: &str, platform: Platform) -> ProcessInput {
        ProcessInput {
            query: query.to_string(),
            platform,
            bookmarks: vec![],
            history: vec![],
            open_tabs: vec![],
            internal_pages: vec![],
            api_result: None,
        }
    }

    fn api_result_basic() -> Vec<ApiSuggestion> {
        vec![
            ApiSuggestion { phrase: "Test".into(), is_nav: None },
            ApiSuggestion { phrase: "Test 2".into(), is_nav: None },
            ApiSuggestion { phrase: "www.example.com".into(), is_nav: None },
        ]
    }

    fn api_result_with_nav() -> Vec<ApiSuggestion> {
        vec![
            ApiSuggestion { phrase: "Test".into(), is_nav: None },
            ApiSuggestion { phrase: "Test 2".into(), is_nav: None },
            ApiSuggestion { phrase: "www.example.com".into(), is_nav: Some(true) },
            ApiSuggestion { phrase: "www.othersite.com".into(), is_nav: Some(false) },
        ]
    }

    fn duck_history_without_duckduckgo() -> Vec<HistoryInput> {
        vec![
            HistoryInput {
                url: "http://www.ducktails.com".into(),
                title: None,
                number_of_visits: 100,
                last_visit: 0.0,
                failed_to_load: false,
            },
            HistoryInput {
                url: "http://www.duck.com".into(),
                title: Some("DuckMail".into()),
                number_of_visits: 300,
                last_visit: 0.0,
                failed_to_load: false,
            },
        ]
    }

    fn a_history() -> Vec<HistoryInput> {
        vec![HistoryInput {
            url: "http://www.duckduckgo.com".into(),
            title: None,
            number_of_visits: 1000,
            last_visit: 0.0,
            failed_to_load: false,
        }]
    }

    fn some_bookmarks() -> Vec<BookmarkInput> {
        vec![
            BookmarkInput { url: "http://duckduckgo.com".into(), title: "DuckDuckGo".into(), is_favorite: true },
            BookmarkInput { url: "spreadprivacy.com".into(), title: "Test 2".into(), is_favorite: true },
            BookmarkInput { url: "wikipedia.org".into(), title: "Wikipedia".into(), is_favorite: false },
        ]
    }

    fn some_internal_pages() -> Vec<InternalPageInput> {
        vec![
            InternalPageInput { title: "Settings".into(), url: "duck://settings".into() },
            InternalPageInput { title: "Bookmarks".into(), url: "duck://bookmarks".into() },
            InternalPageInput { title: "Duck Player Settings".into(), url: "duck://bookmarks/duck-player".into() },
        ]
    }

    #[test]
    fn test_empty_query_returns_empty() {
        let input = make_input("", Platform::Desktop);
        let result = process(&input);
        assert!(result.top_hits.is_empty());
        assert!(result.ddg_suggestions.is_empty());
        assert!(result.local_suggestions.is_empty());
    }

    #[test]
    fn test_history_in_top_hits() {
        let mut input = make_input("Duck", Platform::Mobile);
        input.history = duck_history_without_duckduckgo();
        input.api_result = Some(api_result_basic());

        let result = process(&input);
        let has_duckmail = result.top_hits.iter().any(|s| match s {
            SuggestionOutput::HistoryEntry { title: Some(t), .. } => t == "DuckMail",
            _ => false,
        });
        assert!(has_duckmail, "DuckMail should be in top hits");
        assert_eq!(result.top_hits.len(), 2);
        assert_eq!(result.local_suggestions.len(), 0);
    }

    #[test]
    fn test_mobile_bookmarks_in_top_hits() {
        let mut input = make_input("Duck", Platform::Mobile);
        input.bookmarks = some_bookmarks();
        input.api_result = Some(api_result_basic());

        let result = process(&input);
        let has_duckduckgo = result.top_hits.iter().any(|s| match s {
            SuggestionOutput::Bookmark { title, .. } => title == "DuckDuckGo",
            _ => false,
        });
        assert!(has_duckduckgo, "DuckDuckGo bookmark should be in top hits on mobile");
    }

    #[test]
    fn test_mobile_ddg_suggestions_limited() {
        let api: Vec<_> = (0..30).map(|i| ApiSuggestion {
            phrase: format!("suggestion_{i}"),
            is_nav: if i % 10 == 0 { Some(true) } else { None },
        }).collect();

        let mut input = make_input("Duck", Platform::Mobile);
        input.api_result = Some(api);

        let result = process(&input);
        assert!(result.top_hits.len() <= MAX_TOP_HITS);
        assert!(result.ddg_suggestions.len() <= MAX_DDG_MOBILE);
    }

    #[test]
    fn test_desktop_ddg_suggestions_limited_by_max() {
        let api: Vec<_> = (0..30).map(|i| ApiSuggestion {
            phrase: format!("unique_phrase_{i}"),
            is_nav: Some(false),
        }).collect();

        let mut input = make_input("Duck", Platform::Desktop);
        input.api_result = Some(api);

        let result = process(&input);
        assert_eq!(result.top_hits.len(), 0);
        assert_eq!(result.ddg_suggestions.len(), MAX_SUGGESTIONS);
    }

    #[test]
    fn test_deduplication_picks_highest_quality() {
        let mut input = make_input("DuckDuckGo", Platform::Desktop);
        input.history = a_history();
        input.bookmarks = some_bookmarks();
        input.internal_pages = some_internal_pages();
        input.api_result = Some(api_result_basic());

        let result = process(&input);
        assert_eq!(result.top_hits.len(), 1);
        let title = match &result.top_hits[0] {
            SuggestionOutput::Bookmark { title, .. } => Some(title.as_str()),
            _ => None,
        };
        assert_eq!(title, Some("DuckDuckGo"));
    }

    #[test]
    fn test_nav_suggestions_in_top_hits() {
        let mut input = make_input("DuckDuckGo", Platform::Desktop);
        input.history = a_history();
        input.bookmarks = some_bookmarks();
        input.internal_pages = some_internal_pages();
        input.api_result = Some(api_result_with_nav());

        let result = process(&input);
        assert_eq!(result.top_hits.len(), 2);
        let last_url = match &result.top_hits[1] {
            SuggestionOutput::Website { url, .. } => Some(url.as_str()),
            _ => None,
        };
        assert_eq!(last_url, Some("http://www.example.com/"));
    }

    #[test]
    fn test_website_in_top_hits_removed_from_ddg_suggestions() {
        let mut input = make_input("DuckDuckGo", Platform::Desktop);
        input.api_result = Some(api_result_with_nav());

        let result = process(&input);
        assert_eq!(result.top_hits.len(), 1);
        assert!(matches!(&result.top_hits[0], SuggestionOutput::Website { url, .. } if url == "http://www.example.com/"));

        let has_example_website = result.ddg_suggestions.iter().any(|s| match s {
            SuggestionOutput::Website { url, .. } => url.contains("example.com"),
            _ => false,
        });
        assert!(!has_example_website, "example.com website should not be in ddg_suggestions");
    }
}

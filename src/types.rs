use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Desktop,
    Mobile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessInput {
    pub query: String,
    pub platform: Platform,
    #[serde(default)]
    pub bookmarks: Vec<BookmarkInput>,
    #[serde(default)]
    pub history: Vec<HistoryInput>,
    #[serde(default)]
    pub open_tabs: Vec<OpenTabInput>,
    #[serde(default)]
    pub internal_pages: Vec<InternalPageInput>,
    pub api_result: Option<Vec<ApiSuggestion>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookmarkInput {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HistoryInput {
    pub url: String,
    pub title: Option<String>,
    #[serde(default)]
    pub number_of_visits: i64,
    #[serde(default)]
    pub last_visit: f64,
    #[serde(default)]
    pub failed_to_load: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenTabInput {
    pub url: String,
    pub title: String,
    pub tab_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InternalPageInput {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiSuggestion {
    pub phrase: String,
    #[serde(default)]
    pub is_nav: Option<bool>,
}

// --- Output types ---

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProcessOutput {
    pub top_hits: Vec<SuggestionOutput>,
    pub ddg_suggestions: Vec<SuggestionOutput>,
    pub local_suggestions: Vec<SuggestionOutput>,
    pub can_be_autocompleted: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SuggestionOutput {
    Phrase {
        phrase: String,
    },
    Website {
        url: String,
    },
    Bookmark {
        title: String,
        url: String,
        is_favorite: bool,
        score: i64,
    },
    HistoryEntry {
        title: Option<String>,
        url: String,
        score: i64,
    },
    InternalPage {
        title: String,
        url: String,
        score: i64,
    },
    OpenTab {
        title: String,
        url: String,
        tab_id: Option<String>,
        score: i64,
    },
    Unknown {
        value: String,
    },
    AskAiChat {
        value: String,
    },
}

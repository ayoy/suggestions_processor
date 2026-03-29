# Suggestion Scoring

This document describes how the suggestion scoring algorithm works. The implementation lives in `src/scoring.rs`.

## Overview

Each local suggestion (bookmark, history entry, open tab, internal page) is scored against the user's query based on how well its title and URL match. Suggestions that don't match at all receive a score of 0 and are discarded. Non-zero scores are amplified and combined with visit count to produce the final ranking value.

## Inputs

| Parameter | Type | Description |
|---|---|---|
| `title` | `Option<&str>` | Page title (lowercased for comparison) |
| `url` | `&Url` | Full URL of the suggestion |
| `visit_count` | `i64` | Number of visits from browsing history |
| `lowercased_query` | `&str` | User's query, trimmed and lowercased |
| `query_tokens` | `&[&str]` | Query split by whitespace |

## Match Hierarchy

The algorithm evaluates match conditions in order and stops at the first one that succeeds. This means a URL match always takes priority over a title match, and so on.

### 1. URL starts with query (300 points)

The URL's [naked string](#url-normalization) is checked for a prefix match against the query.

```
query: "testcase.com/no"
url:   "https://www.testcase.com/notroot"
naked: "testcase.com/notroot"
       ^^^^^^^^^^^^^^^^ match → 300 points
```

If the URL is a [root URL](#root-url-bonus), an additional **+2000** points are added.

### 2. Title starts with query at word boundary (200 points)

The lowercased title is checked using `leading_boundary_starts_with`, which matches from the start of the string or from the first alphanumeric character (skipping leading punctuation like quotes).

```
query: "cats"
title: "\"Cats and Dogs\""   → trimmed to "Cats and Dogs\"" → match
title: "«Рукописи не горят»" → query "рукописи" matches after «
```

If the URL is a [root URL](#root-url-bonus), an additional **+2000** points are added.

### 3. Domain contains query (150 points)

**Requires query length > 2 characters.**

The domain (host without `www.`) is checked for whether it contains the query anywhere.

```
query:  "web" (3 chars)
domain: "website.com"
         ^^^ contains → 150 points

query:  "we" (2 chars)
         → skipped, query too short
```

### 4. Title word contains query (100 points)

**Requires query length > 2 characters.**

The lowercased title is checked for the query preceded by a space (word boundary match).

```
query: "duck" (4 chars)
title: "Big duck pond"
           ^^^^ " duck" found → 100 points
```

### 5. Multi-token query match (10-80 points)

**Requires multiple tokens in the query.**

All tokens must be found in either the title (at a word boundary) or the URL (naked string prefix). If any token is missing, the score is 0.

| Condition | Points |
|---|---|
| All tokens found | 10 |
| + first token at URL start | +70 |
| + first token at title start (if not URL) | +50 |

```
query:  "cats dogs"
title:  "Cats and Dogs"
url:    "https://www.other.com"

"cats" → title boundary match ✓
"dogs" → title contains " dogs" ✓
All tokens found → 10 points
First token "cats" at title start → +50
Total: 60 points
```

## Root URL Bonus

A root URL is one with no meaningful path, query string, or fragment:

- `https://example.com` or `https://example.com/` → root
- `https://example.com/path` → not root

The **+2000** bonus applies only to match types 1 (URL match) and 2 (title match). It heavily promotes root domains so that typing a domain name surfaces the homepage above deep links.

## Final Score Transformation

All non-zero base scores are transformed before ranking:

```
final_score = (base_score << 10) + visit_count
```

The left shift by 10 bits (multiplication by 1024) ensures the match type dominates the ranking. Visit count serves as a fine-grained tiebreaker between suggestions with the same match type.

### Examples

| Scenario | Base | Shift | Visit Count | Final |
|---|---|---|---|---|
| URL match, 100 visits | 300 | 307,200 | 100 | 307,300 |
| Root URL match, 0 visits | 2,300 | 2,355,200 | 0 | 2,355,200 |
| Title start match, 50 visits | 200 | 204,800 | 50 | 204,850 |
| Domain contains, 500 visits | 150 | 153,600 | 500 | 154,100 |
| No match | 0 | 0 | any | 0 |

## Score Summary

| Priority | Match Type | Base Points | Root Bonus |
|---|---|---|---|
| 1 | URL starts with query | 300 | +2000 |
| 2 | Title starts with query (word boundary) | 200 | +2000 |
| 3 | Domain contains query (>2 chars) | 150 | — |
| 4 | Title word contains query (>2 chars) | 100 | — |
| 5 | All multi-token query found | 10 | — |
| 5a | + first token at URL start | +70 | — |
| 5b | + first token at title start | +50 | — |

## URL Normalization

Several URL utility functions (in `src/url_utils.rs`) support the scoring logic:

**`naked_string(url)`** — Removes scheme, `www.` prefix, and trailing slash. Preserves query string and fragment.
```
"https://www.example.com/path/" → "example.com/path"
"https://example.com/path?q=1#frag" → "example.com/path?q=1#frag"
```

**`host_without_www(url)`** — Returns the domain without `www.` prefix.
```
"https://www.website.com" → "website.com"
```

**`leading_boundary_starts_with(s, prefix)`** — Checks if a string starts with `prefix`, either directly or after trimming leading non-alphanumeric characters. Handles quoted and Unicode-prefixed strings.

**`tokenize(query)`** — Splits by whitespace, filtering empty tokens.

## How Scores Are Used

After scoring, the processing pipeline in `src/processing.rs` uses scores for:

1. **Filtering** — Suggestions with score 0 are discarded
2. **Sorting** — Scored suggestions are sorted descending; top 100 kept
3. **Deduplication** — Duplicate URLs (by naked string) are merged: highest quality kind is kept, max score is used, and visit counts from history entries are summed
4. **Top hits selection** — Up to 2 top hits are picked from the highest-scoring navigational suggestions, subject to eligibility rules (e.g. history-only entries need >3 visits or a root URL)
5. **Local suggestions** — Remaining navigational suggestions fill available slots, count limited by query length
6. **Platform limits** — Desktop allows up to 12 total suggestions; mobile caps DDG suggestions at 5

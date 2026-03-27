# suggestions_processor

A Rust library that ranks and deduplicates browser search suggestions from multiple sources (bookmarks, history, open tabs, internal pages, and DuckDuckGo API results). Designed for use in DuckDuckGo browsers via a C FFI interface.

## Architecture

```
src/
├── lib.rs           # C FFI entry points
├── types.rs         # Input/output data structures
├── processing.rs    # Main suggestion ranking and deduplication
├── scoring.rs       # Query-matching score calculation
└── url_utils.rs     # URL normalization and tokenization
```

### Processing pipeline

1. **Score** all local sources (bookmarks, history, open tabs, internal pages) against the query
2. **Deduplicate** by normalized URL, keeping the highest-quality version and aggregating visit counts
3. **Select top hits** (up to 2) from high-scoring or frequently visited entries
4. **Build DDG suggestions** from API results, removing duplicates already present in top hits
5. **Apply platform limits** — desktop allows up to 12 total suggestions; mobile caps DDG suggestions at 5

### Scoring

Scores are based on how the query matches each suggestion's title and URL:

| Match type | Base points |
|---|---|
| URL starts with query | 300 |
| Title starts with query (word boundary) | 200 |
| Domain contains query (>2 chars) | 150 |
| Title word contains query (>2 chars) | 100 |
| All query tokens found | 10–70 (context-dependent) |
| Root URL bonus | +2000 |

The final score is `(base_score << 10) + visit_count`, giving score priority while using visit count as a tiebreaker.

## FFI interface

The library exposes two C-compatible functions:

```c
// Takes a JSON string, returns a JSON string with ranked suggestions.
// Caller must free the result with ddg_sp_free_string.
char *ddg_sp_process_json(const char *input);

// Frees a string returned by ddg_sp_process_json.
void ddg_sp_free_string(char *ptr);
```

Input and output are JSON-serialized. See `types.rs` for the full schema (`ProcessInput` / `ProcessOutput`).

## Building

### Prerequisites

- Rust toolchain (stable)
- For Apple targets: Xcode command line tools, `cbindgen` (`cargo install cbindgen`)

### Run tests

```sh
cargo test
```

### Build for Apple platforms

```sh
./scripts/build_apple.sh
```

This produces `dist/apple/SuggestionsProcessorRust.xcframework.zip` containing a universal xcframework for:
- macOS (arm64, x86_64)
- iOS (arm64)
- iOS Simulator (arm64, x86_64)

Minimum deployment targets: macOS 11.3, iOS 15.0.

## Integration

In the DuckDuckGo browser codebase, the xcframework is consumed as a Swift package (`SuggestionProcessing`). The Swift wrapper (`Processor.process`) calls the FFI functions and maps the JSON response back into native types.

A feature flag (`unifiedSuggestionsEngine`) controls whether the Rust implementation or the original Swift implementation is used at runtime.

use crate::model::Tool;
use bm25::{Document, Language, SearchEngineBuilder, SearchResult as BM25Result};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::HashMap;

pub struct SearchResult {
    pub tool: Tool,
    pub score: f64,
}

/// Synonym map for CLI domain query expansion
const SYNONYMS: &[(&str, &[&str])] = &[
    ("search", &["find", "grep", "lookup", "locate"]),
    ("find", &["search", "grep", "locate", "fd"]),
    ("grep", &["search", "find", "ripgrep", "rg"]),
    ("convert", &["transform", "translate", "encode", "decode"]),
    ("transform", &["convert", "translate"]),
    ("http", &["web", "api", "request", "curl", "rest"]),
    ("request", &["http", "api", "curl", "fetch"]),
    ("monitor", &["watch", "observe", "dashboard", "top", "htop"]),
    ("process", &["ps", "pid", "task", "daemon"]),
    ("editor", &["edit", "vim", "nano", "text"]),
    ("diff", &["delta", "patch", "difftool"]),
    ("compare", &["diff", "delta"]),
    (
        "benchmark",
        &["bench", "performance", "speed", "measure", "hyperfine"],
    ),
    ("profile", &["benchmark", "perf", "measure"]),
    ("compress", &["zip", "gzip", "tar", "archive", "pack"]),
    ("archive", &["compress", "zip", "tar", "pack"]),
    ("download", &["fetch", "get", "wget", "curl"]),
    ("rename", &["move", "mv", "batch-rename"]),
    ("format", &["prettify", "beautify", "lint", "pretty-print"]),
    (
        "pretty",
        &["format", "beautify", "syntax-highlighting", "colorize"],
    ),
    ("container", &["docker", "podman", "kubernetes", "k8s"]),
    ("disk", &["storage", "space", "du", "usage"]),
    ("directory", &["folder", "dir", "cd", "navigate", "path"]),
    ("navigate", &["cd", "jump", "directory", "autojump"]),
    ("prompt", &["shell", "ps1", "starship", "powerline"]),
    (
        "help",
        &["man", "manual", "documentation", "tldr", "cheatsheet"],
    ),
    ("man", &["help", "manual", "documentation", "tldr"]),
    (
        "dns",
        &["nameserver", "resolve", "lookup", "dig", "nslookup"],
    ),
    ("git", &["version-control", "vcs"]),
    ("cat", &["view", "display", "print", "bat", "less"]),
    ("ls", &["list", "directory", "exa", "eza", "lsd"]),
    ("spreadsheet", &["csv", "xlsx", "tsv", "tabular", "excel"]),
    ("json", &["jq", "structured-data"]),
    ("csv", &["comma-separated", "tabular", "spreadsheet", "tsv"]),
    ("terminal", &["shell", "tty", "tui", "cli"]),
    ("multiplexer", &["tmux", "screen", "zellij", "session"]),
    ("interactive", &["tui", "select", "picker", "menu", "fuzzy"]),
    ("file", &["files", "filesystem", "fs"]),
    ("image", &["picture", "photo", "png", "jpg", "svg"]),
    ("video", &["media", "stream", "mp4", "ffmpeg"]),
    ("database", &["db", "sql", "sqlite", "postgres", "mysql"]),
    ("clipboard", &["copy", "paste", "pbcopy", "xclip"]),
    ("color", &["colour", "hex", "rgb"]),
    ("test", &["testing", "spec", "check", "verify"]),
    ("count", &["lines", "wc", "statistics", "loc", "sloc"]),
];

const QUERY_NOISE: &[&str] = &[
    "fast",
    "modern",
    "simple",
    "easy",
    "best",
    "good",
    "nice",
    "great",
    "better",
    "alternative",
    "replacement",
    "like",
    "similar",
    "beautiful",
    "tool",
    "tools",
    "command",
    "program",
    "app",
    "application",
    "utility",
    "want",
    "need",
    "looking",
    "for",
    "something",
    "that",
    "can",
    "way",
    "how",
    "what",
    "which",
    "the",
    "my",
    "your",
    "please",
    "show",
    "me",
    "i",
    "a",
    "an",
    "is",
    "are",
    "of",
    "to",
    "in",
    "with",
    "using",
];

/// Popularity boost using stars (primary) or brew install count (fallback)
fn popularity_boost(tool: &Tool) -> f64 {
    // Primary: GitHub stars
    if let Some(s) = tool.stars {
        if s > 0 {
            return match s {
                s if s > 50000 => 20.0,
                s if s > 10000 => 12.0 + 8.0 * (s as f64 - 10000.0) / 40000.0,
                s if s > 1000 => 4.0 + 8.0 * (s as f64 - 1000.0) / 9000.0,
                s => (s as f64 / 1000.0) * 4.0,
            };
        }
    }
    // Fallback: Homebrew install-on-request (annual)
    if let Some(installs) = tool.brew_installs_30d {
        if installs > 0 {
            return match installs {
                i if i > 500000 => 18.0,
                i if i > 100000 => 10.0 + 8.0 * (i as f64 - 100000.0) / 400000.0,
                i if i > 10000 => 4.0 + 6.0 * (i as f64 - 10000.0) / 90000.0,
                i => (i as f64 / 10000.0) * 4.0,
            };
        }
    }
    0.5 // no popularity data — lower default
}

fn preprocess_query(query: &str) -> String {
    let terms: Vec<&str> = query
        .split_whitespace()
        .filter(|t| !QUERY_NOISE.contains(&t.to_lowercase().as_str()))
        .collect();
    if terms.is_empty() {
        query.to_string()
    } else {
        terms.join(" ")
    }
}

fn build_synonym_map() -> HashMap<&'static str, &'static [&'static str]> {
    SYNONYMS.iter().copied().collect()
}

fn expand_query(query: &str, syn_map: &HashMap<&str, &[&str]>) -> String {
    let terms: Vec<&str> = query.split_whitespace().collect();
    // Repeat original terms 2x to weight them higher than synonyms
    let mut expanded = Vec::new();
    for _ in 0..2 {
        for term in &terms {
            expanded.push(term.to_string());
        }
    }
    for term in &terms {
        let lower = term.to_lowercase();
        if let Some(syns) = syn_map.get(lower.as_str()) {
            for syn in *syns {
                if !expanded.iter().any(|e| e.eq_ignore_ascii_case(syn)) {
                    expanded.push(syn.to_string());
                }
            }
        }
    }
    expanded.join(" ")
}

/// Build searchable text for BM25 indexing with field weighting via repetition.
/// Name 3x, binary 3x, tags+category 2x, description 1x.
fn build_search_text(tool: &Tool) -> String {
    let mut parts = Vec::new();

    // Name 3x weight
    for _ in 0..3 {
        parts.push(tool.name.clone());
        if let Some(ref bin) = tool.binary {
            parts.push(bin.clone());
        }
    }

    // Category + tags 2x weight
    for _ in 0..2 {
        parts.push(tool.category.clone());
        for tag in &tool.tags {
            parts.push(tag.clone());
        }
    }

    // Description 1x
    parts.push(tool.desc.clone());

    parts.join(" ")
}

pub fn search(tools: &[Tool], query: &str, max_results: usize) -> Vec<SearchResult> {
    if tools.is_empty() {
        return vec![];
    }

    let syn_map = build_synonym_map();
    let query_for_search = preprocess_query(query);
    let query_lower = query.to_lowercase(); // original for exact matching

    // BM25 search
    let documents: Vec<Document<usize>> = tools
        .iter()
        .enumerate()
        .map(|(i, t)| Document {
            id: i,
            contents: build_search_text(t),
        })
        .collect();

    let engine = SearchEngineBuilder::<usize>::with_documents(Language::English, documents)
        .b(0.5) // lower length normalization (docs are padded via repetition)
        .build();

    let expanded = expand_query(&query_for_search, &syn_map);
    // Always fetch enough candidates for re-ranking (category boost, popularity, etc.)
    let bm25_fetch = (max_results * 5).max(100);
    let bm25_results: Vec<BM25Result<usize>> = engine.search(&expanded, bm25_fetch);

    // Fuzzy name matching as fallback
    let fuzzy_matcher = SkimMatcherV2::default();

    let mut scored: Vec<SearchResult> = Vec::new();

    for bm25_res in &bm25_results {
        let idx = bm25_res.document.id;
        let tool = &tools[idx];
        let bm25_score = bm25_res.score as f64;

        // Name exact match bonus
        let name_lower = tool.name.to_lowercase();
        let name_bonus = if name_lower == query_lower {
            50.0
        } else if tool
            .binary
            .as_ref()
            .is_some_and(|b: &String| b.eq_ignore_ascii_case(&query_lower))
        {
            45.0
        } else {
            0.0
        };

        // Description match bonus — reward when query terms appear directly in description
        let desc_lower = tool.desc.to_lowercase();
        let query_terms: Vec<&str> = query_for_search.split_whitespace().collect();
        let desc_match_count = query_terms
            .iter()
            .filter(|t| t.len() > 2 && desc_lower.contains(&t.to_lowercase()))
            .count();
        let desc_bonus = desc_match_count as f64 * 5.0;

        // Category match bonus — if query terms appear in the category, boost
        // Uses stemming-like matching (checks if cat word starts with query term or vice versa)
        let cat_lower = tool.category.to_lowercase();
        let cat_words: Vec<&str> = cat_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2)
            .collect();
        let cat_bonus = {
            let matching_terms = query_for_search
                .to_lowercase()
                .split_whitespace()
                .filter(|t| {
                    t.len() > 2
                        && cat_words
                            .iter()
                            .any(|cw| cw.starts_with(t) || t.starts_with(cw))
                })
                .count();
            if desc_match_count > 0 {
                matching_terms as f64 * 8.0
            } else {
                matching_terms as f64 * 2.0 // weak bonus if category matches but description doesn't
            }
        };

        // Popularity boost (normalized 0-20) — uses stars, falls back to brew installs
        let pop_boost = popularity_boost(tool);

        let final_score = bm25_score + name_bonus + cat_bonus + pop_boost + desc_bonus;
        scored.push(SearchResult {
            tool: tool.clone(),
            score: final_score,
        });
    }

    // Also check fuzzy name matches and tag matches not in BM25 results
    let bm25_indices: std::collections::HashSet<usize> =
        bm25_results.iter().map(|r| r.document.id).collect();

    // For short queries, require higher fuzzy threshold to avoid false positives
    let fuzzy_threshold = if query_lower.len() <= 3 { 80 } else { 50 };

    for (i, tool) in tools.iter().enumerate() {
        if bm25_indices.contains(&i) {
            continue;
        }
        let name_lower = tool.name.to_lowercase();
        let fuzzy_score = fuzzy_matcher
            .fuzzy_match(&name_lower, &query_lower)
            .unwrap_or(0);

        let bin_score = tool
            .binary
            .as_ref()
            .and_then(|b| fuzzy_matcher.fuzzy_match(&b.to_lowercase(), &query_lower))
            .unwrap_or(0);

        // Exact tag match bonus (helps short queries like "rg", "fd")
        let tag_match = tool.tags.iter().any(|t| t == &query_lower);

        let best_fuzzy = fuzzy_score.max(bin_score);
        if best_fuzzy > fuzzy_threshold || tag_match {
            let pop_boost = popularity_boost(tool);
            let tag_bonus = if tag_match { 20.0 } else { 0.0 };
            scored.push(SearchResult {
                tool: tool.clone(),
                score: best_fuzzy as f64 * 0.3 + pop_boost + tag_bonus,
            });
        }
    }

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(max_results);
    scored
}

pub fn filter_by_category(tools: &[Tool], category: &str) -> Vec<Tool> {
    let cat_lower = category.to_lowercase();
    let mut filtered: Vec<Tool> = tools
        .iter()
        .filter(|t| t.category.to_lowercase().contains(&cat_lower))
        .cloned()
        .collect();
    filtered.sort_by(|a, b| b.stars.unwrap_or(0).cmp(&a.stars.unwrap_or(0)));
    filtered
}

pub fn get_categories(tools: &[Tool]) -> Vec<(String, usize)> {
    let mut map = std::collections::BTreeMap::new();
    for tool in tools {
        *map.entry(tool.category.clone()).or_insert(0usize) += 1;
    }
    let mut cats: Vec<(String, usize)> = map.into_iter().collect();
    cats.sort_by(|a, b| b.1.cmp(&a.1));
    cats
}

pub fn find_tool(tools: &[Tool], name: &str) -> Option<Tool> {
    let name_lower = name.to_lowercase();
    tools
        .iter()
        .find(|t| {
            t.name.to_lowercase() == name_lower
                || t.binary
                    .as_ref()
                    .is_some_and(|b| b.to_lowercase() == name_lower)
        })
        .cloned()
}

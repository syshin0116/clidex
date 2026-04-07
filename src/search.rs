use crate::model::Tool;
use bm25::{Document, Language, SearchEngine, SearchEngineBuilder, SearchResult as BM25Result};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::collections::HashMap;

/// Simple Levenshtein (edit) distance for typo detection.
/// Handles transpositions that subsequence matchers like nucleo miss.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate().take(m + 1) {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate().take(n + 1) {
        *val = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

pub struct SearchResult {
    pub tool: Tool,
    pub score: f64,
    /// Index into the original tools slice (used by hybrid search)
    pub tool_idx: usize,
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

/// Minimum lexical score (BM25 + bonuses, excluding popularity) for a result to be included.
/// Exact name/binary matches bypass this threshold.
const MIN_LEXICAL_THRESHOLD: f64 = 2.0;

/// Minimum semantic similarity for a result to be included in hybrid search.
#[cfg(feature = "semantic")]
const MIN_SEMANTIC_SIMILARITY: f32 = 0.3;

/// Popularity boost using stars (primary) or brew install count (fallback).
/// Max ~8.0 — acts as tie-breaker, not a primary ranking signal.
fn popularity_boost(tool: &Tool) -> f64 {
    // Primary: GitHub stars
    if let Some(s) = tool.stars {
        if s > 0 {
            return match s {
                s if s > 50000 => 8.0,
                s if s > 10000 => 5.0 + 3.0 * (s as f64 - 10000.0) / 40000.0,
                s if s > 1000 => 2.0 + 3.0 * (s as f64 - 1000.0) / 9000.0,
                s => (s as f64 / 1000.0) * 2.0,
            };
        }
    }
    // Fallback: Homebrew install-on-request (annual)
    if let Some(installs) = tool.brew_installs_365d {
        if installs > 0 {
            return match installs {
                i if i > 500000 => 7.0,
                i if i > 100000 => 4.0 + 3.0 * (i as f64 - 100000.0) / 400000.0,
                i if i > 10000 => 2.0 + 2.0 * (i as f64 - 10000.0) / 90000.0,
                i => (i as f64 / 10000.0) * 2.0,
            };
        }
    }
    0.5 // no popularity data — lower default
}

/// Check if a term matches within text at a word boundary.
/// Returns true if `term` appears in `text` as a complete token or prefix of a token.
fn word_boundary_match(text: &str, term: &str) -> bool {
    for word in text.split(|c: char| !c.is_alphanumeric() && c != '-') {
        if word.is_empty() {
            continue;
        }
        let w = word.to_lowercase();
        if w == term || w.starts_with(term) || term.starts_with(&w) {
            return true;
        }
    }
    false
}

/// Compute intent coverage bonus based on how many query terms appear in
/// the tool's name, binary, description, tags, and category.
/// Returns (coverage_bonus, covered_count, total_checkable_terms).
///
/// Uses token-aware matching:
/// - name/binary: exact match
/// - tags: exact or prefix match
/// - category: prefix match (stemming-like)
/// - description: word-boundary match (not raw substring)
fn intent_coverage(query_terms: &[&str], tool: &Tool) -> (f64, usize, usize) {
    if query_terms.is_empty() {
        return (0.0, 0, 0);
    }

    let name_lower = tool.name.to_lowercase();
    let bin_lower = tool
        .binary
        .as_ref()
        .map(|b| b.to_lowercase())
        .unwrap_or_default();
    let desc_lower = tool.desc.to_lowercase();
    let cat_lower = tool.category.to_lowercase();
    let tags_lower: Vec<String> = tool.tags.iter().map(|t| t.to_lowercase()).collect();

    let covered = query_terms
        .iter()
        .filter(|t| {
            let tl = t.to_lowercase();
            if tl.len() <= 2 {
                // Short terms: only match against name, binary, or exact tag match
                // (prevents false positives from short substrings in descriptions)
                name_lower == tl || bin_lower == tl || tags_lower.iter().any(|tag| tag == &tl)
            } else {
                // Token-aware matching per field:
                // 1. Name/binary: exact match
                name_lower == tl
                    || bin_lower == tl
                    // 2. Tags: exact or prefix match
                    || tags_lower
                        .iter()
                        .any(|tag| tag == &tl || tag.starts_with(&tl) || tl.starts_with(tag.as_str()))
                    // 3. Category: word-boundary prefix match
                    || word_boundary_match(&cat_lower, &tl)
                    // 4. Description: word-boundary match
                    || word_boundary_match(&desc_lower, &tl)
            }
        })
        .count();
    let total = query_terms.len();

    let bonus = if covered == total {
        // All terms covered → strong intent match
        12.0
    } else if total > 1 && covered > 0 {
        // Partial coverage → proportional bonus
        (covered as f64 / total as f64) * 6.0
    } else {
        0.0
    };

    (bonus, covered, total)
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

/// Pre-built search index that caches the BM25 engine for repeated queries.
/// Use this when running multiple searches against the same tool set.
pub struct SearchIndex {
    tools: Vec<Tool>,
    engine: SearchEngine<usize>,
    syn_map: HashMap<&'static str, &'static [&'static str]>,
}

impl SearchIndex {
    /// Build a search index from a tool list. The BM25 engine is constructed once.
    pub fn new(tools: Vec<Tool>) -> Self {
        let documents: Vec<Document<usize>> = tools
            .iter()
            .enumerate()
            .map(|(i, t)| Document {
                id: i,
                contents: build_search_text(t),
            })
            .collect();

        let engine = SearchEngineBuilder::<usize>::with_documents(Language::English, documents)
            .b(0.5)
            .build();

        let syn_map = build_synonym_map();

        Self {
            tools,
            engine,
            syn_map,
        }
    }

    /// Search the cached index.
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        search_with_engine(&self.tools, &self.engine, &self.syn_map, query, max_results)
    }

    /// Hybrid search combining cached BM25 + semantic similarity via RRF.
    #[cfg(feature = "semantic")]
    pub fn hybrid_search(
        &self,
        query: &str,
        max_results: usize,
        embeddings: &[Vec<f32>],
        query_embedding: &[f32],
    ) -> Vec<SearchResult> {
        if self.tools.is_empty() || embeddings.len() != self.tools.len() {
            return self.search(query, max_results);
        }

        // 1. BM25 search using cached engine
        let bm25_results = self.search(query, max_results * 3);
        let has_lexical_confidence = !bm25_results.is_empty();
        let bm25_ranked: Vec<(usize, f64)> =
            bm25_results.iter().map(|r| (r.tool_idx, r.score)).collect();

        // 2. Semantic search — cosine similarity, filtered by minimum threshold
        let mut semantic_scores: Vec<(usize, f32)> = embeddings
            .iter()
            .enumerate()
            .map(|(i, emb)| (i, crate::semantic::cosine_similarity(query_embedding, emb)))
            .filter(|(_, sim)| *sim >= MIN_SEMANTIC_SIMILARITY)
            .collect();
        semantic_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let has_semantic_confidence = !semantic_scores.is_empty();
        semantic_scores.truncate(max_results * 3);

        // Gate: at least one channel must be confident
        if !has_lexical_confidence && !has_semantic_confidence {
            return vec![];
        }

        // 3. RRF combination
        let combined =
            crate::semantic::rrf_combine(&bm25_ranked, &semantic_scores, self.tools.len(), 60.0);

        // 4. Build results
        combined
            .into_iter()
            .take(max_results)
            .map(|(idx, score)| SearchResult {
                tool: self.tools[idx].clone(),
                score,
                tool_idx: idx,
            })
            .collect()
    }

    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }
}

/// Convenience function that builds a BM25 engine per call.
/// For single searches this is fine; for repeated searches use `SearchIndex`.
pub fn search(tools: &[Tool], query: &str, max_results: usize) -> Vec<SearchResult> {
    if tools.is_empty() {
        return vec![];
    }

    let documents: Vec<Document<usize>> = tools
        .iter()
        .enumerate()
        .map(|(i, t)| Document {
            id: i,
            contents: build_search_text(t),
        })
        .collect();

    let engine = SearchEngineBuilder::<usize>::with_documents(Language::English, documents)
        .b(0.5)
        .build();

    let syn_map = build_synonym_map();
    search_with_engine(tools, &engine, &syn_map, query, max_results)
}

fn search_with_engine(
    tools: &[Tool],
    engine: &SearchEngine<usize>,
    syn_map: &HashMap<&str, &[&str]>,
    query: &str,
    max_results: usize,
) -> Vec<SearchResult> {
    if tools.is_empty() {
        return vec![];
    }

    let query_for_search = preprocess_query(query);
    let query_lower = query.to_lowercase(); // original for exact matching

    let expanded = expand_query(&query_for_search, syn_map);
    // Always fetch enough candidates for re-ranking (category boost, popularity, etc.)
    let bm25_fetch = (max_results * 5).max(100);
    let bm25_results: Vec<BM25Result<usize>> = engine.search(&expanded, bm25_fetch);

    // Fuzzy name matching as fallback (nucleo-matcher)
    let mut fuzzy_matcher = Matcher::new(Config::DEFAULT);
    let mut needle_buf = Vec::new();
    let needle_utf32 = Utf32Str::new(&query_lower, &mut needle_buf);

    let mut scored: Vec<SearchResult> = Vec::new();

    let query_terms: Vec<&str> = query_for_search.split_whitespace().collect();
    // Pre-compute lowercased query terms (avoids per-tool re-lowercasing)
    let query_terms_lower: Vec<String> = query_terms.iter().map(|t| t.to_lowercase()).collect();
    let query_terms_lower_refs: Vec<&str> = query_terms_lower.iter().map(|s| s.as_str()).collect();
    // Collect synonym-expanded terms for coverage gate (so synonym-only matches aren't killed)
    let expanded_terms: Vec<String> = expanded
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let expanded_term_refs: Vec<&str> = expanded_terms.iter().map(|s| s.as_str()).collect();

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

        // Description match bonus — reward when query terms appear at word boundaries
        let desc_lower = tool.desc.to_lowercase();
        let desc_match_count = query_terms_lower_refs
            .iter()
            .filter(|t| t.len() > 2 && word_boundary_match(&desc_lower, t))
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
            let matching_terms = query_terms_lower_refs
                .iter()
                .filter(|t| {
                    t.len() > 2
                        && cat_words
                            .iter()
                            .any(|cw| cw.starts_with(*t) || t.starts_with(cw))
                })
                .count();
            if desc_match_count > 0 {
                matching_terms as f64 * 8.0
            } else {
                matching_terms as f64 * 2.0 // weak bonus if category matches but description doesn't
            }
        };

        // Intent coverage bonus — how well query terms are covered by the tool's metadata
        let (intent_bonus, covered, _) = intent_coverage(&query_terms, tool);

        // Synonym coverage: gate + weak scoring bonus for synonym-only matches
        let (syn_intent_bonus, syn_covered) = if covered == 0 {
            let (sb, sc, _) = intent_coverage(&expanded_term_refs, tool);
            // Half weight for synonym-derived intent bonus
            (sb * 0.5, sc)
        } else {
            (0.0, covered)
        };

        // Lexical score: all relevance signals, excluding popularity
        let lexical_score =
            bm25_score + name_bonus + cat_bonus + desc_bonus + intent_bonus + syn_intent_bonus;

        // Gate: require minimum lexical evidence (exact name/binary matches bypass).
        // Also require at least one query term (or its synonym) to appear in tool metadata —
        // this prevents BM25 false positives from tokenized garbage queries,
        // while allowing synonym-only matches through.
        if name_bonus == 0.0 && (lexical_score < MIN_LEXICAL_THRESHOLD || syn_covered == 0) {
            continue;
        }

        // Popularity as tie-breaker only (max ~8)
        let pop_boost = popularity_boost(tool);

        let final_score = lexical_score + pop_boost;
        scored.push(SearchResult {
            tool: tool.clone(),
            score: final_score,
            tool_idx: idx,
        });
    }

    // Also check fuzzy name matches and tag matches not in BM25 results
    let bm25_indices: std::collections::HashSet<usize> =
        bm25_results.iter().map(|r| r.document.id).collect();

    // For short queries, require higher fuzzy threshold to avoid false positives
    let fuzzy_threshold = if query_lower.len() <= 3 { 80 } else { 50 };
    // High-confidence fuzzy threshold: allows typo matches without anchor
    // (e.g. "ripgrpe" -> "ripgrep", "zoxdie" -> "zoxide")
    let fuzzy_high_confidence = if query_lower.len() <= 3 { 200 } else { 120 };
    // Edit distance is only useful for single-word queries (tool name typos).
    // Multi-word queries like "csv to json" should never trigger edit distance.
    let is_single_word_query = !query_lower.contains(' ');

    let mut hay_buf = Vec::new();
    for (i, tool) in tools.iter().enumerate() {
        if bm25_indices.contains(&i) {
            continue;
        }
        let name_lower = tool.name.to_lowercase();
        hay_buf.clear();
        let fuzzy_score = fuzzy_matcher
            .fuzzy_match(Utf32Str::new(&name_lower, &mut hay_buf), needle_utf32)
            .map(|s| s as i64)
            .unwrap_or(0);

        let bin_score = tool
            .binary
            .as_ref()
            .map(|b| {
                let bl = b.to_lowercase();
                hay_buf.clear();
                fuzzy_matcher
                    .fuzzy_match(Utf32Str::new(&bl, &mut hay_buf), needle_utf32)
                    .map(|s| s as i64)
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        // Edit distance for typo detection (catches transpositions that nucleo misses)
        // Only for single-word queries (tool name typos, not multi-word searches)
        // Early bailouts: skip for short queries, skip when length difference is too large
        let qlen = query_lower.len();
        let max_edit = if !is_single_word_query {
            0
        } else if qlen >= 6 {
            2
        } else if qlen >= 4 {
            1
        } else {
            0
        };
        let is_typo_match = if max_edit > 0 {
            let name_len_diff = qlen.abs_diff(name_lower.len());
            let name_edit_dist = if name_len_diff <= max_edit {
                edit_distance(&query_lower, &name_lower)
            } else {
                usize::MAX
            };
            let bin_edit_dist = tool
                .binary
                .as_ref()
                .map(|b| {
                    let bl = b.to_lowercase();
                    if qlen.abs_diff(bl.len()) <= max_edit {
                        edit_distance(&query_lower, &bl)
                    } else {
                        usize::MAX
                    }
                })
                .unwrap_or(usize::MAX);
            name_edit_dist.min(bin_edit_dist) <= max_edit
        } else {
            false
        };

        // Exact tag match bonus (helps short queries like "rg", "fd")
        let tag_match = tool.tags.iter().any(|t| t == &query_lower);

        // Anchor check: require at least one of these to prevent spurious fuzzy matches
        let has_anchor = tag_match
            || name_lower.contains(&query_lower)
            || query_lower.contains(&name_lower)
            || tool.binary.as_ref().is_some_and(|b| {
                let bl = b.to_lowercase();
                bl.contains(&query_lower) || query_lower.contains(&bl)
            });

        let best_fuzzy = fuzzy_score.max(bin_score);
        // Four paths to inclusion:
        // 1. Edit distance typo match (handles transpositions like "ripgrpe" -> "ripgrep")
        // 2. High-confidence fuzzy match — no anchor needed
        // 3. Normal fuzzy match with anchor (prevents spurious matches)
        // 4. Exact tag match
        if is_typo_match
            || best_fuzzy > fuzzy_high_confidence
            || (best_fuzzy > fuzzy_threshold && has_anchor)
            || tag_match
        {
            let pop_boost = popularity_boost(tool);
            let tag_bonus = if tag_match { 20.0 } else { 0.0 };
            let base_score = if is_typo_match && best_fuzzy == 0 {
                // Pure edit-distance match: score inversely proportional to max allowed distance
                let edit_score = if max_edit <= 1 { 40.0 } else { 25.0 };
                edit_score + pop_boost + tag_bonus
            } else {
                // Discount score slightly for unanchored fuzzy matches
                let fuzzy_weight =
                    if best_fuzzy > fuzzy_high_confidence && !has_anchor && !tag_match {
                        0.2
                    } else {
                        0.3
                    };
                best_fuzzy as f64 * fuzzy_weight + pop_boost + tag_bonus
            };
            scored.push(SearchResult {
                tool: tool.clone(),
                score: base_score,
                tool_idx: i,
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

/// Hybrid search combining BM25 + semantic similarity via RRF.
/// Prefer `SearchIndex::hybrid_search()` which reuses the cached BM25 engine.
#[cfg(feature = "semantic")]
#[deprecated(note = "Use SearchIndex::hybrid_search() for cached BM25 engine")]
pub fn hybrid_search(
    tools: &[Tool],
    query: &str,
    max_results: usize,
    embeddings: &[Vec<f32>],
    query_embedding: &[f32],
) -> Vec<SearchResult> {
    if tools.is_empty() || embeddings.len() != tools.len() {
        return search(tools, query, max_results);
    }

    // 1. BM25 search (already applies lexical threshold gate)
    let bm25_results = search(tools, query, max_results * 3);
    let has_lexical_confidence = !bm25_results.is_empty();
    let bm25_ranked: Vec<(usize, f64)> =
        bm25_results.iter().map(|r| (r.tool_idx, r.score)).collect();

    // 2. Semantic search — cosine similarity, filtered by minimum threshold
    let mut semantic_scores: Vec<(usize, f32)> = embeddings
        .iter()
        .enumerate()
        .map(|(i, emb)| (i, crate::semantic::cosine_similarity(query_embedding, emb)))
        .filter(|(_, sim)| *sim >= MIN_SEMANTIC_SIMILARITY)
        .collect();
    semantic_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let has_semantic_confidence = !semantic_scores.is_empty();
    semantic_scores.truncate(max_results * 3);

    // Gate: at least one channel must be confident
    if !has_lexical_confidence && !has_semantic_confidence {
        return vec![];
    }

    // 3. RRF combination
    let combined = crate::semantic::rrf_combine(&bm25_ranked, &semantic_scores, tools.len(), 60.0);

    // 4. Build results
    combined
        .into_iter()
        .take(max_results)
        .map(|(idx, score)| SearchResult {
            tool: tools[idx].clone(),
            score,
            tool_idx: idx,
        })
        .collect()
}

/// Filter tools by category using hierarchical matching.
/// "File" matches "File Management" (word prefix) but not "Text Filters" (substring).
/// "Utilities" matches "Utilities > General" and "Utilities > Network" (hierarchy prefix).
/// "Docker" matches "Development > Docker" (leaf segment match).
pub fn filter_by_category(tools: &[Tool], category: &str) -> Vec<Tool> {
    let cat_lower = category.to_lowercase();
    let mut filtered: Vec<Tool> = tools
        .iter()
        .filter(|t| {
            let tool_cat = t.category.to_lowercase();
            // Exact match
            tool_cat == cat_lower
                // Hierarchical prefix: "Utilities" matches "Utilities > Network"
                || tool_cat.starts_with(&format!("{} > ", cat_lower))
                // Word-prefix match: "File" matches "File Management" (starts at word boundary)
                || tool_cat.starts_with(&format!("{} ", cat_lower))
                // Leaf segment match: "Docker" matches "Development > Docker"
                || tool_cat.ends_with(&format!(" > {}", cat_lower))
                || tool_cat.ends_with(&format!(" > {} ", cat_lower))
                // Leaf word-prefix: "docker" matches "Development > Docker Tools"
                || tool_cat
                    .rsplit(" > ")
                    .next()
                    .is_some_and(|leaf| leaf.starts_with(&cat_lower) || leaf == cat_lower)
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Links, Tool};
    use std::collections::BTreeMap;

    fn make_tool(
        name: &str,
        binary: Option<&str>,
        desc: &str,
        category: &str,
        tags: &[&str],
        stars: Option<u64>,
    ) -> Tool {
        Tool {
            name: name.to_string(),
            binary: binary.map(|s| s.to_string()),
            desc: desc.to_string(),
            category: category.to_string(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            install: BTreeMap::new(),
            stars,
            brew_installs_365d: None,
            links: Links::default(),
            last_updated: None,
        }
    }

    // =================== edit_distance ===================

    #[test]
    fn edit_distance_identical() {
        assert_eq!(edit_distance("ripgrep", "ripgrep"), 0);
    }

    #[test]
    fn edit_distance_single_substitution() {
        assert_eq!(edit_distance("ripgrep", "ripgrpp"), 1);
    }

    #[test]
    fn edit_distance_transposition() {
        // "ripgrpe" -> "ripgrep" requires 2 ops in classic Levenshtein (no Damerau)
        assert_eq!(edit_distance("ripgrpe", "ripgrep"), 2);
    }

    #[test]
    fn edit_distance_insertion_deletion() {
        assert_eq!(edit_distance("bat", "bats"), 1); // insertion
        assert_eq!(edit_distance("bats", "bat"), 1); // deletion
    }

    #[test]
    fn edit_distance_empty_strings() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("abc", ""), 3);
        assert_eq!(edit_distance("", "xyz"), 3);
    }

    #[test]
    fn edit_distance_completely_different() {
        assert_eq!(edit_distance("abc", "xyz"), 3);
    }

    // =================== word_boundary_match ===================

    #[test]
    fn word_boundary_exact_token() {
        assert!(word_boundary_match("command-line json processor", "json"));
        assert!(word_boundary_match(
            "command-line json processor",
            "processor"
        ));
    }

    #[test]
    fn word_boundary_prefix_match() {
        // "process" is a prefix of "processor"
        assert!(word_boundary_match(
            "command-line json processor",
            "process"
        ));
    }

    #[test]
    fn word_boundary_reverse_prefix() {
        // token "cpu" is prefix of term "cpuinfo"
        assert!(word_boundary_match("cpu monitor", "cpuinfo"));
    }

    #[test]
    fn word_boundary_no_substring_match() {
        // "son" appears as substring in "json" but not at a word boundary
        assert!(!word_boundary_match("command-line json processor", "son"));
    }

    #[test]
    fn word_boundary_hyphenated() {
        // `-` is preserved in tokens, so "pretty-print" is one token
        assert!(word_boundary_match("pretty-print output", "pretty"));
        assert!(word_boundary_match("pretty-print output", "pretty-print"));
        // "print" alone won't match "pretty-print" (not a prefix, not equal)
        assert!(!word_boundary_match("pretty-print output", "print"));
    }

    #[test]
    fn word_boundary_empty_text() {
        assert!(!word_boundary_match("", "test"));
    }

    // =================== intent_coverage ===================

    #[test]
    fn intent_coverage_full_match() {
        let tool = make_tool(
            "jq",
            None,
            "Command-line JSON processor",
            "Data Processing",
            &["json", "filter"],
            Some(30000),
        );
        let terms = vec!["json", "processor"];
        let (bonus, covered, total) = intent_coverage(&terms, &tool);
        assert_eq!(covered, 2);
        assert_eq!(total, 2);
        assert_eq!(bonus, 12.0); // full coverage bonus
    }

    #[test]
    fn intent_coverage_partial_match() {
        let tool = make_tool(
            "jq",
            None,
            "Command-line JSON processor",
            "Data Processing",
            &["json", "filter"],
            Some(30000),
        );
        let terms = vec!["json", "yaml"];
        let (bonus, covered, total) = intent_coverage(&terms, &tool);
        assert_eq!(covered, 1);
        assert_eq!(total, 2);
        assert!(bonus > 0.0 && bonus < 12.0); // partial bonus
    }

    #[test]
    fn intent_coverage_no_match() {
        let tool = make_tool(
            "jq",
            None,
            "Command-line JSON processor",
            "Data Processing",
            &["json", "filter"],
            Some(30000),
        );
        let terms = vec!["video", "stream"];
        let (bonus, covered, _) = intent_coverage(&terms, &tool);
        assert_eq!(covered, 0);
        assert_eq!(bonus, 0.0);
    }

    #[test]
    fn intent_coverage_empty_terms() {
        let tool = make_tool("jq", None, "desc", "cat", &[], Some(1000));
        let terms: Vec<&str> = vec![];
        let (bonus, covered, total) = intent_coverage(&terms, &tool);
        assert_eq!(covered, 0);
        assert_eq!(total, 0);
        assert_eq!(bonus, 0.0);
    }

    #[test]
    fn intent_coverage_short_term_name_match() {
        // Short terms (<=2 chars) should only match name/binary/exact-tag
        let tool = make_tool(
            "fd",
            None,
            "Find files fast",
            "File Management",
            &["find", "fd"],
            Some(34000),
        );
        let terms = vec!["fd"];
        let (_, covered, _) = intent_coverage(&terms, &tool);
        assert_eq!(covered, 1); // matches name
    }

    #[test]
    fn intent_coverage_short_term_no_desc_match() {
        // "fi" should NOT match "find" in description (too short, only name/binary/tag)
        let tool = make_tool(
            "grep",
            None,
            "Find patterns in files",
            "Search",
            &["search"],
            Some(1000),
        );
        let terms = vec!["fi"];
        let (_, covered, _) = intent_coverage(&terms, &tool);
        assert_eq!(covered, 0);
    }

    // =================== popularity_boost ===================

    #[test]
    fn popularity_boost_high_stars() {
        let tool = make_tool("fzf", None, "desc", "cat", &[], Some(66000));
        assert_eq!(popularity_boost(&tool), 8.0);
    }

    #[test]
    fn popularity_boost_medium_stars() {
        let tool = make_tool("t", None, "d", "c", &[], Some(5000));
        let boost = popularity_boost(&tool);
        assert!(boost > 2.0 && boost < 5.0, "5000 stars boost = {}", boost);
    }

    #[test]
    fn popularity_boost_low_stars() {
        let tool = make_tool("t", None, "d", "c", &[], Some(500));
        let boost = popularity_boost(&tool);
        assert!(boost > 0.0 && boost < 2.0, "500 stars boost = {}", boost);
    }

    #[test]
    fn popularity_boost_no_data() {
        let tool = make_tool("t", None, "d", "c", &[], None);
        assert_eq!(popularity_boost(&tool), 0.5);
    }

    #[test]
    fn popularity_boost_brew_fallback() {
        let mut tool = make_tool("t", None, "d", "c", &[], None);
        tool.brew_installs_365d = Some(200000);
        let boost = popularity_boost(&tool);
        assert!(boost > 4.0, "200k brew installs boost = {}", boost);
    }

    #[test]
    fn popularity_boost_stars_over_brew() {
        // Stars take priority over brew installs
        let mut tool = make_tool("t", None, "d", "c", &[], Some(50000));
        tool.brew_installs_365d = Some(1000000);
        assert_eq!(popularity_boost(&tool), 8.0); // uses stars, not brew
    }

    // =================== preprocess_query ===================

    #[test]
    fn preprocess_removes_noise() {
        let result = preprocess_query("show me the best json tool");
        assert!(!result.contains("show"));
        assert!(!result.contains("best"));
        assert!(result.contains("json"));
    }

    #[test]
    fn preprocess_all_noise_keeps_original() {
        let result = preprocess_query("show me the best");
        assert_eq!(result, "show me the best"); // fallback to original
    }

    // =================== expand_query ===================

    #[test]
    fn expand_adds_synonyms() {
        let syn_map = build_synonym_map();
        let expanded = expand_query("grep", &syn_map);
        assert!(expanded.contains("grep"));
        assert!(expanded.contains("search"));
        assert!(expanded.contains("ripgrep"));
    }

    #[test]
    fn expand_original_weighted_higher() {
        let syn_map = build_synonym_map();
        let expanded = expand_query("grep", &syn_map);
        // "grep" should appear twice (2x weight), synonyms once
        let grep_count = expanded.split_whitespace().filter(|w| *w == "grep").count();
        assert_eq!(grep_count, 2);
    }

    #[test]
    fn expand_no_synonyms() {
        let syn_map = build_synonym_map();
        let expanded = expand_query("xyzzy", &syn_map);
        assert_eq!(expanded, "xyzzy xyzzy"); // just doubled, no synonyms
    }

    // =================== filter_by_category ===================

    #[test]
    fn filter_category_exact() {
        let tools = vec![
            make_tool("a", None, "d", "File Management", &[], None),
            make_tool("b", None, "d", "Text Filters", &[], None),
            make_tool("c", None, "d", "File Management", &[], None),
        ];
        let filtered = filter_by_category(&tools, "File Management");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|t| t.category == "File Management"));
    }

    #[test]
    fn filter_category_case_insensitive() {
        let tools = vec![make_tool("a", None, "d", "File Management", &[], None)];
        let filtered = filter_by_category(&tools, "file management");
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_category_hierarchical() {
        let tools = vec![
            make_tool("a", None, "d", "Utilities > General", &[], None),
            make_tool("b", None, "d", "Utilities > Network", &[], None),
            make_tool("c", None, "d", "Development", &[], None),
        ];
        // "Utilities" should match both Utilities subcategories
        let filtered = filter_by_category(&tools, "Utilities");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_category_no_false_positive() {
        let tools = vec![
            make_tool("a", None, "d", "File Management", &[], None),
            make_tool("b", None, "d", "Text Filters", &[], None),
            make_tool("c", None, "d", "Profile Tools", &[], None),
        ];
        // "File" should match "File Management" but NOT "Text Filters" or "Profile Tools"
        let filtered = filter_by_category(&tools, "File");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].category, "File Management");
    }

    #[test]
    fn filter_category_leaf_segment() {
        let tools = vec![
            make_tool("a", None, "d", "Development > Docker", &[], None),
            make_tool("b", None, "d", "Development > Kubernetes", &[], None),
            make_tool("c", None, "d", "Utilities > General", &[], None),
        ];
        // "Docker" should match "Development > Docker" via leaf segment
        let filtered = filter_by_category(&tools, "Docker");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].category, "Development > Docker");
    }
}

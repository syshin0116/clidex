use clidex::model::{Links, Tool};
use clidex::search;
use std::collections::BTreeMap;
use std::time::Instant;

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

/// 20 real CLI tools + adversarial near-miss competitors for ranking tests.
fn sample_tools() -> Vec<Tool> {
    vec![
        // === Core tools ===
        make_tool("jq", None, "Command-line JSON processor", "Data Processing",
            &["json", "filter", "transform", "parse", "query", "pipe", "structured-data"],
            Some(30500)),
        make_tool("miller", Some("mlr"), "Like awk, sed, cut, join, and sort for name-indexed data such as CSV, TSV, and tabular JSON", "Data Processing",
            &["csv", "tsv", "tabular", "data-wrangling", "etl", "column", "json", "convert", "transform"],
            Some(8900)),
        make_tool("csvkit", Some("csvjson"), "Suite of utilities for converting to and working with CSV", "Data Processing",
            &["csv", "json", "convert", "excel", "sql", "tabular", "spreadsheet"],
            Some(6100)),
        make_tool("ripgrep", Some("rg"), "Line-oriented search tool that recursively searches the current directory for a regex pattern", "Search",
            &["search", "grep", "regex", "find-in-files", "code-search", "fast"],
            Some(49000)),
        make_tool("fd", None, "Simple, fast and user-friendly alternative to find", "File Management",
            &["find", "files", "search", "filesystem", "glob", "fast"],
            Some(34000)),
        make_tool("bat", None, "Cat clone with syntax highlighting and Git integration", "File Management",
            &["cat", "file-viewer", "syntax-highlighting", "pager", "pretty-print", "line-numbers", "colorize"],
            Some(50000)),
        make_tool("fzf", None, "General-purpose command-line fuzzy finder", "Search",
            &["fuzzy", "search", "filter", "picker", "interactive", "select", "menu"],
            Some(66000)),
        make_tool("httpie", Some("http"), "User-friendly cURL replacement", "HTTP",
            &["http", "api", "rest", "curl", "request", "client", "web"],
            Some(34000)),
        make_tool("ncdu", None, "NCurses Disk Usage", "System",
            &["disk", "space", "usage", "storage", "cleanup", "size", "du"],
            Some(2300)),
        make_tool("tokei", None, "Count your code, quickly", "Development",
            &["lines-of-code", "loc", "statistics", "count", "sloc", "code"],
            Some(11000)),
        make_tool("hyperfine", None, "Command-line benchmarking tool", "Development",
            &["benchmark", "timing", "performance", "speed", "measure", "time", "profile"],
            Some(22000)),
        make_tool("tldr", None, "Simplified and community-driven man pages", "Help",
            &["help", "documentation", "man", "examples", "cheatsheet", "reference"],
            Some(50000)),
        make_tool("dog", None, "DNS client like dig", "Network",
            &["dns", "lookup", "resolve", "nameserver", "dig", "query"],
            Some(6000)),
        make_tool("dust", None, "More intuitive version of du in rust", "System",
            &["disk", "usage", "size", "du", "treemap", "storage", "space"],
            Some(8500)),
        make_tool("procs", None, "Modern replacement for ps", "System",
            &["process", "ps", "list", "pid", "monitor", "task"],
            Some(5000)),
        make_tool("bottom", Some("btm"), "Cross-platform graphical process/system monitor", "System",
            &["system-monitor", "cpu", "memory", "network", "top", "htop", "dashboard"],
            Some(10000)),
        make_tool("lazygit", None, "Simple terminal UI for git commands", "Git",
            &["git", "tui", "commit", "branch", "diff", "ui", "interactive"],
            Some(53000)),
        make_tool("delta", None, "Viewer for git and diff output", "Git",
            &["git", "diff", "syntax-highlighting", "pager", "side-by-side", "compare"],
            Some(24000)),
        make_tool("eza", None, "Modern replacement for ls", "File Management",
            &["ls", "list", "directory", "files", "tree", "icons", "colors"],
            Some(12000)),
        make_tool("zoxide", None, "Smarter cd command", "Navigation",
            &["cd", "directory", "jump", "autojump", "navigate", "frecent", "z"],
            Some(22000)),

        // === Adversarial near-miss competitors (for ranking precision) ===
        // ag competes with ripgrep for "grep" queries
        make_tool("ag", None, "The Silver Searcher - code searching tool similar to ack", "Search",
            &["search", "grep", "code-search", "ack", "find-in-files"],
            Some(26000)),
        // dasel + yq compete with jq for JSON/YAML queries
        make_tool("dasel", None, "Select, put and delete data from JSON, TOML, YAML, XML and CSV", "Data Processing",
            &["json", "yaml", "toml", "xml", "csv", "query", "select", "jq-alternative"],
            Some(5300)),
        make_tool("yq", None, "YAML processor - jq wrapper for YAML", "Data Processing",
            &["yaml", "json", "query", "jq", "transform", "filter"],
            Some(12000)),
        // gitui competes with lazygit
        make_tool("gitui", None, "Blazing fast terminal-ui for git", "Git",
            &["git", "tui", "commit", "branch", "diff", "ui", "terminal"],
            Some(18000)),
        // difftastic competes with delta for diff
        make_tool("difftastic", Some("difft"), "Structural diff tool that compares files based on their syntax", "Git",
            &["diff", "syntax", "compare", "structural", "git"],
            Some(21000)),
        // lsd competes with eza for ls
        make_tool("lsd", None, "The next gen ls command", "File Management",
            &["ls", "list", "directory", "files", "icons", "colors"],
            Some(13500)),
        // navi competes with tldr for help
        make_tool("navi", None, "Interactive cheatsheet tool for the command-line", "Help",
            &["cheatsheet", "help", "reference", "snippets", "interactive"],
            Some(15000)),
        // curlie competes with httpie
        make_tool("curlie", None, "Power of curl, ease of use of httpie", "HTTP",
            &["http", "api", "curl", "request", "client", "rest"],
            Some(2800)),
    ]
}

// =================== Basic functionality tests ===================

#[test]
fn test_exact_name_match() {
    let tools = sample_tools();
    let results = search::search(&tools, "jq", 10);
    assert!(!results.is_empty(), "jq search returned no results");
    assert_eq!(results[0].tool.name, "jq");
}

#[test]
fn test_binary_name_match() {
    let tools = sample_tools();
    let results = search::search(&tools, "mlr", 10);
    assert!(!results.is_empty(), "mlr search returned no results");
    assert_eq!(results[0].tool.name, "miller");
}

#[test]
fn test_category_filter() {
    let tools = sample_tools();
    let filtered = search::filter_by_category(&tools, "Data Processing");
    assert_eq!(filtered.len(), 5); // jq, miller, csvkit, dasel, yq
    assert_eq!(filtered[0].name, "jq"); // highest stars
}

#[test]
fn test_get_categories() {
    let tools = sample_tools();
    let cats = search::get_categories(&tools);
    assert!(cats.len() >= 5);
}

#[test]
fn test_find_tool() {
    let tools = sample_tools();
    assert!(search::find_tool(&tools, "ripgrep").is_some());
    assert!(search::find_tool(&tools, "rg").is_some()); // binary name
    assert!(search::find_tool(&tools, "nonexistent").is_none());
}

#[test]
fn test_max_results() {
    let tools = sample_tools();
    let results = search::search(&tools, "file search", 3);
    assert!(results.len() <= 3);
}

#[test]
fn test_empty_tools() {
    let results = search::search(&[], "anything", 10);
    assert!(results.is_empty());
}

// =================== Empty result / confidence gate tests ===================

#[test]
fn test_nonsense_query_returns_empty() {
    let tools = sample_tools();

    let r = search::search(&tools, "no-such-tool-name", 10);
    assert!(
        r.is_empty(),
        "nonsense query should return empty, got {:?}",
        r.iter().map(|x| &x.tool.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_random_string_returns_empty() {
    let tools = sample_tools();

    let r = search::search(&tools, "xyzzyplugh42", 10);
    assert!(
        r.is_empty(),
        "random string should return empty, got {:?}",
        r.iter().map(|x| &x.tool.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_gibberish_query_returns_empty() {
    let tools = sample_tools();

    let r = search::search(&tools, "aaabbbccc dddeeefff", 10);
    assert!(
        r.is_empty(),
        "gibberish query should return empty, got {:?}",
        r.iter().map(|x| &x.tool.name).collect::<Vec<_>>()
    );
}

// =================== 30-query accuracy test ===================

/// Helper: check if expected tool is in top N results
fn assert_in_top_n(results: &[search::SearchResult], expected: &str, n: usize, query: &str) {
    let top_names: Vec<&str> = results
        .iter()
        .take(n)
        .map(|r| r.tool.name.as_str())
        .collect();
    assert!(
        top_names.contains(&expected),
        "Query '{}': expected '{}' in top {}, got {:?}",
        query,
        expected,
        n,
        top_names
    );
}

/// Helper: check that tool A ranks above tool B
fn assert_ranks_above(results: &[search::SearchResult], higher: &str, lower: &str, query: &str) {
    let pos_higher = results.iter().position(|r| r.tool.name == higher);
    let pos_lower = results.iter().position(|r| r.tool.name == lower);
    match (pos_higher, pos_lower) {
        (Some(h), Some(l)) => assert!(
            h < l,
            "Query '{}': '{}' (#{}) should rank above '{}' (#{})",
            query,
            higher,
            h + 1,
            lower,
            l + 1
        ),
        (None, _) => panic!(
            "Query '{}': '{}' not found in results {:?}",
            query,
            higher,
            results.iter().map(|r| &r.tool.name).collect::<Vec<_>>()
        ),
        _ => {} // lower not in results is fine
    }
}

#[test]
fn test_easy_queries() {
    let tools = sample_tools();

    // Q1: exact name
    let r = search::search(&tools, "jq", 5);
    assert_in_top_n(&r, "jq", 1, "jq");

    // Q2: description keywords
    let r = search::search(&tools, "json processor", 5);
    assert_in_top_n(&r, "jq", 3, "json processor");

    // Q3: exact name
    let r = search::search(&tools, "ripgrep", 5);
    assert_in_top_n(&r, "ripgrep", 1, "ripgrep");

    // Q4: description keywords
    let r = search::search(&tools, "fuzzy finder", 5);
    assert_in_top_n(&r, "fzf", 3, "fuzzy finder");

    // Q5: description keywords
    let r = search::search(&tools, "git diff viewer", 5);
    assert_in_top_n(&r, "delta", 3, "git diff viewer");

    // Q6: tag + description
    let r = search::search(&tools, "disk usage", 5);
    let names: Vec<&str> = r.iter().take(5).map(|x| x.tool.name.as_str()).collect();
    assert!(
        names.contains(&"ncdu") || names.contains(&"dust"),
        "disk usage: expected ncdu or dust, got {:?}",
        names
    );

    // Q7: exact name
    let r = search::search(&tools, "bat", 5);
    assert_in_top_n(&r, "bat", 1, "bat");

    // Q8: tag match
    let r = search::search(&tools, "dns lookup", 5);
    assert_in_top_n(&r, "dog", 3, "dns lookup");

    // Q9: description + stemming
    let r = search::search(&tools, "benchmark command line", 5);
    assert_in_top_n(&r, "hyperfine", 3, "benchmark command line");

    // Q10: tag match
    let r = search::search(&tools, "process monitor", 5);
    let names: Vec<&str> = r.iter().take(5).map(|x| x.tool.name.as_str()).collect();
    assert!(
        names.contains(&"procs") || names.contains(&"bottom"),
        "process monitor: expected procs or bottom, got {:?}",
        names
    );
}

#[test]
fn test_medium_queries() {
    let tools = sample_tools();

    // Q11: "find files" -> fd (more specific than "search files")
    let r = search::search(&tools, "find files by name", 5);
    assert_in_top_n(&r, "fd", 3, "find files by name");

    // Q12: tag "pretty-print"
    let r = search::search(&tools, "pretty print file", 5);
    assert_in_top_n(&r, "bat", 3, "pretty print file");

    // Q13: tag "lines-of-code" + "count"
    let r = search::search(&tools, "count lines of code", 5);
    assert_in_top_n(&r, "tokei", 3, "count lines of code");

    // Q14: synonym "http" + "request" — both httpie and curlie are valid
    let r = search::search(&tools, "make http requests", 5);
    assert_in_top_n(&r, "httpie", 3, "make http requests");
    assert_in_top_n(&r, "curlie", 3, "make http requests");

    // Q15: synonym "navigate" -> "cd", "directory"
    let r = search::search(&tools, "navigate directories quickly", 5);
    assert_in_top_n(&r, "zoxide", 3, "navigate directories quickly");

    // Q16: description keywords — miller/csvkit should beat dasel
    let r = search::search(&tools, "csv data processing", 5);
    assert_in_top_n(&r, "miller", 3, "csv data processing");

    // Q17: exact + description — tldr should beat navi
    let r = search::search(&tools, "tldr pages", 5);
    assert_in_top_n(&r, "tldr", 1, "tldr pages");

    // Q18: tag "ls" — eza should compete with lsd
    let r = search::search(&tools, "better ls command", 5);
    let names: Vec<&str> = r.iter().take(3).map(|x| x.tool.name.as_str()).collect();
    assert!(
        names.contains(&"eza") || names.contains(&"lsd"),
        "better ls command: expected eza or lsd in top 3, got {:?}",
        names
    );

    // Q19: description keywords — lazygit should beat gitui
    let r = search::search(&tools, "git terminal ui", 5);
    assert_in_top_n(&r, "lazygit", 3, "git terminal ui");
    assert_ranks_above(&r, "lazygit", "gitui", "git terminal ui");

    // Q20: synonym "compare" -> "diff"
    let r = search::search(&tools, "compare text differences", 5);
    assert_in_top_n(&r, "delta", 3, "compare text differences");
}

#[test]
fn test_hard_queries() {
    let tools = sample_tools();

    // Q21: synonym "disk" + tag "space"
    let r = search::search(&tools, "what is eating my disk space", 5);
    let names: Vec<&str> = r.iter().take(5).map(|x| x.tool.name.as_str()).collect();
    assert!(
        names.contains(&"ncdu") || names.contains(&"dust"),
        "disk space: expected ncdu or dust, got {:?}",
        names
    );

    // Q22: synonym "grep" -> "ripgrep" should beat ag
    let r = search::search(&tools, "faster grep", 5);
    assert_in_top_n(&r, "ripgrep", 1, "faster grep");
    assert_ranks_above(&r, "ripgrep", "ag", "faster grep");

    // Q23: synonym "man" -> "tldr" should beat navi
    let r = search::search(&tools, "nicer way to read man pages", 5);
    assert_in_top_n(&r, "tldr", 3, "nicer way to read man pages");
    assert_ranks_above(&r, "tldr", "navi", "nicer way to read man pages");

    // Q24: synonym "profile" -> "benchmark"
    let r = search::search(&tools, "profile how long my script takes", 5);
    assert_in_top_n(&r, "hyperfine", 5, "profile how long my script takes");

    // Q25: synonym "interactive" + "select"
    let r = search::search(&tools, "interactively select from a list", 5);
    assert_in_top_n(&r, "fzf", 5, "interactively select from a list");

    // Q26: synonym "monitor" -> "htop" -> tag match
    let r = search::search(&tools, "something like htop", 5);
    assert_in_top_n(&r, "bottom", 5, "something like htop");

    // Q27: synonym "navigate" + tag "cd" (should be top 1)
    let r = search::search(&tools, "smart cd command", 5);
    assert_in_top_n(&r, "zoxide", 1, "smart cd command");

    // Q28: tag "csv" + synonym "spreadsheet" -> "csv" (top 3 should have relevant tools)
    let r = search::search(&tools, "csv to json", 5);
    let top3: Vec<&str> = r.iter().take(3).map(|x| x.tool.name.as_str()).collect();
    assert!(
        top3.contains(&"miller") || top3.contains(&"csvkit") || top3.contains(&"jq"),
        "csv to json: expected miller/csvkit/jq in top 3, got {:?}",
        top3
    );

    // Q29: tag "shell" + "prompt"
    let tools_with_starship = {
        let mut t = sample_tools();
        t.push(make_tool(
            "starship",
            None,
            "Cross-shell prompt",
            "Shell",
            &[
                "prompt",
                "shell",
                "zsh",
                "bash",
                "fish",
                "theme",
                "powerline",
            ],
            Some(45000),
        ));
        t
    };
    let r = search::search(&tools_with_starship, "customize my shell prompt", 5);
    assert_in_top_n(&r, "starship", 5, "customize my shell prompt");

    // Q30: synonym "cat" -> "bat"
    let r = search::search(&tools, "cat replacement with colors", 5);
    assert_in_top_n(&r, "bat", 5, "cat replacement with colors");
}

// =================== Typo correction tests ===================

#[test]
fn test_typo_queries() {
    let tools = sample_tools();

    // Typo: "ripgrpe" -> should find "ripgrep" in top 3
    let r = search::search(&tools, "ripgrpe", 5);
    assert_in_top_n(&r, "ripgrep", 3, "ripgrpe (typo)");

    // Typo: "zoxdie" -> should find "zoxide" in top 3
    let r = search::search(&tools, "zoxdie", 5);
    assert_in_top_n(&r, "zoxide", 3, "zoxdie (typo)");

    // Typo: "lazigit" -> should find "lazygit" in top 3
    let r = search::search(&tools, "lazigit", 5);
    assert_in_top_n(&r, "lazygit", 3, "lazigit (typo)");
}

// =================== Synonym-only match tests ===================

#[test]
fn test_synonym_only_queries() {
    let tools = sample_tools();

    // "manual pages" -> tldr in top 3
    let r = search::search(&tools, "manual pages", 5);
    assert_in_top_n(&r, "tldr", 3, "manual pages (synonym-only)");

    // "grep files" -> ripgrep in top 3, should beat ag
    let r = search::search(&tools, "grep files", 5);
    assert_in_top_n(&r, "ripgrep", 3, "grep files (synonym-only)");

    // "navigate quickly" -> zoxide in top 3
    let r = search::search(&tools, "navigate quickly", 5);
    assert_in_top_n(&r, "zoxide", 3, "navigate quickly (synonym-only)");
}

// =================== Performance benchmark ===================

#[test]
fn test_search_performance() {
    let tools = sample_tools();

    // Warm up
    let _ = search::search(&tools, "json", 10);

    let queries = [
        "jq",
        "csv to json",
        "search files by name",
        "pretty print file with line numbers",
        "navigate directories quickly",
        "process monitor",
        "benchmark command line tool",
        "interactively select from a list",
        "faster grep",
        "disk usage analyzer",
    ];

    let start = Instant::now();
    let iterations = 100;
    for _ in 0..iterations {
        for q in &queries {
            let _ = search::search(&tools, q, 10);
        }
    }
    let elapsed = start.elapsed();
    let per_query = elapsed / (iterations * queries.len() as u32);

    eprintln!(
        "Performance: {} queries x {} iterations = {:?} total, {:?} per query",
        queries.len(),
        iterations,
        elapsed,
        per_query
    );

    // Target: < 50ms per query (should be well under with 20 tools)
    assert!(
        per_query.as_millis() < 50,
        "Search too slow: {:?} per query (target < 50ms)",
        per_query
    );
}

// =================== Recall summary test ===================

#[test]
fn test_recall_summary() {
    let tools = {
        let mut t = sample_tools();
        t.push(make_tool(
            "starship",
            None,
            "Cross-shell prompt",
            "Shell",
            &[
                "prompt",
                "shell",
                "zsh",
                "bash",
                "fish",
                "theme",
                "powerline",
            ],
            Some(45000),
        ));
        t
    };

    let test_cases: Vec<(&str, &str, usize)> = vec![
        // (query, expected_tool, top_n)
        // --- Exact / basic ---
        ("jq", "jq", 1),
        ("ripgrep", "ripgrep", 1),
        ("bat", "bat", 1),
        ("json processor", "jq", 3),
        ("fuzzy finder", "fzf", 3),
        ("git diff viewer", "delta", 3),
        ("dns lookup", "dog", 3),
        ("benchmark command line", "hyperfine", 3),
        ("tldr pages", "tldr", 1),
        // --- Medium (tighter than before) ---
        ("find files by name", "fd", 3),
        ("pretty print file", "bat", 3),
        ("count lines of code", "tokei", 3),
        ("make http requests", "httpie", 3),
        ("navigate directories quickly", "zoxide", 3),
        ("csv data processing", "miller", 3),
        ("git terminal ui", "lazygit", 3),
        ("compare text differences", "delta", 3),
        // --- Hard ---
        ("faster grep", "ripgrep", 1),
        ("nicer way to read man pages", "tldr", 3),
        ("profile how long my script takes", "hyperfine", 5),
        ("interactively select from a list", "fzf", 5),
        ("something like htop", "bottom", 5),
        ("smart cd command", "zoxide", 1),
        ("csv to json", "miller", 3),
        ("customize my shell prompt", "starship", 5),
        ("cat replacement with colors", "bat", 5),
        ("disk usage", "ncdu", 5),
        ("process monitor", "procs", 5),
        // --- Typo (top 3) ---
        ("ripgrpe", "ripgrep", 3),
        ("zoxdie", "zoxide", 3),
        ("lazigit", "lazygit", 3),
        // --- Synonym-only (top 3) ---
        ("manual pages", "tldr", 3),
        ("grep files", "ripgrep", 3),
        ("navigate quickly", "zoxide", 3),
    ];

    let mut pass = 0;
    let mut fail = 0;
    let mut failures = Vec::new();

    for (query, expected, top_n) in &test_cases {
        let results = search::search(&tools, query, *top_n);
        let top_names: Vec<&str> = results.iter().map(|r| r.tool.name.as_str()).collect();
        if top_names.contains(expected) {
            pass += 1;
        } else {
            fail += 1;
            failures.push(format!(
                "  FAIL: '{}' -> expected '{}' in top {}, got {:?}",
                query, expected, top_n, top_names
            ));
        }
    }

    let total = pass + fail;
    let recall = pass as f64 / total as f64 * 100.0;

    eprintln!("\n=== Search Recall Summary ===");
    eprintln!("Pass: {}/{} ({:.1}%)", pass, total, recall);
    if !failures.is_empty() {
        eprintln!("Failures:");
        for f in &failures {
            eprintln!("{}", f);
        }
    }

    // Target: >= 95% recall (stricter now with adversarial fixtures)
    assert!(
        recall >= 95.0,
        "Recall too low: {:.1}% (target >= 95%)",
        recall
    );
}

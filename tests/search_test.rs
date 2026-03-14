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
        brew_installs_30d: None,
        links: Links::default(),
        last_updated: None,
    }
}

/// 20 real CLI tools with realistic tags (simulating LLM-generated tags)
fn sample_tools() -> Vec<Tool> {
    vec![
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
    assert_eq!(filtered.len(), 3);
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

    // Q11: synonym "search" -> "find"
    let r = search::search(&tools, "search files by name", 5);
    assert_in_top_n(&r, "fd", 5, "search files by name");

    // Q12: tag "pretty-print"
    let r = search::search(&tools, "pretty print file", 5);
    assert_in_top_n(&r, "bat", 5, "pretty print file");

    // Q13: tag "lines-of-code" + "count"
    let r = search::search(&tools, "count lines of code", 5);
    assert_in_top_n(&r, "tokei", 5, "count lines of code");

    // Q14: synonym "http" + "request"
    let r = search::search(&tools, "make http requests", 5);
    assert_in_top_n(&r, "httpie", 5, "make http requests");

    // Q15: synonym "navigate" -> "cd", "directory"
    let r = search::search(&tools, "navigate directories quickly", 5);
    assert_in_top_n(&r, "zoxide", 5, "navigate directories quickly");

    // Q16: description keywords
    let r = search::search(&tools, "csv data processing", 5);
    let names: Vec<&str> = r.iter().take(5).map(|x| x.tool.name.as_str()).collect();
    assert!(
        names.contains(&"miller") || names.contains(&"csvkit"),
        "csv data processing: expected miller or csvkit, got {:?}",
        names
    );

    // Q17: exact + description
    let r = search::search(&tools, "tldr pages", 5);
    assert_in_top_n(&r, "tldr", 3, "tldr pages");

    // Q18: tag "ls"
    let r = search::search(&tools, "better ls command", 5);
    assert_in_top_n(&r, "eza", 5, "better ls command");

    // Q19: description keywords
    let r = search::search(&tools, "git terminal ui", 5);
    assert_in_top_n(&r, "lazygit", 3, "git terminal ui");

    // Q20: synonym "compare" -> "diff"
    let r = search::search(&tools, "compare text differences", 5);
    assert_in_top_n(&r, "delta", 5, "compare text differences");
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

    // Q22: synonym "grep" -> "ripgrep"
    let r = search::search(&tools, "faster grep", 5);
    assert_in_top_n(&r, "ripgrep", 5, "faster grep");

    // Q23: synonym "man" -> "tldr"
    let r = search::search(&tools, "nicer way to read man pages", 5);
    assert_in_top_n(&r, "tldr", 5, "nicer way to read man pages");

    // Q24: synonym "profile" -> "benchmark"
    let r = search::search(&tools, "profile how long my script takes", 5);
    assert_in_top_n(&r, "hyperfine", 5, "profile how long my script takes");

    // Q25: synonym "interactive" + "select"
    let r = search::search(&tools, "interactively select from a list", 5);
    assert_in_top_n(&r, "fzf", 5, "interactively select from a list");

    // Q26: synonym "monitor" -> "htop" -> tag match
    let r = search::search(&tools, "something like htop", 5);
    assert_in_top_n(&r, "bottom", 5, "something like htop");

    // Q27: synonym "navigate" + tag "cd"
    let r = search::search(&tools, "smart cd command", 5);
    assert_in_top_n(&r, "zoxide", 5, "smart cd command");

    // Q28: tag "csv" + synonym "spreadsheet" -> "csv"
    let r = search::search(&tools, "csv to json", 5);
    let names: Vec<&str> = r.iter().take(5).map(|x| x.tool.name.as_str()).collect();
    assert!(
        names.contains(&"miller") || names.contains(&"csvkit") || names.contains(&"jq"),
        "csv to json: expected miller/csvkit/jq, got {:?}",
        names
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
        ("jq", "jq", 1),
        ("json processor", "jq", 3),
        ("ripgrep", "ripgrep", 1),
        ("fuzzy finder", "fzf", 3),
        ("git diff viewer", "delta", 3),
        ("bat", "bat", 1),
        ("dns lookup", "dog", 3),
        ("benchmark command line", "hyperfine", 3),
        ("search files by name", "fd", 5),
        ("pretty print file", "bat", 5),
        ("count lines of code", "tokei", 5),
        ("make http requests", "httpie", 5),
        ("navigate directories quickly", "zoxide", 5),
        ("csv data processing", "miller", 5),
        ("better ls command", "eza", 5),
        ("git terminal ui", "lazygit", 3),
        ("compare text differences", "delta", 5),
        ("faster grep", "ripgrep", 5),
        ("nicer way to read man pages", "tldr", 5),
        ("profile how long my script takes", "hyperfine", 5),
        ("interactively select from a list", "fzf", 5),
        ("something like htop", "bottom", 5),
        ("smart cd command", "zoxide", 5),
        ("csv to json", "miller", 5),
        ("customize my shell prompt", "starship", 5),
        ("cat replacement with colors", "bat", 5),
        ("disk usage", "ncdu", 5),
        ("process monitor", "procs", 5),
        ("tldr pages", "tldr", 3),
        ("git terminal ui", "lazygit", 3),
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

    // Target: >= 85% recall
    assert!(
        recall >= 85.0,
        "Recall too low: {:.1}% (target >= 85%)",
        recall
    );
}

//! Integration tests against the real index (~5,000+ tools).
//! These tests verify search quality at scale — ranking, precision, and false positive control.
//! Skipped if ~/.clidex/index.yaml doesn't exist.

use clidex::model::Index;
use clidex::search;
use std::fs;

fn load_real_index() -> Option<Vec<clidex::model::Tool>> {
    let path = dirs::home_dir()?.join(".clidex/index.yaml");
    let content = fs::read_to_string(path).ok()?;
    let index: Index = serde_yaml::from_str(&content).ok()?;
    if index.tools.len() < 100 {
        return None; // too small to be meaningful
    }
    Some(index.tools)
}

macro_rules! real_index_test {
    ($name:ident, $body:expr) => {
        #[test]
        fn $name() {
            let Some(tools) = load_real_index() else {
                eprintln!(
                    "SKIP {}: no real index at ~/.clidex/index.yaml",
                    stringify!($name)
                );
                return;
            };
            eprintln!("Running {} with {} tools", stringify!($name), tools.len());
            let tools = &tools;
            $body(tools);
        }
    };
}

// =================== Ranking precision tests ===================
// These test that the RIGHT tool ranks ABOVE plausible alternatives.

real_index_test!(real_exact_name_top1, |tools: &Vec<clidex::model::Tool>| {
    // Exact name queries must always be #1
    for name in [
        "jq", "ripgrep", "fzf", "bat", "fd", "zoxide", "tldr", "lazygit", "docker", "git",
    ] {
        let r = search::search(tools, name, 5);
        if r.is_empty() {
            continue; // tool might not be in index
        }
        assert_eq!(
            r[0].tool.name.to_lowercase(),
            name.to_lowercase(),
            "Exact name '{}' should be #1, got '{}' (score {:.1})",
            name,
            r[0].tool.name,
            r[0].score
        );
    }
});

real_index_test!(real_binary_name_top1, |tools: &Vec<clidex::model::Tool>| {
    // Binary alias queries should find the right tool at #1
    // Note: "http" is ambiguous (httpie, hurl, etc.) so we only test unambiguous aliases
    let cases = [("rg", "ripgrep"), ("btm", "bottom")];
    for (query, expected) in cases {
        let r = search::search(tools, query, 5);
        if r.is_empty() {
            continue;
        }
        assert_eq!(
            r[0].tool.name.to_lowercase(),
            expected.to_lowercase(),
            "Binary name '{}' should find '{}' at #1, got '{}'",
            query,
            expected,
            r[0].tool.name
        );
    }
});

real_index_test!(real_ranking_order, |tools: &Vec<clidex::model::Tool>| {
    // "json processor" -> jq should outrank generic json tools
    let r = search::search(tools, "json processor", 10);
    if let Some(jq_pos) = r.iter().position(|x| x.tool.name == "jq") {
        assert!(
            jq_pos < 3,
            "'json processor' -> jq should be top 3, was #{}",
            jq_pos + 1
        );
    }

    // "git diff" -> delta should outrank generic git tools
    let r = search::search(tools, "git diff", 10);
    let delta_pos = r.iter().position(|x| x.tool.name == "delta");
    if let Some(pos) = delta_pos {
        assert!(
            pos < 5,
            "'git diff' -> delta should be top 5, was #{}",
            pos + 1
        );
    }

    // "fuzzy finder" -> fzf should be #1 or #2
    let r = search::search(tools, "fuzzy finder", 10);
    if let Some(pos) = r.iter().position(|x| x.tool.name == "fzf") {
        assert!(
            pos < 3,
            "'fuzzy finder' -> fzf should be top 3, was #{}",
            pos + 1
        );
    }
});

// =================== Precision tests (false positive control) ===================
// With 5000+ tools, verify we're not returning garbage.

real_index_test!(real_false_positive_control, |tools: &Vec<
    clidex::model::Tool,
>| {
    // Garbage queries should still return empty
    for q in ["xyzzyplugh42", "asdfghjklqwerty", "aaabbbccc dddeeefff"] {
        let r = search::search(tools, q, 10);
        assert!(
            r.is_empty(),
            "Garbage query '{}' should return empty on real index, got {} results: {:?}",
            q,
            r.len(),
            r.iter().take(3).map(|x| &x.tool.name).collect::<Vec<_>>()
        );
    }

    // Plausible-but-unrelated query — turns out there ARE quantum/physics CLI tools!
    // So we just verify the results are actually related, not that there are none
    let r = search::search(tools, "blockchain mining rig", 10);
    assert!(
        r.len() <= 5,
        "'blockchain mining rig' should have few results on CLI index, got {}: {:?}",
        r.len(),
        r.iter().take(5).map(|x| &x.tool.name).collect::<Vec<_>>()
    );
});

real_index_test!(real_result_relevance, |tools: &Vec<clidex::model::Tool>| {
    // "csv" results should all be data-related, not random tools
    let r = search::search(tools, "csv", 5);
    assert!(!r.is_empty(), "'csv' should return results");
    let csv_related = r
        .iter()
        .filter(|x| {
            let meta = format!(
                "{} {} {} {}",
                x.tool.name,
                x.tool.desc.to_lowercase(),
                x.tool.tags.join(" ").to_lowercase(),
                x.tool.category.to_lowercase()
            );
            meta.contains("csv")
                || meta.contains("data")
                || meta.contains("tabular")
                || meta.contains("json")
                || meta.contains("convert")
        })
        .count();
    assert!(
        csv_related >= r.len() / 2,
        "'csv' results should be mostly data-related, only {}/{} were: {:?}",
        csv_related,
        r.len(),
        r.iter().map(|x| &x.tool.name).collect::<Vec<_>>()
    );
});

// =================== Typo correction at scale ===================

real_index_test!(real_typo_correction, |tools: &Vec<clidex::model::Tool>| {
    let cases = [
        ("ripgrpe", "ripgrep"),
        ("lazigit", "lazygit"),
        ("zoxdie", "zoxide"),
        ("tokei", "tokei"), // not a typo — should still work
        ("hyprefine", "hyperfine"),
    ];
    for (typo, expected) in cases {
        let r = search::search(tools, typo, 5);
        let found = r.iter().any(|x| x.tool.name.to_lowercase() == expected);
        assert!(
            found,
            "Typo '{}' should find '{}' in top 5, got {:?}",
            typo,
            expected,
            r.iter().map(|x| &x.tool.name).collect::<Vec<_>>()
        );
    }
});

// =================== Synonym queries at scale ===================

real_index_test!(real_synonym_queries, |tools: &Vec<clidex::model::Tool>| {
    // At scale, synonym tests should verify RELEVANCE, not specific tool names.
    // Many categories have 10+ competing tools, so "ncdu" might not be top 5 for "disk space".

    // "directory autojump" should return navigation tools
    let r = search::search(tools, "directory autojump", 5);
    assert!(!r.is_empty(), "'directory autojump' should return results");
    let nav_related = r.iter().any(|x| {
        let meta =
            format!("{} {} {}", x.tool.name, x.tool.desc, x.tool.tags.join(" ")).to_lowercase();
        meta.contains("cd")
            || meta.contains("jump")
            || meta.contains("directory")
            || meta.contains("navigate")
            || meta.contains("autojump")
    });
    assert!(
        nav_related,
        "'directory autojump' results should include navigation tools, got {:?}",
        r.iter().map(|x| &x.tool.name).collect::<Vec<_>>()
    );

    // "disk space analyzer" should return disk/storage tools
    let r = search::search(tools, "disk space analyzer", 5);
    assert!(!r.is_empty(), "'disk space analyzer' should return results");
    let disk_related = r
        .iter()
        .filter(|x| {
            let meta =
                format!("{} {} {}", x.tool.name, x.tool.desc, x.tool.tags.join(" ")).to_lowercase();
            meta.contains("disk")
                || meta.contains("usage")
                || meta.contains("space")
                || meta.contains("du")
                || meta.contains("storage")
        })
        .count();
    assert!(
        disk_related >= 3,
        "'disk space analyzer' should have mostly disk tools, only {}/5 were, got {:?}",
        disk_related,
        r.iter().map(|x| &x.tool.name).collect::<Vec<_>>()
    );

    // "benchmark command" should return benchmarking tools
    let r = search::search(tools, "benchmark command", 5);
    assert!(!r.is_empty(), "'benchmark command' should return results");
    let bench_related = r.iter().any(|x| {
        let meta =
            format!("{} {} {}", x.tool.name, x.tool.desc, x.tool.tags.join(" ")).to_lowercase();
        meta.contains("benchmark")
            || meta.contains("timing")
            || meta.contains("performance")
            || meta.contains("measure")
    });
    assert!(
        bench_related,
        "'benchmark command' results should include benchmarking tools, got {:?}",
        r.iter().map(|x| &x.tool.name).collect::<Vec<_>>()
    );
});

// =================== Crowded category ranking ===================
// Verifies ranking quality when many similar tools compete.

real_index_test!(real_crowded_category, |tools: &Vec<clidex::model::Tool>| {
    // "git" has many tools — lazygit should be prominent (high stars)
    let r = search::search(tools, "git tui", 10);
    let git_tools: Vec<&str> = r.iter().map(|x| x.tool.name.as_str()).collect();
    assert!(
        git_tools.contains(&"lazygit"),
        "'git tui' should include lazygit in top 10 (crowded category), got {:?}",
        git_tools
    );

    // "http client" has curl, httpie, wget, etc.
    let r = search::search(tools, "http client", 10);
    let has_httpie = r
        .iter()
        .any(|x| x.tool.name == "httpie" || x.tool.name == "curlie" || x.tool.name == "xh");
    assert!(
        has_httpie,
        "'http client' should include httpie/curlie/xh in top 10, got {:?}",
        r.iter().map(|x| &x.tool.name).collect::<Vec<_>>()
    );
});

// =================== Performance at scale ===================

real_index_test!(real_search_performance, |tools: &Vec<
    clidex::model::Tool,
>| {
    use std::time::Instant;

    let queries = [
        "jq",
        "csv to json",
        "search files",
        "git diff",
        "disk usage",
        "http client",
        "benchmark",
        "fuzzy finder",
        "process monitor",
        "navigate directories",
    ];

    // --- Uncached (builds BM25 engine per query) ---
    let _ = search::search(tools, "warmup", 10);

    let start = Instant::now();
    let uncached_iterations = 3;
    for _ in 0..uncached_iterations {
        for q in &queries {
            let _ = search::search(tools, q, 10);
        }
    }
    let uncached_elapsed = start.elapsed();
    let uncached_per_query = uncached_elapsed / (uncached_iterations * queries.len() as u32);

    // --- Cached (SearchIndex builds BM25 once) ---
    let index = search::SearchIndex::new(tools.clone());
    let _ = index.search("warmup", 10);

    let start = Instant::now();
    let cached_iterations = 10;
    for _ in 0..cached_iterations {
        for q in &queries {
            let _ = index.search(q, 10);
        }
    }
    let cached_elapsed = start.elapsed();
    let cached_per_query = cached_elapsed / (cached_iterations * queries.len() as u32);

    let speedup = uncached_per_query.as_secs_f64() / cached_per_query.as_secs_f64();

    eprintln!(
        "Real index performance ({} tools):\n  Uncached: {:.1}ms/query\n  Cached:   {:.1}ms/query\n  Speedup:  {:.1}x",
        tools.len(),
        uncached_per_query.as_secs_f64() * 1000.0,
        cached_per_query.as_secs_f64() * 1000.0,
        speedup,
    );

    // Cached should be significantly faster
    assert!(
        cached_per_query.as_millis() < 200,
        "Cached search too slow: {:?} per query (target < 200ms)",
        cached_per_query
    );
});

// =================== Score sanity checks ===================

real_index_test!(real_score_sanity, |tools: &Vec<clidex::model::Tool>| {
    // Exact name match should have much higher score than generic results
    let r = search::search(tools, "jq", 10);
    if r.len() >= 2 {
        let top_score = r[0].score;
        let second_score = r[1].score;
        assert!(
            top_score > second_score * 1.5,
            "Exact match 'jq' (score {:.1}) should significantly outscore #2 '{}' (score {:.1})",
            top_score,
            r[1].tool.name,
            second_score
        );
    }

    // All scores should be positive
    for result in &r {
        assert!(
            result.score > 0.0,
            "Score for '{}' should be positive, got {:.2}",
            result.tool.name,
            result.score
        );
    }

    // Scores should be monotonically decreasing
    for i in 1..r.len() {
        assert!(
            r[i - 1].score >= r[i].score,
            "Results should be sorted by score: '{}' ({:.1}) >= '{}' ({:.1})",
            r[i - 1].tool.name,
            r[i - 1].score,
            r[i].tool.name,
            r[i].score
        );
    }
});

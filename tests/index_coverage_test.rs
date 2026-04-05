//! Index coverage tests — verify that the index contains must-have CLI tools
//! and has sufficient category breadth.
//! Skipped if ~/.clidex/index.yaml doesn't exist.

use clidex::model::Index;
use std::collections::{HashMap, HashSet};
use std::fs;

fn load_real_index() -> Option<Vec<clidex::model::Tool>> {
    let path = dirs::home_dir()?.join(".clidex/index.yaml");
    let content = fs::read_to_string(path).ok()?;
    let index: Index = serde_yaml::from_str(&content).ok()?;
    if index.tools.len() < 100 {
        return None;
    }
    Some(index.tools)
}

/// Must-have tools organized by functional category.
/// If any of these are missing (and not in KNOWN_MISSING), the test fails.
const MUST_HAVE_TOOLS: &[(&str, &[&str])] = &[
    // Data processing
    (
        "Data Processing",
        &["jq", "yq", "dasel", "miller", "csvkit"],
    ),
    // Search
    ("Search", &["ripgrep", "fd", "fzf", "ag"]),
    // File management
    ("File Management", &["bat", "eza", "tree"]),
    // Navigation
    ("Navigation", &["zoxide"]),
    // Git
    ("Git", &["lazygit", "delta", "git"]),
    // HTTP
    ("HTTP", &["httpie", "curl", "wget", "xh", "hurl"]),
    // Help / Documentation
    ("Help", &["tldr", "navi"]),
    // System
    (
        "System",
        &["ncdu", "dust", "procs", "bottom", "htop", "glances"],
    ),
    // Development
    ("Development", &["tokei", "hyperfine"]),
    // Shell
    ("Shell", &["starship", "tmux", "zellij", "nushell"]),
    // Editor
    ("Editor", &["neovim"]),
    // Containers
    ("Containers", &["docker", "dive", "lazydocker", "k9s"]),
    // Network
    ("Network", &["dog", "mtr"]),
    // Security
    ("Security", &["trivy", "grype", "nmap"]),
    // Media
    ("Media", &["ffmpeg", "yt-dlp"]),
    // Document
    ("Document", &["pandoc", "glow"]),
    // Python ecosystem
    ("Python", &["poetry", "ruff", "black", "pgcli"]),
    // Cloud
    ("Cloud", &["awscli", "terraform"]),
    // Benchmarking / Load testing
    ("Load Testing", &["wrk", "hey", "k6", "vegeta"]),
    // Terminal UI
    ("TUI", &["gum", "fx"]),
    // Modern Unix
    (
        "Modern Unix",
        &["atuin", "ouch", "just", "broot", "yazi", "gping"],
    ),
];

/// Tools in MUST_HAVE_TOOLS that we know are currently not in the index.
/// Track these explicitly — remove from here when they get added to the pipeline.
const KNOWN_MISSING: &[&str] = &[
    // "dog" — DNS client, project archived, not in major package managers
    "dog",
];

/// Ecosystem-specific representative tools.
/// Verifies each install method has at least one representative present.
const ECOSYSTEM_REPRESENTATIVES: &[(&str, &[&str])] = &[
    // brew-only: tools typically installed only via brew
    ("brew-only", &["watch", "tree", "wget"]),
    // cargo-only: Rust tools primarily installed via cargo
    ("cargo-only", &["tokei", "hyperfine", "dust"]),
    // npm-only: Node tools primarily installed via npm
    ("npm-only", &["tldr"]),
];

#[test]
fn test_must_have_tools_present() {
    let Some(tools) = load_real_index() else {
        eprintln!("SKIP: no real index at ~/.clidex/index.yaml");
        return;
    };

    let tool_names: HashSet<String> = tools
        .iter()
        .flat_map(|t| {
            let mut names = vec![t.name.to_lowercase()];
            if let Some(ref b) = t.binary {
                names.push(b.to_lowercase());
            }
            names
        })
        .collect();

    // Collect all must-have tool names for KNOWN_MISSING validation
    let all_must_have: HashSet<&str> = MUST_HAVE_TOOLS
        .iter()
        .flat_map(|(_, names)| names.iter().copied())
        .collect();

    let mut missing = Vec::new();
    let mut known_missing_found = 0;
    let mut total = 0;

    for (category, names) in MUST_HAVE_TOOLS {
        for name in *names {
            total += 1;
            let lower = name.to_lowercase();
            if !tool_names.contains(&lower) {
                if KNOWN_MISSING.contains(name) {
                    known_missing_found += 1;
                    eprintln!("  KNOWN MISSING: {} ({})", name, category);
                } else {
                    missing.push(format!("{} ({})", name, category));
                }
            }
        }
    }

    // Validate KNOWN_MISSING entries are actually in MUST_HAVE_TOOLS
    for km in KNOWN_MISSING {
        assert!(
            all_must_have.contains(km),
            "KNOWN_MISSING entry '{}' is not in MUST_HAVE_TOOLS — remove it or add it to must-have list", km,
        );
    }

    let found = total - missing.len() - known_missing_found;

    eprintln!("\n=== Index Coverage ===");
    eprintln!("Total must-have: {}", total);
    eprintln!("Found:           {}", found);
    eprintln!(
        "Known missing:   {} ({})",
        known_missing_found,
        KNOWN_MISSING.join(", ")
    );
    eprintln!("New missing:     {}", missing.len());

    if !missing.is_empty() {
        eprintln!("\nNEW MISSING TOOLS (should be investigated):");
        for m in &missing {
            eprintln!("  - {}", m);
        }
    }

    assert!(
        missing.is_empty(),
        "Must-have tools missing from index: {:?}",
        missing
    );
}

#[test]
fn test_category_breadth() {
    let Some(tools) = load_real_index() else {
        eprintln!("SKIP: no real index at ~/.clidex/index.yaml");
        return;
    };

    // Count tools per top-level category (before ">")
    let mut category_counts: HashMap<String, usize> = HashMap::new();
    for tool in &tools {
        let top_cat = tool
            .category
            .split('>')
            .next()
            .unwrap_or(&tool.category)
            .trim()
            .to_string();
        *category_counts.entry(top_cat).or_insert(0) += 1;
    }

    // Count tools per full sub-category
    let mut subcategory_counts: HashMap<String, usize> = HashMap::new();
    for tool in &tools {
        *subcategory_counts.entry(tool.category.clone()).or_insert(0) += 1;
    }

    let total_categories = category_counts.len();
    let total_subcategories = subcategory_counts.len();

    eprintln!("\n=== Category Breadth ===");
    eprintln!("Top-level categories: {}", total_categories);
    eprintln!("Sub-categories:       {}", total_subcategories);
    eprintln!("Total tools:          {}", tools.len());

    // Structural minimums
    assert!(
        total_categories >= 10,
        "Expected at least 10 top-level categories, got {}",
        total_categories
    );
    assert!(
        total_subcategories >= 50,
        "Expected at least 50 sub-categories, got {}",
        total_subcategories
    );
    assert!(
        tools.len() >= 4000,
        "Expected at least 4000 tools in index, got {}",
        tools.len()
    );

    // Key categories that must have substantial representation.
    // These cover the most important functional areas for CLI tool discovery.
    let expected_categories: &[(&str, usize)] = &[
        ("Utilities", 100),
        ("Development", 50),
        ("Security", 20),
        ("Version Control", 10),
        ("Data Manipulation", 10),
    ];

    let mut cat_results = Vec::new();
    for &(cat, min_count) in expected_categories {
        let count = category_counts.get(cat).copied().unwrap_or(0);
        let status = if count >= min_count { "OK" } else { "FAIL" };
        cat_results.push((cat, count, min_count, status));
    }

    eprintln!("\nCategory minimums:");
    for (cat, count, min, status) in &cat_results {
        eprintln!("  {:25} {:>5} / {:>5}  {}", cat, count, min, status);
    }

    for (cat, count, min_count, _) in &cat_results {
        assert!(
            count >= min_count,
            "Category '{}' should have at least {} tools, got {}",
            cat,
            min_count,
            count
        );
    }

    // Also verify that no single category dominates excessively (> 60%)
    let max_cat = category_counts.values().max().copied().unwrap_or(0);
    let max_ratio = max_cat as f64 / tools.len() as f64;
    assert!(
        max_ratio < 0.65,
        "Single category has {:.0}% of all tools ({}/{}), index is too skewed",
        max_ratio * 100.0,
        max_cat,
        tools.len()
    );
}

#[test]
fn test_ecosystem_presence() {
    let Some(tools) = load_real_index() else {
        eprintln!("SKIP: no real index at ~/.clidex/index.yaml");
        return;
    };

    let tool_names: HashSet<String> = tools.iter().map(|t| t.name.to_lowercase()).collect();

    let with_brew = tools
        .iter()
        .filter(|t| t.install.contains_key("brew"))
        .count();
    let with_cargo = tools
        .iter()
        .filter(|t| t.install.contains_key("cargo"))
        .count();
    let with_npm = tools
        .iter()
        .filter(|t| t.install.contains_key("npm"))
        .count();
    let with_stars = tools.iter().filter(|t| t.stars.is_some()).count();

    eprintln!("\n=== Ecosystem Presence ===");
    eprintln!("Brew:  {} tools", with_brew);
    eprintln!("Cargo: {} tools", with_cargo);
    eprintln!("npm:   {} tools", with_npm);
    eprintln!("Stars: {} tools", with_stars);
    eprintln!("Total: {} tools", tools.len());

    // Install method minimums
    assert!(
        with_brew >= 500,
        "Expected >= 500 brew tools, got {}",
        with_brew
    );
    assert!(
        with_cargo >= 30,
        "Expected >= 30 cargo tools, got {}",
        with_cargo
    );
    assert!(with_npm >= 10, "Expected >= 10 npm tools, got {}", with_npm);
    assert!(
        with_stars >= 1000,
        "Expected >= 1000 tools with stars, got {}",
        with_stars
    );

    // Ecosystem representatives: at least one tool from each group should be present
    eprintln!("\nEcosystem representatives:");
    for (ecosystem, reps) in ECOSYSTEM_REPRESENTATIVES {
        let present: Vec<&&str> = reps
            .iter()
            .filter(|r| tool_names.contains(&r.to_lowercase()))
            .collect();
        let missing: Vec<&&str> = reps
            .iter()
            .filter(|r| !tool_names.contains(&r.to_lowercase()))
            .collect();
        eprintln!(
            "  {:12} {}/{} present{}",
            ecosystem,
            present.len(),
            reps.len(),
            if missing.is_empty() {
                String::new()
            } else {
                format!(
                    " (missing: {})",
                    missing.iter().map(|m| **m).collect::<Vec<_>>().join(", ")
                )
            }
        );
        assert!(
            !present.is_empty(),
            "Ecosystem '{}' has no representative tools in index (expected at least one of {:?})",
            ecosystem,
            reps
        );
    }
}

#[test]
fn test_no_regression_from_previous_build() {
    let Some(tools) = load_real_index() else {
        eprintln!("SKIP: no real index at ~/.clidex/index.yaml");
        return;
    };

    // Baselines from 2026-04-02 build. Update when data sources change intentionally.
    let total = tools.len();
    let baseline_total = 5000;

    assert!(
        total >= baseline_total,
        "Index size regression: expected >= {} tools, got {} (did a data source break?)",
        baseline_total,
        total
    );
}

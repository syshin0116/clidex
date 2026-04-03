use clidex::model::{Index, Links, Tool};
use regex::Regex;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

const AWESOME_CLI_APPS_URL: &str =
    "https://raw.githubusercontent.com/agarrharr/awesome-cli-apps/master/readme.md";
const HOMEBREW_FORMULA_URL: &str = "https://formulae.brew.sh/api/formula.json";
const TOOLLEEO_APPS_URL: &str =
    "https://raw.githubusercontent.com/toolleeo/cli-apps/master/data/apps.csv";
const TOOLLEEO_CATEGORIES_URL: &str =
    "https://raw.githubusercontent.com/toolleeo/cli-apps/master/data/categories.csv";
const MODERN_UNIX_URL: &str =
    "https://raw.githubusercontent.com/ibraheemdev/modern-unix/master/readme.md";
const AWESOME_TUIS_URL: &str =
    "https://raw.githubusercontent.com/rothgar/awesome-tuis/main/README.md";
const BREW_ANALYTICS_URL: &str =
    "https://formulae.brew.sh/api/analytics/install-on-request/365d.json";
const HOMEBREW_CASK_URL: &str = "https://formulae.brew.sh/api/cask.json";
const NPM_SEARCH_URL: &str = "https://registry.npmjs.org/-/v1/search";

#[derive(Debug, Deserialize)]
struct BrewFormula {
    name: String,
    desc: Option<String>,
    homepage: Option<String>,
    #[serde(default)]
    keg_only: bool,
}

#[derive(Debug, Deserialize)]
struct BrewCask {
    token: String,
    name: Vec<String>,
    desc: Option<String>,
    homepage: Option<String>,
}

/// Parse awesome-cli-apps markdown into tools
fn parse_awesome_cli_apps(markdown: &str) -> Vec<Tool> {
    let mut tools = Vec::new();
    let mut current_category = String::new();
    let mut current_subcategory: Option<String> = None;

    let link_re = Regex::new(r"^\s*-\s+\[([^\]]+)\]\(([^)]+)\)\s*[-–—]\s*(.+)$").unwrap();
    let link_no_desc_re = Regex::new(r"^\s*-\s+\[([^\]]+)\]\(([^)]+)\)\s*$").unwrap();
    let multi_link_re =
        Regex::new(r"^\s*-\s+\[([^\]]+)\]\(([^)]+)\),\s*\[([^\]]+)\]\(([^)]+)\)\s*[-–—]\s*(.+)$")
            .unwrap();
    let heading_re = Regex::new(r"^(#{2,4})\s+(.+)$").unwrap();

    for line in markdown.lines() {
        if let Some(caps) = heading_re.captures(line) {
            let level = caps[1].len();
            let heading = caps[2].trim().to_string();

            if matches!(
                heading.as_str(),
                "Contents" | "Related" | "License" | "Other Awesome Lists"
            ) || heading.starts_with("Related")
            {
                continue;
            }

            match level {
                2 => {
                    current_category = heading;
                    current_subcategory = None;
                }
                3 | 4 => {
                    current_subcategory = Some(heading);
                }
                _ => {}
            }
            continue;
        }

        let entries: Vec<(String, String, String)> =
            if let Some(caps) = multi_link_re.captures(line) {
                let desc = caps[5].trim().trim_end_matches('.').to_string();
                vec![
                    (
                        caps[1].trim().to_string(),
                        caps[2].trim().to_string(),
                        desc.clone(),
                    ),
                    (caps[3].trim().to_string(), caps[4].trim().to_string(), desc),
                ]
            } else if let Some(caps) = link_re.captures(line) {
                vec![(
                    caps[1].trim().to_string(),
                    caps[2].trim().to_string(),
                    caps[3].trim().trim_end_matches('.').to_string(),
                )]
            } else if let Some(caps) = link_no_desc_re.captures(line) {
                let url = caps[2].trim().to_string();
                if url.starts_with("http") {
                    vec![(
                        caps[1].trim().to_string(),
                        url,
                        current_subcategory
                            .clone()
                            .unwrap_or_else(|| current_category.clone()),
                    )]
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

        for (name, url, desc) in entries {
            if current_category.is_empty() {
                continue;
            }

            let category = if let Some(ref sub) = current_subcategory {
                format!("{} > {}", current_category, sub)
            } else {
                current_category.clone()
            };

            let repo = if url.starts_with("https://github.com/") {
                Some(url.clone())
            } else {
                None
            };

            let homepage = if !url.starts_with("https://github.com/") {
                Some(url.clone())
            } else {
                None
            };

            let tags = generate_tags(&name, &desc, &category);

            tools.push(Tool {
                name,
                binary: None,
                desc,
                category,
                tags,
                install: BTreeMap::new(),
                stars: None,
                brew_installs_365d: None,
                links: Links {
                    repo,
                    homepage,
                    docs: None,
                    llms_txt: None,
                },
                last_updated: None,
            });
        }
    }

    tools
}

/// Parse a single CSV line handling quoted fields
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                // Check for escaped quote (double quote)
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(c);
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == ',' {
            fields.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
    }
    fields.push(current.trim().to_string());
    fields
}

/// Parse toolleeo/cli-apps CSV data into tools
fn parse_toolleeo_csv(apps_csv: &str, categories_csv: &str) -> Vec<Tool> {
    // Build label → name mapping from categories CSV
    let mut category_map: HashMap<String, String> = HashMap::new();
    for line in categories_csv.lines().skip(1) {
        let fields = parse_csv_line(line);
        if fields.len() >= 2 {
            let label = fields[0].clone();
            let name = fields[1].clone();
            if !label.is_empty() && !name.is_empty() {
                category_map.insert(label, name);
            }
        }
    }

    let mut tools = Vec::new();

    for line in apps_csv.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let fields = parse_csv_line(line);
        // columns: category,name,homepage,git,description
        if fields.len() < 5 {
            continue;
        }

        let cat_label = &fields[0];
        let name = &fields[1];
        let homepage = &fields[2];
        let git = &fields[3];
        let desc = &fields[4];

        if name.is_empty() {
            continue;
        }

        let category = category_map
            .get(cat_label.as_str())
            .cloned()
            .unwrap_or_else(|| cat_label.clone());

        let repo = if git.starts_with("https://github.com/") {
            Some(git.clone())
        } else {
            None
        };

        let tool_homepage = if !homepage.is_empty() && !homepage.starts_with("https://github.com/")
        {
            Some(homepage.clone())
        } else {
            None
        };

        let tags = generate_tags(name, desc, &category);

        tools.push(Tool {
            name: name.clone(),
            binary: None,
            desc: desc.clone(),
            category,
            tags,
            install: BTreeMap::new(),
            stars: None,
            brew_installs_365d: None,
            links: Links {
                repo,
                homepage: tool_homepage,
                docs: None,
                llms_txt: None,
            },
            last_updated: None,
        });
    }

    tools
}

/// Well-known CLI tool aliases (tool name → common binary/alias names)
const KNOWN_ALIASES: &[(&str, &[&str])] = &[
    ("ripgrep", &["rg"]),
    ("fd", &["fd-find"]),
    ("bat", &["batcat"]),
    ("eza", &["exa"]),
    ("dust", &["du-dust"]),
    ("bottom", &["btm"]),
    ("procs", &["ps"]),
    ("tokei", &["cloc", "loc"]),
    ("hyperfine", &["bench"]),
    ("delta", &["git-delta"]),
    ("zoxide", &["z", "cd"]),
    ("The Fuck", &["thefuck", "fuck"]),
    ("youtube-dl", &["ytdl"]),
    ("yt-dlp", &["ytdlp"]),
    ("tmux", &["multiplexer"]),
    ("dog", &["dns"]),
    ("htop", &["top", "process-monitor"]),
    ("lazygit", &["lgit"]),
    ("lazydocker", &["ldocker"]),
    ("navi", &["cheatsheet"]),
    ("tldr", &["manpage", "man"]),
    ("starship", &["prompt"]),
    ("fzf", &["fuzzy-finder"]),
];

/// Generate search tags from tool metadata
fn generate_tags(name: &str, desc: &str, category: &str) -> Vec<String> {
    let mut tags = Vec::new();

    for part in category.split(['>', ' ']) {
        let part = part.trim().to_lowercase();
        if part.len() > 2 && !["and", "the", "for", "with"].contains(&part.as_str()) {
            tags.push(part);
        }
    }

    let stopwords: &[&str] = &[
        "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
        "from", "is", "it", "that", "this", "as", "are", "was", "be", "has", "had", "have", "do",
        "does", "did", "will", "would", "could", "should", "may", "might", "your", "you", "its",
        "like", "into", "than", "more", "very", "just", "also", "such", "which", "their", "them",
        "been", "being", "through", "between",
    ];

    for word in desc.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_') {
        let w = word.to_lowercase();
        if w.len() > 2 && !stopwords.contains(&w.as_str()) && !tags.contains(&w) {
            tags.push(w);
        }
    }

    let name_lower = name.to_lowercase();
    if !tags.contains(&name_lower) {
        tags.push(name_lower.clone());
    }

    for (tool_name, aliases) in KNOWN_ALIASES {
        if name.eq_ignore_ascii_case(tool_name) || name_lower == tool_name.to_lowercase() {
            for alias in *aliases {
                let a = alias.to_lowercase();
                if !tags.contains(&a) {
                    tags.push(a);
                }
            }
            break;
        }
    }

    tags
}

/// CLI filtering heuristics for Homebrew formulae
/// Returns true if the formula is likely a CLI tool (not a library)
fn is_likely_cli(formula: &BrewFormula) -> bool {
    // keg_only formulae are usually libraries
    if formula.keg_only {
        return false;
    }

    let desc = formula.desc.as_deref().unwrap_or("").to_lowercase();

    // Library indicators
    let lib_keywords = [
        "library",
        "libraries",
        "lib for",
        "binding",
        "bindings",
        "development framework",
        "header files",
        "c++ ",
        "c/c++",
        "sdk for",
        "api for",
    ];
    for kw in &lib_keywords {
        if desc.contains(kw) {
            return false;
        }
    }

    // CLI indicators (positive signals)
    let cli_keywords = [
        "command-line",
        "command line",
        "cli ",
        "cli tool",
        "terminal",
        "shell",
        "console",
        "tui",
        "ncurses",
        "curses",
    ];
    for kw in &cli_keywords {
        if desc.contains(kw) {
            return true;
        }
    }

    // Action verbs that suggest a tool
    let tool_verbs = [
        "manage",
        "monitor",
        "search",
        "find",
        "convert",
        "process",
        "analyze",
        "benchmark",
        "compile",
        "format",
        "lint",
        "download",
        "upload",
        "compress",
        "encrypt",
        "decrypt",
        "generate",
        "visualize",
        "diff",
        "merge",
        "test",
        "debug",
    ];
    for verb in &tool_verbs {
        if desc.contains(verb) {
            return true;
        }
    }

    // Default: include (most brew formulae are tools)
    true
}

/// Enrich tools with Homebrew data
fn enrich_with_homebrew(tools: &mut [Tool], brew_data: &[BrewFormula]) {
    let brew_map: HashMap<String, &BrewFormula> =
        brew_data.iter().map(|f| (f.name.clone(), f)).collect();

    for tool in tools.iter_mut() {
        let name_lower = tool.name.to_lowercase();
        let candidates = [
            name_lower.clone(),
            name_lower.replace('-', ""),
            name_lower.replace('_', "-"),
        ];

        for candidate in &candidates {
            if let Some(formula) = brew_map.get(candidate.as_str()) {
                tool.install
                    .insert("brew".to_string(), format!("brew install {}", formula.name));

                if tool.links.homepage.is_none() {
                    if let Some(ref hp) = formula.homepage {
                        if !hp.starts_with("https://github.com/") {
                            tool.links.homepage = Some(hp.clone());
                        }
                    }
                }

                break;
            }
        }
    }
}

/// Add popular Homebrew-only CLI tools not in awesome-cli-apps
fn add_homebrew_cli_tools(existing: &mut Vec<Tool>, brew_data: &[BrewFormula]) {
    let existing_names: std::collections::HashSet<String> =
        existing.iter().map(|t| t.name.to_lowercase()).collect();

    // Well-known CLI tools that should be in the index
    let wanted: HashMap<&str, &str> = [
        ("tmux", "Terminal Multiplexer"),
        ("hyperfine", "Development > Benchmarking"),
        ("tokei", "Development > Code Statistics"),
        ("git-delta", "Version Control > Git"),
        ("bottom", "Utilities > System Monitoring"),
        ("procs", "Utilities > System Monitoring"),
        ("htop", "Utilities > System Monitoring"),
        ("wget", "Utilities > Networking"),
        ("watch", "Utilities > Shell Utilities"),
        ("tree", "Files and Directories > Directory Listing"),
        ("jc", "Data Manipulation > Processors"),
        ("glow", "Utilities > Terminal Rendering"),
        ("xh", "Development > HTTP Client"),
        ("zellij", "Terminal Multiplexer"),
        ("nushell", "Utilities > Shell"),
        ("fish", "Utilities > Shell"),
        ("helix", "Development > Editor"),
        ("neovim", "Development > Editor"),
        ("micro", "Development > Editor"),
    ]
    .iter()
    .copied()
    .collect();

    let brew_map: HashMap<String, &BrewFormula> =
        brew_data.iter().map(|f| (f.name.clone(), f)).collect();

    for (name, category) in &wanted {
        if existing_names.contains(&name.to_lowercase()) {
            continue;
        }

        if let Some(formula) = brew_map.get(*name) {
            let desc = formula.desc.clone().unwrap_or_default();
            let tags = generate_tags(name, &desc, category);

            let homepage = formula.homepage.as_ref().and_then(|hp| {
                if hp.starts_with("https://github.com/") {
                    None
                } else {
                    Some(hp.clone())
                }
            });

            let repo = formula.homepage.as_ref().and_then(|hp| {
                if hp.starts_with("https://github.com/") {
                    Some(hp.clone())
                } else {
                    None
                }
            });

            let mut install = BTreeMap::new();
            install.insert("brew".to_string(), format!("brew install {}", formula.name));

            existing.push(Tool {
                name: name.to_string(),
                binary: None,
                desc,
                category: category.to_string(),
                tags,
                install,
                stars: None,
                brew_installs_365d: None,
                links: Links {
                    repo,
                    homepage,
                    docs: None,
                    llms_txt: None,
                },
                last_updated: None,
            });
        }
    }
}

fn parse_github_repo(url: &str) -> Option<(String, String)> {
    let url = url.trim_end_matches('/');
    if let Some(path) = url.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    None
}

/// Load stars cache from previous index file
fn load_stars_cache(path: &str) -> HashMap<String, (u64, Option<String>, Option<String>)> {
    // Returns: name -> (stars, last_updated, homepage)
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let index: Index = match serde_yaml::from_str(&content) {
        Ok(i) => i,
        Err(_) => return HashMap::new(),
    };
    index
        .tools
        .into_iter()
        .filter_map(|t| {
            let stars = t.stars?;
            Some((
                t.name.to_lowercase(),
                (stars, t.last_updated, t.links.homepage),
            ))
        })
        .collect()
}

/// Check if a cached entry is fresh (within max_age_days)
fn is_cache_fresh(last_updated: &Option<String>, max_age_days: u64) -> bool {
    let updated = match last_updated {
        Some(s) => s,
        None => return false,
    };
    // Parse ISO 8601 date prefix (YYYY-MM-DD)
    if updated.len() < 10 {
        return false;
    }
    let now = chrono_now();
    if now.len() < 10 {
        return false;
    }
    // Simple day-based comparison using the date command
    // If we can't parse, treat as stale
    let updated_date = &updated[..10];
    let now_date = &now[..10];
    // Calculate approximate days difference
    let days_diff = date_diff_days(updated_date, now_date);
    days_diff < max_age_days as i64
}

fn date_diff_days(date_a: &str, date_b: &str) -> i64 {
    // Simple YYYY-MM-DD diff. Parse to days since epoch (approximate).
    fn to_days(d: &str) -> Option<i64> {
        let parts: Vec<&str> = d.split('-').collect();
        if parts.len() < 3 {
            return None;
        }
        let y: i64 = parts[0].parse().ok()?;
        let m: i64 = parts[1].parse().ok()?;
        let d: i64 = parts[2].parse().ok()?;
        Some(y * 365 + m * 30 + d)
    }
    match (to_days(date_a), to_days(date_b)) {
        (Some(a), Some(b)) => (b - a).abs(),
        _ => 999, // treat as stale if can't parse
    }
}

/// Fetch GitHub stars for a single tool, returning updated fields
async fn fetch_single_github(
    owner: String,
    repo: String,
    token: Option<String>,
    client: reqwest::Client,
) -> Option<(u64, Option<String>, Option<String>)> {
    let api_url = format!("https://api.github.com/repos/{}/{}", owner, repo);
    let mut req = client
        .get(&api_url)
        .header("User-Agent", "clidex-build/0.1")
        .header("Accept", "application/vnd.github.v3+json");
    if let Some(ref t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }

    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                let stars = json["stargazers_count"].as_u64();
                let pushed = json["pushed_at"].as_str().map(String::from);
                let homepage = json["homepage"].as_str().and_then(|hp| {
                    if !hp.is_empty() && !hp.starts_with("https://github.com/") {
                        Some(hp.to_string())
                    } else {
                        None
                    }
                });
                return stars.map(|s| (s, pushed, homepage));
            }
            None
        }
        _ => None,
    }
}

/// Enrich tools with GitHub stars — uses cache from previous index + parallel fetching
async fn enrich_with_github(
    tools: &mut [Tool],
    client: &reqwest::Client,
    max_requests: usize,
    stars_cache: &HashMap<String, (u64, Option<String>, Option<String>)>,
    cache_max_age_days: u64,
) {
    let token = std::env::var("GITHUB_TOKEN").ok();
    let mut cached_hits = 0;

    // Phase 1: Apply cache (3-day freshness)
    for tool in tools.iter_mut() {
        if tool.stars.is_some() {
            continue;
        }
        let key = tool.name.to_lowercase();
        if let Some((stars, last_updated, homepage)) = stars_cache.get(&key) {
            if is_cache_fresh(last_updated, cache_max_age_days) {
                tool.stars = Some(*stars);
                if tool.last_updated.is_none() {
                    tool.last_updated = last_updated.clone();
                }
                if tool.links.homepage.is_none() {
                    tool.links.homepage = homepage.clone();
                }
                cached_hits += 1;
            }
        }
    }
    eprintln!(
        "GitHub stars: {} from cache ({}d fresh)",
        cached_hits, cache_max_age_days
    );

    // Phase 2: Collect tools that need fresh API calls
    let mut to_fetch: Vec<(usize, String, String)> = Vec::new();
    for (i, tool) in tools.iter().enumerate() {
        if tool.stars.is_some() {
            continue;
        }
        if to_fetch.len() >= max_requests {
            break;
        }
        let repo_url = match &tool.links.repo {
            Some(url) => url.clone(),
            None => continue,
        };
        if let Some((owner, repo)) = parse_github_repo(&repo_url) {
            to_fetch.push((i, owner, repo));
        }
    }
    eprintln!("GitHub stars: {} to fetch via API", to_fetch.len());

    // Phase 3: Parallel fetch (10 concurrent)
    let concurrency = 10;
    let mut fetched = 0;
    for chunk in to_fetch.chunks(concurrency) {
        let futures: Vec<_> = chunk
            .iter()
            .map(|(_, owner, repo)| {
                fetch_single_github(owner.clone(), repo.clone(), token.clone(), client.clone())
            })
            .collect();

        let results = futures::future::join_all(futures).await;

        for (result, (idx, _, _)) in results.into_iter().zip(chunk.iter()) {
            if let Some((stars, pushed, homepage)) = result {
                tools[*idx].stars = Some(stars);
                if tools[*idx].last_updated.is_none() {
                    tools[*idx].last_updated = pushed;
                }
                if tools[*idx].links.homepage.is_none() {
                    tools[*idx].links.homepage = homepage;
                }
                fetched += 1;
            }
        }
    }

    eprintln!(
        "GitHub stars: {} cached + {} fetched = {} total",
        cached_hits,
        fetched,
        cached_hits + fetched
    );
}

/// Enrich tools with crates.io install commands
/// Checks if tool name (or known crate name) exists on crates.io and has a binary target
async fn enrich_with_crates_io(tools: &mut [Tool], client: &reqwest::Client, max_requests: usize) {
    // Map of tool names to their crate names (when different)
    let crate_overrides: HashMap<&str, &str> = [
        ("ripgrep", "ripgrep"),
        ("fd", "fd-find"),
        ("bat", "bat"),
        ("dust", "du-dust"),
        ("bottom", "bottom"),
        ("procs", "procs"),
        ("tokei", "tokei"),
        ("hyperfine", "hyperfine"),
        ("delta", "git-delta"),
        ("zoxide", "zoxide"),
        ("eza", "eza"),
        ("sd", "sd"),
        ("choose", "choose"),
        ("grex", "grex"),
        ("tealdeer", "tealdeer"),
        ("starship", "starship"),
        ("zellij", "zellij"),
        ("nushell", "nu"),
        ("helix", "helix-term"),
        ("gitui", "gitui"),
        ("broot", "broot"),
        ("xh", "xh"),
        ("dog", "dog"),
        ("lsd", "lsd"),
        ("bandwhich", "bandwhich"),
        ("diskonaut", "diskonaut"),
        ("xsv", "xsv"),
    ]
    .iter()
    .copied()
    .collect();

    let mut requests_made = 0;
    let mut found = 0;

    for tool in tools.iter_mut() {
        if requests_made >= max_requests {
            break;
        }

        // Skip if already has cargo install
        if tool.install.contains_key("cargo") {
            continue;
        }

        // Determine crate name to look up
        let tool_lower = tool.name.to_lowercase();
        let crate_name = crate_overrides
            .get(tool_lower.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| tool_lower.clone());

        let api_url = format!("https://crates.io/api/v1/crates/{}", crate_name);
        requests_made += 1;

        match client
            .get(&api_url)
            .header(
                "User-Agent",
                "clidex-build/0.1 (https://github.com/syshin0116/clidex)",
            )
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    // Verify it's likely the same project by checking repository URL
                    let crate_repo = json["crate"]["repository"]
                        .as_str()
                        .unwrap_or("")
                        .to_lowercase();
                    let tool_repo = tool.links.repo.as_deref().unwrap_or("").to_lowercase();

                    // Extract owner/repo from GitHub URLs for comparison
                    let extract_owner_repo = |url: &str| -> Option<String> {
                        url.strip_prefix("https://github.com/")
                            .map(|p| p.trim_end_matches('/').to_lowercase())
                    };
                    let repos_same = extract_owner_repo(&crate_repo)
                        .zip(extract_owner_repo(&tool_repo))
                        .map(|(a, b)| a == b)
                        .unwrap_or(false);
                    let repo_match =
                        repos_same || crate_overrides.contains_key(tool_lower.as_str());

                    if repo_match {
                        tool.install
                            .insert("cargo".to_string(), format!("cargo install {}", crate_name));
                        found += 1;
                    }
                }
            }
            Ok(resp) if resp.status().as_u16() == 429 => {
                eprintln!("crates.io: rate limited after {} requests", requests_made);
                break;
            }
            _ => {}
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    eprintln!(
        "crates.io: checked {} tools, added cargo install for {}",
        requests_made, found
    );
}

/// Enrich tools with npm install commands
/// Checks if tool name exists on npm and has a bin field
async fn enrich_with_npm(tools: &mut [Tool], client: &reqwest::Client, max_requests: usize) {
    // Map of tool names to npm package names (when different)
    let npm_overrides: HashMap<&str, &str> = [
        ("tldr", "tldr"),
        ("trash-cli", "trash-cli"),
        ("empty-trash-cli", "empty-trash-cli"),
        ("np", "np"),
        ("npm-name-cli", "npm-name-cli"),
        ("speed-test", "speed-test"),
        ("emoj", "emoj"),
        ("pageres-cli", "pageres-cli"),
        ("vtop", "vtop"),
        ("tmpin", "tmpin"),
        ("cpy-cli", "cpy-cli"),
        ("clipboard-cli", "clipboard-cli"),
        ("live-server", "live-server"),
        ("strip-json-comments-cli", "strip-json-comments-cli"),
        ("is-online-cli", "is-online-cli"),
        ("is-up-cli", "is-up-cli"),
        ("public-ip-cli", "public-ip-cli"),
        ("pen.md", "pen.md"),
        ("gist-cli", "gist-cli"),
        ("diff2html-cli", "diff2html-cli"),
    ]
    .iter()
    .copied()
    .collect();

    let mut requests_made = 0;
    let mut found = 0;

    for tool in tools.iter_mut() {
        if requests_made >= max_requests {
            break;
        }

        // Skip if already has npm install
        if tool.install.contains_key("npm") {
            continue;
        }

        let tool_lower = tool.name.to_lowercase();
        let pkg_name = npm_overrides
            .get(tool_lower.as_str())
            .map(|s| s.to_string());

        // Only check known npm packages or tools from npm-heavy categories
        let should_check = pkg_name.is_some()
            || tool
                .links
                .repo
                .as_deref()
                .map(|r| r.contains("npmjs.com"))
                .unwrap_or(false)
            || tool
                .links
                .homepage
                .as_deref()
                .map(|h| h.contains("npmjs.com"))
                .unwrap_or(false);

        if !should_check {
            continue;
        }

        let pkg = pkg_name.unwrap_or_else(|| tool_lower.clone());
        let api_url = format!("https://registry.npmjs.org/{}", pkg);
        requests_made += 1;

        match client
            .get(&api_url)
            .header("User-Agent", "clidex-build/0.1")
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    // Check bin at top level or in latest version
                    let has_bin = json.get("bin").is_some()
                        || json.get("directories").and_then(|d| d.get("bin")).is_some()
                        || json
                            .get("dist-tags")
                            .and_then(|dt| dt.get("latest"))
                            .and_then(|v| v.as_str())
                            .and_then(|latest| {
                                json.get("versions")
                                    .and_then(|vs| vs.get(latest))
                                    .and_then(|v| v.get("bin"))
                            })
                            .is_some();

                    if has_bin {
                        tool.install
                            .insert("npm".to_string(), format!("npm install -g {}", pkg));
                        found += 1;
                    }
                }
            }
            Ok(resp) if resp.status().as_u16() == 429 => {
                eprintln!("npm: rate limited after {} requests", requests_made);
                break;
            }
            _ => {}
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    eprintln!(
        "npm: checked {} tools, added npm install -g for {}",
        requests_made, found
    );
}

/// Probe for llms.txt at known locations for tools with GitHub repos
async fn probe_llms_txt(tools: &mut [Tool], client: &reqwest::Client, max_probes: usize) {
    let mut probed = 0;
    let mut found = 0;

    for tool in tools.iter_mut() {
        if probed >= max_probes {
            break;
        }

        // Skip if already has llms.txt
        if tool.links.llms_txt.is_some() {
            continue;
        }

        // Try homepage/llms.txt first, then common patterns
        let mut urls_to_try = Vec::new();

        if let Some(ref hp) = tool.links.homepage {
            let base = hp.trim_end_matches('/');
            urls_to_try.push(format!("{}/llms.txt", base));
        }

        // Try docs site patterns
        if let Some(ref repo) = tool.links.repo {
            if let Some((owner, name)) = parse_github_repo(repo) {
                // GitHub Pages pattern
                urls_to_try.push(format!("https://{}.github.io/{}/llms.txt", owner, name));
                // readthedocs pattern
                urls_to_try.push(format!("https://{}.readthedocs.io/llms.txt", name));
            }
        }

        for url in &urls_to_try {
            probed += 1;
            if probed > max_probes {
                break;
            }

            match client
                .head(url)
                .header("User-Agent", "clidex-build/0.1")
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    // Verify it's text content, not an HTML error page
                    let content_type = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    if content_type.contains("text/plain")
                        || content_type.contains("text/markdown")
                        || !content_type.contains("text/html")
                    {
                        tool.links.llms_txt = Some(url.clone());
                        found += 1;
                        eprintln!("  llms.txt found: {} -> {}", tool.name, url);
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    eprintln!("llms.txt: probed {} URLs, found {}", probed, found);
}

/// Deduplicate tools by repo URL and name, merging data from duplicates
fn deduplicate(tools: &mut Vec<Tool>) {
    // Phase 1: Merge by repo URL — collect merge pairs first
    let mut by_repo: HashMap<String, usize> = HashMap::new();
    let mut to_remove = Vec::new();
    // (target_idx, source_idx) pairs for merging
    let mut merge_pairs: Vec<(usize, usize)> = Vec::new();

    for (i, tool) in tools.iter().enumerate() {
        if let Some(ref repo) = tool.links.repo {
            let key = repo
                .trim_end_matches('/')
                .trim_end_matches(".git")
                .to_lowercase();
            if let Some(&prev_idx) = by_repo.get(&key) {
                merge_pairs.push((prev_idx, i));
                to_remove.push(i);
            } else {
                by_repo.insert(key, i);
            }
        }
    }

    // Apply merges (source into target)
    for (target_idx, source_idx) in &merge_pairs {
        let other = tools[*source_idx].clone();
        let base = &mut tools[*target_idx];

        // Keep longer description
        if other.desc.len() > base.desc.len() {
            base.desc = other.desc;
        }
        // Merge install methods
        for (k, v) in other.install {
            base.install.entry(k).or_insert(v);
        }
        // Merge tags
        for tag in other.tags {
            if !base.tags.contains(&tag) {
                base.tags.push(tag);
            }
        }
        // Keep higher stars
        match (base.stars, other.stars) {
            (None, Some(s)) => base.stars = Some(s),
            (Some(a), Some(b)) if b > a => base.stars = Some(b),
            _ => {}
        }
        // Fill empty links
        if base.links.homepage.is_none() {
            base.links.homepage = other.links.homepage;
        }
        if base.links.docs.is_none() {
            base.links.docs = other.links.docs;
        }
        // Keep brew_installs_365d
        if base.brew_installs_365d.is_none() {
            base.brew_installs_365d = other.brew_installs_365d;
        }
    }

    // Phase 2: Merge by name (for entries without repo URL)
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for (i, tool) in tools.iter().enumerate() {
        if to_remove.contains(&i) {
            continue;
        }
        let key = tool.name.to_lowercase();
        if let Some(&prev_idx) = by_name.get(&key) {
            if to_remove.contains(&prev_idx) {
                by_name.insert(key, i);
                continue;
            }
            let prev = &tools[prev_idx];
            let prev_score = prev.install.len() + prev.tags.len() + prev.stars.is_some() as usize;
            let cur_score = tool.install.len() + tool.tags.len() + tool.stars.is_some() as usize;
            if cur_score > prev_score {
                to_remove.push(prev_idx);
                by_name.insert(key, i);
            } else {
                to_remove.push(i);
            }
        } else {
            by_name.insert(key, i);
        }
    }

    to_remove.sort_unstable();
    to_remove.dedup();
    for idx in to_remove.into_iter().rev() {
        tools.remove(idx);
    }
}

fn auto_categorize(desc: &str, name: &str) -> String {
    let desc_lower = desc.to_lowercase();
    let name_lower = name.to_lowercase();
    let text = format!("{} {}", name_lower, desc_lower);

    let rules: &[(&[&str], &str)] = &[
        (
            &["docker", "container", "podman", "image layer"],
            "Development > Docker",
        ),
        (
            &["kubernetes", "k8s", "kubectl", "helm"],
            "Development > Kubernetes",
        ),
        (
            &["git ", "git-", "commit", "branch", "version control"],
            "Version Control > Git",
        ),
        (
            &["linter", "lint ", "linting", "code quality"],
            "Development > Linting",
        ),
        (
            &["formatter", "formatting", "prettify", "beautif"],
            "Development > Formatting",
        ),
        (
            &["http client", "http request", "curl", "api client"],
            "Development > HTTP Client",
        ),
        (
            &["http test", "api test", "load test"],
            "Development > Testing",
        ),
        (
            &["file manager", "file explorer", "file browser"],
            "Files and Directories > File Managers",
        ),
        (
            &["search", "grep", "find files", "regex"],
            "Files and Directories > Search",
        ),
        (
            &["rename", "batch rename"],
            "Files and Directories > Renaming",
        ),
        (
            &["shell history", "history search"],
            "Utilities > Shell History",
        ),
        (&["shell ", "bash ", "zsh ", "fish "], "Utilities > Shell"),
        (
            &["prompt", "starship", "powerline"],
            "Utilities > Shell Prompt",
        ),
        (
            &["monitor", "system monitor", "top ", "htop", "process"],
            "Utilities > System Monitoring",
        ),
        (
            &["disk usage", "disk space", " du "],
            "Utilities > Disk Usage",
        ),
        (
            &["json", "yaml", "toml", "csv", "data process"],
            "Data Manipulation > Processors",
        ),
        (
            &["encrypt", "decrypt", "cipher", "crypto", "gpg", "age "],
            "Security > Encryption",
        ),
        (&["terminal emulator"], "Utilities > Terminal Emulator"),
        (
            &["multiplexer", "tmux", "terminal workspace"],
            "Utilities > Terminal Multiplexer",
        ),
        (
            &["package manager", "package installer", "dependency manager"],
            "Development > Package Manager",
        ),
        (
            &[
                "version manager",
                "node manager",
                "python manager",
                "runtime manager",
            ],
            "Development > Version Manager",
        ),
        (
            &["benchmark", "timing", "performance", "profil"],
            "Development > Benchmarking",
        ),
        (
            &["editor", "text editor", "vim", "neovim", "nano"],
            "Development > Editor",
        ),
        (&["markdown", "md "], "Utilities > Markdown"),
        (
            &["test ", "testing", "test framework", "unit test"],
            "Development > Testing",
        ),
        (
            &["ci/cd", "ci cd", "deploy", "devops", "release"],
            "Development > DevOps",
        ),
        (&["diff", "compare", "merge"], "Development > Diff"),
        (
            &["network", "dns", "ping", "traceroute", "ip "],
            "Utilities > Networking",
        ),
        (
            &["download", "wget", "fetch", "scrape"],
            "Utilities > Download",
        ),
        (
            &["compress", "decompress", "archive", "zip", "tar", "gzip"],
            "Utilities > Compression",
        ),
        (
            &["image", "photo", "picture", "png", "jpg", "svg"],
            "Utilities > Image Processing",
        ),
        (
            &["video", "media", "stream", "mp4", "ffmpeg"],
            "Utilities > Media",
        ),
        (
            &["database", "sql", "sqlite", "postgres", "mysql", "redis"],
            "Development > Database",
        ),
        (
            &[
                "ai ",
                "llm",
                "gpt",
                "claude",
                "openai",
                "gemini",
                "copilot",
                "coding assistant",
            ],
            "AI > LLM Interaction",
        ),
        (&["clipboard", "copy", "paste"], "Utilities > Clipboard"),
        (
            &["note", "todo", "task", "productivity"],
            "Productivity > Note Taking",
        ),
        (&["presentation", "slides"], "Productivity > Presentations"),
        (
            &["log ", "logging", "log viewer", "log file"],
            "Utilities > Log Viewer",
        ),
        (
            &["hex ", "hex viewer", "hexdump", "binary viewer"],
            "Utilities > Hex Viewer",
        ),
        (
            &["watch ", "file watch", "file change"],
            "Utilities > File Watching",
        ),
        (
            &["typesett", "latex", "tex ", "document"],
            "Utilities > Document Processing",
        ),
        (&["spell", "grammar", "typo"], "Development > Spell Check"),
    ];

    for (keywords, category) in rules {
        if keywords.iter().any(|kw| text.contains(kw)) {
            return category.to_string();
        }
    }

    "Utilities > General".to_string()
}

/// Discover CLI tools from Homebrew formulae not already in the index
fn discover_from_homebrew(
    existing: &mut Vec<Tool>,
    brew_data: &[BrewFormula],
    analytics: &HashMap<String, u64>,
) {
    let existing_names: std::collections::HashSet<String> =
        existing.iter().map(|t| t.name.to_lowercase()).collect();

    // Only hard-exclude things that are definitely NOT tools
    let hard_exclude = [
        "programming language",
        "runtime environment",
        "object-relational database",
        "relational database",
        "database system",
        "message queue",
        "message broker",
        "web server",
        "proxy server",
        "http server",
        "smtp server",
        "compiler collection",
        "compiler infrastructure",
        "x11 ",
        "x.org",
        "protocol buffers",
    ];

    let mut added = 0;
    for formula in brew_data {
        let name = &formula.name;
        if existing_names.contains(&name.to_lowercase()) {
            continue;
        }
        if formula.keg_only {
            continue;
        }
        if name.contains('@') {
            continue;
        }

        let desc = formula.desc.as_deref().unwrap_or("").to_lowercase();
        if hard_exclude.iter().any(|kw| desc.contains(kw)) {
            continue;
        }

        // Require minimum popularity: either in analytics with >5000 installs/year,
        // OR has a GitHub homepage (suggests it's a maintained project)
        let installs = analytics.get(name).copied().unwrap_or(0);
        let has_github = formula
            .homepage
            .as_ref()
            .is_some_and(|h| h.starts_with("https://github.com/"));

        if installs < 5000 && !has_github {
            continue;
        }

        let desc_str = formula.desc.clone().unwrap_or_default();
        let category = auto_categorize(&desc_str, name);
        let tags = generate_tags(name, &desc_str, &category);

        let homepage = formula.homepage.as_ref().and_then(|hp| {
            if hp.starts_with("https://github.com/") {
                None
            } else {
                Some(hp.clone())
            }
        });
        let repo = formula.homepage.as_ref().and_then(|hp| {
            if hp.starts_with("https://github.com/") {
                Some(hp.clone())
            } else {
                None
            }
        });

        let mut install = BTreeMap::new();
        install.insert("brew".to_string(), format!("brew install {}", name));

        existing.push(Tool {
            name: name.to_string(),
            binary: None,
            desc: desc_str,
            category,
            tags,
            install,
            stars: None,
            brew_installs_365d: if installs > 0 { Some(installs) } else { None },
            links: Links {
                repo,
                homepage,
                docs: None,
                llms_txt: None,
            },
            last_updated: None,
        });
        added += 1;
    }
    eprintln!("Discovered {} new tools from Homebrew", added);
}

fn discover_from_homebrew_casks(existing: &mut Vec<Tool>, cask_data: &[BrewCask]) {
    let existing_names: std::collections::HashSet<String> =
        existing.iter().map(|t| t.name.to_lowercase()).collect();

    // Only include casks that are terminal/CLI related
    let cli_cask_keywords = [
        "terminal",
        "shell",
        "command-line",
        "cli",
        "console",
        "emulator",
        "tui",
        "tmux",
        "multiplexer",
    ];

    let mut added = 0;
    for cask in cask_data {
        if existing_names.contains(&cask.token.to_lowercase()) {
            continue;
        }

        let desc = cask.desc.as_deref().unwrap_or("").to_lowercase();
        let name_str = cask.name.first().map(|s| s.as_str()).unwrap_or("");
        let combined = format!("{} {} {}", cask.token, name_str, desc);
        let combined_lower = combined.to_lowercase();

        if !cli_cask_keywords
            .iter()
            .any(|kw| combined_lower.contains(kw))
        {
            continue;
        }

        let desc_str = cask.desc.clone().unwrap_or_else(|| name_str.to_string());
        let category = auto_categorize(&desc_str, &cask.token);
        let tags = generate_tags(&cask.token, &desc_str, &category);

        let repo = cask.homepage.as_ref().and_then(|hp| {
            if hp.starts_with("https://github.com/") {
                Some(hp.clone())
            } else {
                None
            }
        });
        let homepage = cask.homepage.as_ref().and_then(|hp| {
            if hp.starts_with("https://github.com/") {
                None
            } else {
                Some(hp.clone())
            }
        });

        let mut install = BTreeMap::new();
        install.insert(
            "brew".to_string(),
            format!("brew install --cask {}", cask.token),
        );

        existing.push(Tool {
            name: cask.token.clone(),
            binary: None,
            desc: desc_str,
            category,
            tags,
            install,
            stars: None,
            brew_installs_365d: None,
            links: Links {
                repo,
                homepage,
                docs: None,
                llms_txt: None,
            },
            last_updated: None,
        });
        added += 1;
    }
    eprintln!("Discovered {} CLI-related casks from Homebrew", added);
}

async fn discover_from_npm(existing: &mut Vec<Tool>, client: &reqwest::Client) {
    let existing_names: std::collections::HashSet<String> =
        existing.iter().map(|t| t.name.to_lowercase()).collect();

    // Search for popular CLI packages on npm
    let queries = ["keywords:cli", "keywords:command-line-tool"];
    let mut added = 0;

    for query in &queries {
        let url = format!(
            "{}?text={}&size=50&quality=0.0&popularity=1.0&maintenance=0.0",
            NPM_SEARCH_URL, query
        );
        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => continue,
        };
        let data: serde_json::Value = match resp.json().await {
            Ok(d) => d,
            Err(_) => continue,
        };

        if let Some(objects) = data["objects"].as_array() {
            for obj in objects {
                let pkg = &obj["package"];
                let name = match pkg["name"].as_str() {
                    Some(n) => n,
                    None => continue,
                };

                // Skip scoped packages (like @anthropic-ai/claude-code) - use unscoped name
                let display_name = if name.contains('/') {
                    name.split('/').next_back().unwrap_or(name)
                } else {
                    name
                };

                if existing_names.contains(&display_name.to_lowercase()) {
                    continue;
                }

                let desc = pkg["description"].as_str().unwrap_or("").to_string();
                if desc.is_empty() {
                    continue;
                }

                let homepage = pkg["links"]["homepage"].as_str().map(String::from);
                let repo_url = pkg["links"]["repository"].as_str().map(String::from);

                let category = auto_categorize(&desc, display_name);
                let tags = generate_tags(display_name, &desc, &category);

                let mut install = BTreeMap::new();
                install.insert("npm".to_string(), format!("npm install -g {}", name));

                existing.push(Tool {
                    name: display_name.to_string(),
                    binary: None,
                    desc,
                    category,
                    tags,
                    install,
                    stars: None,
                    brew_installs_365d: None,
                    links: Links {
                        repo: repo_url,
                        homepage,
                        docs: None,
                        llms_txt: None,
                    },
                    last_updated: None,
                });
                added += 1;
            }
        }
    }
    eprintln!("Discovered {} CLI packages from npm", added);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    // Step 1: Fetch awesome-cli-apps
    eprintln!("Fetching awesome-cli-apps...");
    let awesome_md = client
        .get(AWESOME_CLI_APPS_URL)
        .send()
        .await?
        .text()
        .await?;
    eprintln!("Fetched {} bytes", awesome_md.len());

    let mut tools = parse_awesome_cli_apps(&awesome_md);
    eprintln!("Parsed {} tools from awesome-cli-apps", tools.len());

    // Step 1b: Fetch toolleeo/cli-apps
    eprintln!("Fetching toolleeo/cli-apps...");
    let toolleeo_apps = client.get(TOOLLEEO_APPS_URL).send().await?.text().await?;
    let toolleeo_cats = client
        .get(TOOLLEEO_CATEGORIES_URL)
        .send()
        .await?
        .text()
        .await?;
    eprintln!(
        "Fetched toolleeo: {} bytes apps, {} bytes categories",
        toolleeo_apps.len(),
        toolleeo_cats.len()
    );

    let mut toolleeo_tools = parse_toolleeo_csv(&toolleeo_apps, &toolleeo_cats);
    eprintln!(
        "Parsed {} tools from toolleeo/cli-apps",
        toolleeo_tools.len()
    );

    let toolleeo_limit: Option<usize> = std::env::var("TOOLLEEO_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok());
    if let Some(limit) = toolleeo_limit {
        toolleeo_tools.truncate(limit);
        eprintln!("Capped toolleeo tools to {} (TOOLLEEO_LIMIT)", limit);
    }

    // Merge: only add tools not already present (case-insensitive)
    let existing_names: std::collections::HashSet<String> =
        tools.iter().map(|t| t.name.to_lowercase()).collect();
    let mut toolleeo_added = 0;
    for tool in toolleeo_tools {
        if !existing_names.contains(&tool.name.to_lowercase()) {
            tools.push(tool);
            toolleeo_added += 1;
        }
    }
    eprintln!("Added {} new tools from toolleeo/cli-apps", toolleeo_added);

    // Step 1c: Fetch additional awesome lists
    for (url, source_name) in &[
        (MODERN_UNIX_URL, "modern-unix"),
        (AWESOME_TUIS_URL, "awesome-tuis"),
    ] {
        eprintln!("Fetching {}...", source_name);
        match client.get(*url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(md) => {
                    let extra_tools = parse_awesome_cli_apps(&md);
                    eprintln!("Parsed {} tools from {}", extra_tools.len(), source_name);
                    let existing_names: std::collections::HashSet<String> =
                        tools.iter().map(|t| t.name.to_lowercase()).collect();
                    let mut extra_added = 0;
                    for tool in extra_tools {
                        if !existing_names.contains(&tool.name.to_lowercase()) {
                            tools.push(tool);
                            extra_added += 1;
                        }
                    }
                    eprintln!("Added {} new tools from {}", extra_added, source_name);
                }
                Err(e) => eprintln!("Warning: Failed to read {}: {}", source_name, e),
            },
            Err(e) => eprintln!("Warning: Failed to fetch {}: {}", source_name, e),
        }
    }

    // Step 2: Fetch Homebrew data + enrich + add missing CLI tools
    eprintln!("Fetching Homebrew formula data...");
    let brew_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let brew_resp = brew_client.get(HOMEBREW_FORMULA_URL).send().await?;
    let brew_data: Vec<BrewFormula> = brew_resp.json().await?;
    let cli_count = brew_data.iter().filter(|f| is_likely_cli(f)).count();
    eprintln!(
        "Fetched {} Homebrew formulae ({} likely CLI)",
        brew_data.len(),
        cli_count
    );

    enrich_with_homebrew(&mut tools, &brew_data);
    let brew_matched = tools
        .iter()
        .filter(|t| t.install.contains_key("brew"))
        .count();
    eprintln!("Matched {} tools with Homebrew", brew_matched);

    let before = tools.len();
    add_homebrew_cli_tools(&mut tools, &brew_data);
    eprintln!(
        "Added {} popular CLI tools from Homebrew",
        tools.len() - before
    );

    // Fetch Homebrew analytics
    eprintln!("Fetching Homebrew install-on-request analytics...");
    let brew_analytics: HashMap<String, u64> = match client.get(BREW_ANALYTICS_URL).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => data["items"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|item| {
                    let name = item["formula"].as_str()?;
                    let count = item["count"]
                        .as_str()?
                        .replace(',', "")
                        .parse::<u64>()
                        .ok()?;
                    Some((name.to_string(), count))
                })
                .collect(),
            Err(e) => {
                eprintln!("Warning: Failed to parse analytics: {}", e);
                HashMap::new()
            }
        },
        Err(e) => {
            eprintln!("Warning: Failed to fetch analytics: {}", e);
            HashMap::new()
        }
    };
    eprintln!("Got analytics for {} formulae", brew_analytics.len());

    // Discover new tools from Homebrew (broad inclusion)
    discover_from_homebrew(&mut tools, &brew_data, &brew_analytics);

    // Apply brew analytics to ALL tools (not just Homebrew-discovered ones)
    let mut analytics_applied = 0;
    for tool in tools.iter_mut() {
        if tool.brew_installs_365d.is_none() {
            if let Some(&count) = brew_analytics.get(&tool.name) {
                tool.brew_installs_365d = Some(count);
                analytics_applied += 1;
            } else if let Some(&count) = brew_analytics.get(&tool.name.to_lowercase()) {
                tool.brew_installs_365d = Some(count);
                analytics_applied += 1;
            }
        }
    }
    eprintln!(
        "Applied brew analytics to {} additional tools",
        analytics_applied
    );

    // Step 2b: Fetch Homebrew Cask data
    eprintln!("Fetching Homebrew cask data...");
    match brew_client.get(HOMEBREW_CASK_URL).send().await {
        Ok(resp) => match resp.json::<Vec<BrewCask>>().await {
            Ok(cask_data) => {
                eprintln!("Fetched {} Homebrew casks", cask_data.len());
                discover_from_homebrew_casks(&mut tools, &cask_data);
            }
            Err(e) => eprintln!("Warning: Failed to parse cask data: {}", e),
        },
        Err(e) => eprintln!("Warning: Failed to fetch cask data: {}", e),
    }

    // Step 2c: Discover CLI packages from npm
    eprintln!("Discovering popular CLI packages from npm...");
    discover_from_npm(&mut tools, &client).await;

    // Deduplicate
    deduplicate(&mut tools);
    eprintln!("After dedup: {} tools", tools.len());

    // Step 3: GitHub stars (with cache from previous index + parallel fetching)
    let github_limit = std::env::var("GITHUB_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let prev_index_path = std::env::args().nth(2).unwrap_or_default();
    let stars_cache = if !prev_index_path.is_empty() {
        eprintln!("Loading stars cache from {}...", prev_index_path);
        let cache = load_stars_cache(&prev_index_path);
        eprintln!("Loaded {} cached stars entries", cache.len());
        cache
    } else {
        HashMap::new()
    };
    eprintln!(
        "Fetching GitHub stars (limit: {}, cache: {} entries)...",
        github_limit,
        stars_cache.len()
    );
    enrich_with_github(&mut tools, &client, github_limit, &stars_cache, 3).await;
    let with_stars = tools.iter().filter(|t| t.stars.is_some()).count();
    eprintln!("Got stars for {} tools", with_stars);

    // Step 4: crates.io enrichment
    let crates_limit = std::env::var("CRATES_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    eprintln!(
        "Checking crates.io (limit: {}, set CRATES_LIMIT to change)...",
        crates_limit
    );
    enrich_with_crates_io(&mut tools, &client, crates_limit).await;

    // Step 5: npm enrichment
    let npm_limit = std::env::var("NPM_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    eprintln!(
        "Checking npm (limit: {}, set NPM_LIMIT to change)...",
        npm_limit
    );
    enrich_with_npm(&mut tools, &client, npm_limit).await;

    // Step 6: Probe for llms.txt
    let llms_limit = std::env::var("LLMS_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    eprintln!(
        "Probing for llms.txt (limit: {}, set LLMS_LIMIT to change)...",
        llms_limit
    );
    probe_llms_txt(&mut tools, &client, llms_limit).await;

    // Step 7: Build and save index
    let index = Index {
        version: 1,
        generated: chrono_now(),
        tools,
    };

    let output_path = std::env::args().nth(1).unwrap_or_else(|| {
        let dir = clidex::config::clidex_dir();
        std::fs::create_dir_all(&dir).ok();
        dir.join("index.yaml").to_string_lossy().to_string()
    });

    clidex::index::save_index(&index, std::path::Path::new(&output_path))?;

    // Step 8: Generate embeddings (if semantic feature is enabled)
    #[cfg(feature = "semantic")]
    {
        eprintln!("Generating semantic embeddings...");
        match model2vec_rs::model::StaticModel::from_pretrained(
            "minishlab/potion-base-2M",
            None,
            None,
            None,
        ) {
            Ok(model) => {
                let texts: Vec<String> = index
                    .tools
                    .iter()
                    .map(|t| format!("{} {} {}", t.name, t.desc, t.tags.join(" ")))
                    .collect();
                let embeddings = model.encode(&texts);
                let dim = embeddings.first().map(|e| e.len()).unwrap_or(64);

                let emb_path = std::path::Path::new(&output_path).with_extension("embeddings.bin");
                match clidex::semantic::save_embeddings(&embeddings, dim, &emb_path) {
                    Ok(()) => eprintln!(
                        "Saved {} embeddings ({}d) to {:?}",
                        embeddings.len(),
                        dim,
                        emb_path
                    ),
                    Err(e) => eprintln!("Warning: Failed to save embeddings: {}", e),
                }
            }
            Err(e) => eprintln!("Warning: Failed to load model2vec: {}", e),
        }
    }

    eprintln!("\nIndex saved to: {}", output_path);
    eprintln!("Total tools: {}", index.tools.len());
    eprintln!(
        "With install: {}",
        index.tools.iter().filter(|t| !t.install.is_empty()).count()
    );
    eprintln!(
        "With stars: {}",
        index.tools.iter().filter(|t| t.stars.is_some()).count()
    );
    eprintln!(
        "With llms.txt: {}",
        index
            .tools
            .iter()
            .filter(|t| t.links.llms_txt.is_some())
            .count()
    );

    // Print category breakdown
    let mut cats: HashMap<String, usize> = HashMap::new();
    for tool in &index.tools {
        let top = tool.category.split(" > ").next().unwrap_or(&tool.category);
        *cats.entry(top.to_string()).or_default() += 1;
    }
    let mut cats_sorted: Vec<_> = cats.into_iter().collect();
    cats_sorted.sort_by(|a, b| b.1.cmp(&a.1));
    eprintln!("\nCategories:");
    for (cat, count) in cats_sorted.iter().take(20) {
        eprintln!("  {:30} {}", cat, count);
    }

    Ok(())
}

fn chrono_now() -> String {
    let output = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    output.trim().to_string()
}

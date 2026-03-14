use clidex::model::{Index, Links, Tool};
use regex::Regex;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

const AWESOME_CLI_APPS_URL: &str =
    "https://raw.githubusercontent.com/agarrharr/awesome-cli-apps/master/readme.md";
const HOMEBREW_FORMULA_URL: &str = "https://formulae.brew.sh/api/formula.json";

#[derive(Debug, Deserialize)]
struct BrewFormula {
    name: String,
    desc: Option<String>,
    homepage: Option<String>,
    #[serde(default)]
    keg_only: bool,
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
                brew_installs_30d: None,
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
                brew_installs_30d: None,
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

/// Enrich tools with GitHub stars
async fn enrich_with_github(tools: &mut [Tool], client: &reqwest::Client, max_requests: usize) {
    let token = std::env::var("GITHUB_TOKEN").ok();
    let mut requests_made = 0;

    for tool in tools.iter_mut() {
        if requests_made >= max_requests {
            eprintln!(
                "GitHub API: reached limit of {} requests, stopping",
                max_requests
            );
            break;
        }

        let repo_url = match &tool.links.repo {
            Some(url) => url.clone(),
            None => continue,
        };

        let (owner, repo) = match parse_github_repo(&repo_url) {
            Some(pair) => pair,
            None => continue,
        };

        let api_url = format!("https://api.github.com/repos/{}/{}", owner, repo);
        requests_made += 1;

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
                    if let Some(stars) = json["stargazers_count"].as_u64() {
                        tool.stars = Some(stars);
                    }
                    if let Some(pushed) = json["pushed_at"].as_str() {
                        tool.last_updated = Some(pushed.to_string());
                    }
                    // Try to get docs URL from homepage if it's not github
                    if tool.links.docs.is_none() {
                        if let Some(hp) = json["homepage"].as_str() {
                            if !hp.is_empty()
                                && !hp.starts_with("https://github.com/")
                                && tool.links.homepage.is_none()
                            {
                                tool.links.homepage = Some(hp.to_string());
                            }
                        }
                    }
                }
            }
            Ok(resp) if resp.status().as_u16() == 403 || resp.status().as_u16() == 429 => {
                eprintln!("GitHub API: rate limited after {} requests", requests_made);
                break;
            }
            Ok(resp) => {
                eprintln!("GitHub API: {} returned {}", api_url, resp.status());
            }
            Err(e) => {
                eprintln!("GitHub API: {} failed: {}", api_url, e);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    eprintln!("GitHub API: made {} requests", requests_made);
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

/// Deduplicate tools by name (case-insensitive, keep the one with more data)
fn deduplicate(tools: &mut Vec<Tool>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut to_remove = Vec::new();

    for (i, tool) in tools.iter().enumerate() {
        let key = tool.name.to_lowercase();
        if let Some(&prev_idx) = seen.get(&key) {
            // Keep the one with more data
            let prev = &tools[prev_idx];
            let prev_score = prev.install.len() + prev.tags.len() + prev.stars.is_some() as usize;
            let cur_score = tool.install.len() + tool.tags.len() + tool.stars.is_some() as usize;
            if cur_score > prev_score {
                to_remove.push(prev_idx);
                seen.insert(key, i);
            } else {
                to_remove.push(i);
            }
        } else {
            seen.insert(key, i);
        }
    }

    to_remove.sort_unstable();
    to_remove.dedup();
    for idx in to_remove.into_iter().rev() {
        tools.remove(idx);
    }
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

    // Deduplicate
    deduplicate(&mut tools);
    eprintln!("After dedup: {} tools", tools.len());

    // Step 3: GitHub stars (limited to avoid rate limiting)
    let github_limit = std::env::var("GITHUB_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    eprintln!(
        "Fetching GitHub stars (limit: {}, set GITHUB_LIMIT to change)...",
        github_limit
    );
    enrich_with_github(&mut tools, &client, github_limit).await;
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

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
const CRATES_IO_API: &str = "https://crates.io/api/v1/crates";
const PYPI_SEARCH_URL: &str = "https://pypi.org/pypi";

#[derive(Debug, Deserialize)]
struct BrewFormula {
    name: String,
    desc: Option<String>,
    homepage: Option<String>,
    #[serde(default)]
    keg_only: bool,
    #[serde(default)]
    urls: serde_json::Value,
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

    let link_re =
        Regex::new(r"^\s*-\s+\[([^\]]+)\]\(([^)]+)\)\s*[-\u{2013}\u{2014}]\s*(.+)$").unwrap();
    let link_no_desc_re = Regex::new(r"^\s*-\s+\[([^\]]+)\]\(([^)]+)\)\s*$").unwrap();
    let multi_link_re = Regex::new(
        r"^\s*-\s+\[([^\]]+)\]\(([^)]+)\),\s*\[([^\]]+)\]\(([^)]+)\)\s*[-\u{2013}\u{2014}]\s*(.+)$",
    )
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
                github_fetched_at: None,
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
            github_fetched_at: None,
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
            format!("python-{name_lower}"),
        ];

        for candidate in &candidates {
            if let Some(formula) = brew_map.get(candidate.as_str()) {
                let matches =
                    same_repository(tool.links.repo.as_deref(), formula.homepage.as_deref())
                        || same_repository(
                            tool.links.repo.as_deref(),
                            formula.urls["stable"]["url"].as_str(),
                        )
                        || tool
                            .links
                            .homepage
                            .as_ref()
                            .zip(formula.homepage.as_ref())
                            .is_some_and(|(a, b)| {
                                a.trim_end_matches('/') == b.trim_end_matches('/')
                            });
                if !matches {
                    continue;
                }
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
                github_fetched_at: None,
            });
        }
    }
}

fn parse_github_repo(url: &str) -> Option<(String, String)> {
    let url = reqwest::Url::parse(url.trim().trim_start_matches("git+")).ok()?;
    if url.host_str()? != "github.com" {
        return None;
    }
    let mut parts = url.path_segments()?;
    let owner = parts.next()?.to_lowercase();
    let repo = parts.next()?.trim_end_matches(".git").to_lowercase();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo))
}

fn same_repository(a: Option<&str>, b: Option<&str>) -> bool {
    a.and_then(parse_github_repo)
        .zip(b.and_then(parse_github_repo))
        .is_some_and(|(a, b)| a == b)
}

fn load_stars_cache(path: &str) -> HashMap<(String, String), Tool> {
    let index: Index = match std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_yaml::from_str(&content).ok())
    {
        Some(index) => index,
        None => return HashMap::new(),
    };
    index
        .tools
        .into_iter()
        .filter_map(|tool| {
            let repo = parse_github_repo(tool.links.repo.as_deref()?)?;
            tool.stars?;
            Some((repo, tool))
        })
        .collect()
}

fn is_cache_fresh(fetched_at: Option<u64>, now: u64, max_age_days: u64) -> bool {
    fetched_at
        .and_then(|fetched| now.checked_sub(fetched))
        .is_some_and(|age| age < max_age_days.saturating_mul(86400))
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

/// Enrich tools with GitHub stars using a previous-index cache and parallel fetching.
async fn enrich_with_github(
    tools: &mut [Tool],
    client: &reqwest::Client,
    max_requests: usize,
    stars_cache: &HashMap<(String, String), Tool>,
    cache_max_age_days: u64,
) {
    let token = std::env::var("GITHUB_TOKEN").ok();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut to_fetch = Vec::new();
    for (i, tool) in tools.iter_mut().enumerate() {
        let Some(repo) = tool.links.repo.as_deref().and_then(parse_github_repo) else {
            continue;
        };
        if let Some(cached) = stars_cache.get(&repo) {
            tool.stars = cached.stars;
            tool.last_updated = cached.last_updated.clone();
            tool.github_fetched_at = cached.github_fetched_at;
            if tool.links.homepage.is_none() {
                tool.links.homepage = cached.links.homepage.clone();
            }
        }
        if !is_cache_fresh(tool.github_fetched_at, now, cache_max_age_days)
            && to_fetch.len() < max_requests
        {
            to_fetch.push((i, repo));
        }
    }
    let mut fetched = 0;
    for chunk in to_fetch.chunks(10) {
        let results = futures::future::join_all(chunk.iter().map(|(_, (owner, repo))| {
            fetch_single_github(owner.clone(), repo.clone(), token.clone(), client.clone())
        }))
        .await;
        for (result, (idx, _)) in results.into_iter().zip(chunk) {
            if let Some((stars, pushed, homepage)) = result {
                let tool = &mut tools[*idx];
                tool.stars = Some(stars);
                tool.last_updated = pushed;
                tool.github_fetched_at = Some(now);
                if tool.links.homepage.is_none() {
                    tool.links.homepage = homepage;
                }
                fetched += 1;
            }
        }
    }
    eprintln!(
        "GitHub stars: {} refreshed; cached values retained on failure",
        fetched
    );
}

fn crate_binary(data: &serde_json::Value) -> Option<String> {
    let version = data["crate"]["max_stable_version"]
        .as_str()
        .or_else(|| data["crate"]["max_version"].as_str())?;
    data["versions"]
        .as_array()?
        .iter()
        .find(|v| v["num"] == version)?["bin_names"]
        .as_array()?
        .iter()
        .find_map(|name| name.as_str().filter(|s| !s.is_empty()).map(String::from))
}

fn npm_binary(data: &serde_json::Value) -> Option<String> {
    match &data["bin"] {
        serde_json::Value::String(path) if !path.is_empty() => {
            data["name"].as_str()?.rsplit('/').next().map(String::from)
        }
        serde_json::Value::Object(bins) => bins
            .iter()
            .find(|(name, path)| !name.is_empty() && path.as_str().is_some_and(|s| !s.is_empty()))
            .map(|(name, _)| name.clone()),
        _ => None,
    }
}

fn npm_repository(data: &serde_json::Value) -> Option<&str> {
    data["repository"]["url"]
        .as_str()
        .or_else(|| data["repository"].as_str())
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

    let mut candidates: Vec<_> = tools.iter_mut().collect();
    candidates.sort_by_key(|tool| !crate_overrides.contains_key(tool.name.to_lowercase().as_str()));
    for tool in candidates {
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
                    if let Some(binary) = crate_binary(&json) {
                        if same_repository(
                            tool.links.repo.as_deref(),
                            json["crate"]["repository"].as_str(),
                        ) {
                            tool.install.insert(
                                "cargo".to_string(),
                                format!("cargo install {}", crate_name),
                            );
                            tool.binary.get_or_insert(binary);
                            found += 1;
                        }
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
        let api_url = format!("https://registry.npmjs.org/{}/latest", pkg);
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
                    if let Some(binary) = npm_binary(&json) {
                        if same_repository(tool.links.repo.as_deref(), npm_repository(&json)) {
                            tool.install
                                .insert("npm".to_string(), format!("npm install -g {}", pkg));
                            tool.binary.get_or_insert(binary);
                            found += 1;
                        }
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

/// Manually curated tools that aren't discoverable via automated data sources.
/// These are app-bundled CLIs, proprietary tools, or tools with unusual distribution.
fn add_manual_tools(tools: &mut Vec<Tool>) {
    let existing: std::collections::HashSet<String> =
        tools.iter().map(|t| t.name.to_lowercase()).collect();

    let manual: Vec<Tool> = vec![
        Tool {
            name: "terraform".to_string(),
            binary: Some("terraform".to_string()),
            desc: "Infrastructure as code CLI for building, changing, and versioning cloud and on-premises resources".to_string(),
            category: "Development > Cloud Infrastructure".to_string(),
            tags: vec![
                "infrastructure-as-code", "iac", "cloud", "provisioning",
                "automation", "aws", "azure", "gcp", "terraform",
            ].into_iter().map(|s| s.to_string()).collect(),
            install: {
                let mut m = BTreeMap::new();
                m.insert(
                    "brew".to_string(),
                    "brew install hashicorp/tap/terraform".to_string(),
                );
                m
            },
            stars: None,
            brew_installs_365d: None,
            links: Links {
                repo: Some("https://github.com/hashicorp/terraform".to_string()),
                homepage: Some("https://developer.hashicorp.com/terraform".to_string()),
                docs: Some("https://developer.hashicorp.com/terraform/docs".to_string()),
                llms_txt: None,
            },
            last_updated: None,
            github_fetched_at: None,
        },
        Tool {
            name: "obsidian".to_string(),
            binary: Some("obsidian".to_string()),
            desc: "Knowledge base CLI for managing vaults, notes, daily notes, search, tasks, tags, properties, and plugins from the terminal".to_string(),
            category: "Productivity > Note Taking".to_string(),
            tags: vec![
                "notes", "knowledge-base", "vault", "markdown", "daily-notes",
                "pkm", "zettelkasten", "wiki", "search", "tasks", "tags",
                "bookmarks", "sync", "plugins", "obsidian",
            ].into_iter().map(|s| s.to_string()).collect(),
            install: {
                let mut m = BTreeMap::new();
                m.insert("brew".to_string(), "brew install --cask obsidian".to_string());
                m
            },
            stars: None,
            brew_installs_365d: None,
            links: Links {
                repo: None,
                homepage: Some("https://obsidian.md".to_string()),
                docs: Some("https://help.obsidian.md".to_string()),
                llms_txt: None,
            },
            last_updated: None,
            github_fetched_at: None,
        },
    ];

    let mut added = 0;
    for tool in manual {
        if !existing.contains(&tool.name.to_lowercase()) {
            tools.push(tool);
            added += 1;
        }
    }
    if added > 0 {
        eprintln!("Added {} manually curated tools", added);
    }
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
    // Phase 1: collect merge pairs, then merge by repository URL.
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
        if base.binary.is_none() {
            base.binary = other.binary;
        }
        if base.last_updated.is_none() {
            base.last_updated = other.last_updated;
        }
        if base.github_fetched_at.is_none() {
            base.github_fetched_at = other.github_fetched_at;
        }
        if base.links.llms_txt.is_none() {
            base.links.llms_txt = other.links.llms_txt;
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
            if let (Some(a), Some(b)) = (&prev.links.repo, &tool.links.repo) {
                if !same_repository(Some(a), Some(b))
                    && a.trim_end_matches('/') != b.trim_end_matches('/')
                {
                    continue;
                }
            }
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
        // --- Containers & Orchestration ---
        (
            &["docker", "container", "podman", "image layer"],
            "Development > Docker",
        ),
        (
            &["kubernetes", "k8s", "kubectl", "helm"],
            "Development > Kubernetes",
        ),
        // --- Version Control ---
        (
            &["git ", "git-", "commit", "branch", "version control"],
            "Version Control > Git",
        ),
        // --- Development Tools ---
        (
            &[
                "linter",
                "lint ",
                "linting",
                "code quality",
                "static analysis",
            ],
            "Development > Linting",
        ),
        (
            &[
                "formatter",
                "formatting",
                "prettify",
                "beautif",
                "code format",
            ],
            "Development > Formatting",
        ),
        (
            &["http client", "http request", "curl", "api client"],
            "Development > HTTP Client",
        ),
        (
            &["http test", "api test", "load test", "stress test"],
            "Development > Testing",
        ),
        (
            &["test ", "testing", "test framework", "unit test", "spec "],
            "Development > Testing",
        ),
        (
            &["benchmark", "timing", "performance", "profil"],
            "Development > Benchmarking",
        ),
        (
            &["editor", "text editor", "vim", "neovim", "nano", "ide "],
            "Development > Editor",
        ),
        (
            &["diff", "compare", "merge", "patch "],
            "Development > Diff",
        ),
        (
            &["debug", "debugger", "breakpoint", "inspect"],
            "Development > Debugging",
        ),
        (
            &["ci/cd", "ci cd", "deploy", "devops", "release", "pipeline"],
            "Development > DevOps",
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
            &[
                "database", "sql", "sqlite", "postgres", "mysql", "redis", "mongo",
            ],
            "Development > Database",
        ),
        (
            &[
                "compiler",
                "transpil",
                "build tool",
                "build system",
                "makefile",
            ],
            "Development > Build",
        ),
        (
            &["documentation", "doc gen", "docstring", "apidoc"],
            "Development > Documentation",
        ),
        (
            &["repl ", "interactive interpreter", "playground"],
            "Development > REPL",
        ),
        (
            &[
                "scaffold",
                "boilerplate",
                "template",
                "cookiecutter",
                "project init",
            ],
            "Development > Scaffolding",
        ),
        (
            &["api ", "rest ", "graphql", "grpc", "protobuf"],
            "Development > API",
        ),
        // --- Files and Directories ---
        (
            &["file manager", "file explorer", "file browser"],
            "Files and Directories > File Managers",
        ),
        (
            &["search", "grep", "find files", "regex", "code search"],
            "Files and Directories > Search",
        ),
        (
            &["rename", "batch rename", "bulk rename"],
            "Files and Directories > Renaming",
        ),
        (
            &["file sync", "rsync", "backup", "mirror", "replicate"],
            "Files and Directories > Sync",
        ),
        (
            &["ls ", "list files", "list director", "directory list"],
            "Files and Directories > Listing",
        ),
        (
            &["cat ", "file viewer", "pager", "less ", "more "],
            "Files and Directories > Viewer",
        ),
        (
            &["tree ", "directory tree", "folder tree"],
            "Files and Directories > Tree",
        ),
        (
            &["file transfer", "upload", "scp ", "sftp"],
            "Files and Directories > Transfer",
        ),
        // --- Shell ---
        (
            &["shell history", "history search"],
            "Utilities > Shell History",
        ),
        (&["shell ", "bash ", "zsh ", "fish "], "Utilities > Shell"),
        (
            &["prompt", "starship", "powerline", "ps1"],
            "Utilities > Shell Prompt",
        ),
        (&["alias", "abbreviation", "shortcut"], "Utilities > Shell"),
        // --- System ---
        (
            &["monitor", "system monitor", "top ", "htop", "process"],
            "Utilities > System Monitoring",
        ),
        (
            &["disk usage", "disk space", " du "],
            "Utilities > Disk Usage",
        ),
        (
            &[
                "system info",
                "sysinfo",
                "hardware info",
                "cpu info",
                "neofetch",
            ],
            "Utilities > System Info",
        ),
        (
            &["cron", "schedule", "periodic", "timer"],
            "Utilities > Scheduling",
        ),
        (
            &["service", "daemon", "systemd", "init "],
            "Utilities > Services",
        ),
        // --- Data ---
        (
            &[
                "json",
                "yaml",
                "toml",
                "csv",
                "data process",
                "data transform",
            ],
            "Data Manipulation > Processors",
        ),
        (
            &["xml ", "xpath", "xslt", "html pars"],
            "Data Manipulation > XML",
        ),
        (
            &["convert", "transform", "translate", "encode", "decode"],
            "Data Manipulation > Conversion",
        ),
        // --- Security ---
        (
            &[
                "encrypt", "decrypt", "cipher", "crypto", "gpg", "age ", "tls", "ssl",
            ],
            "Security > Encryption",
        ),
        (
            &["password", "secret", "vault", "credential", "keychain"],
            "Security > Password",
        ),
        (
            &["scan", "vulnerab", "cve", "audit", "security scan"],
            "Security > Scanning",
        ),
        (
            &["firewall", "iptables", "nftables", "packet filter"],
            "Security > Firewall",
        ),
        (
            &["auth", "oauth", "jwt", "token", "saml"],
            "Security > Authentication",
        ),
        // --- Network ---
        (
            &["network", "dns", "ping", "traceroute", "ip ", "subnet"],
            "Utilities > Networking",
        ),
        (&["ssh ", "ssh-", "remote", "tunnel"], "Utilities > SSH"),
        (
            &["proxy", "vpn", "socks", "reverse proxy"],
            "Utilities > Proxy",
        ),
        (
            &["port scan", "nmap", "netcat", "socket"],
            "Utilities > Network Tools",
        ),
        (
            &["bandwidth", "speed test", "throughput", "traffic"],
            "Utilities > Network Monitor",
        ),
        // --- Terminal ---
        (
            &["terminal emulator", "term emu"],
            "Utilities > Terminal Emulator",
        ),
        (
            &["multiplexer", "tmux", "terminal workspace", "session"],
            "Utilities > Terminal Multiplexer",
        ),
        (
            &[
                "tui ",
                "terminal ui",
                "terminal interface",
                "ncurses",
                "textual",
            ],
            "Utilities > TUI",
        ),
        (
            &["color", "colour", "ansi", "theme", "syntax highlight"],
            "Utilities > Terminal Styling",
        ),
        // --- Utilities ---
        (
            &["download", "wget", "fetch", "scrape", "crawl"],
            "Utilities > Download",
        ),
        (
            &[
                "compress",
                "decompress",
                "archive",
                "zip",
                "tar",
                "gzip",
                "brotli",
                "zstd",
            ],
            "Utilities > Compression",
        ),
        (
            &["clipboard", "copy", "paste", "pbcopy"],
            "Utilities > Clipboard",
        ),
        (
            &[
                "log ",
                "logging",
                "log viewer",
                "log file",
                "structured log",
            ],
            "Utilities > Log Viewer",
        ),
        (
            &[
                "hex ",
                "hex viewer",
                "hexdump",
                "binary viewer",
                "binary editor",
            ],
            "Utilities > Hex Viewer",
        ),
        (
            &["watch ", "file watch", "file change", "inotify", "fswatch"],
            "Utilities > File Watching",
        ),
        (
            &["calculator", "calc ", "math", "arithmetic", "compute"],
            "Utilities > Calculator",
        ),
        (
            &["clock", "time ", "timezone", "date ", "calendar"],
            "Utilities > Time",
        ),
        (
            &["weather", "forecast", "temperature"],
            "Utilities > Weather",
        ),
        (
            &["notification", "alert", "notify"],
            "Utilities > Notification",
        ),
        (&["qrcode", "barcode", "qr "], "Utilities > QR Code"),
        // --- Media ---
        (
            &[
                "image",
                "photo",
                "picture",
                "png",
                "jpg",
                "svg",
                "screenshot",
            ],
            "Utilities > Image Processing",
        ),
        (
            &[
                "video",
                "media player",
                "stream",
                "mp4",
                "ffmpeg",
                "transcode",
            ],
            "Utilities > Media",
        ),
        (
            &["audio", "music", "sound", "mp3", "podcast", "radio"],
            "Utilities > Audio",
        ),
        (&["pdf ", "pdf-", "portable document"], "Utilities > PDF"),
        // --- Productivity ---
        (
            &["note", "todo", "task", "productivity", "kanban"],
            "Productivity > Note Taking",
        ),
        (&["presentation", "slides"], "Productivity > Presentations"),
        (
            &["email", "mail ", "smtp", "imap", "inbox"],
            "Productivity > Email",
        ),
        (
            &["chat ", "messaging", "slack", "discord", "irc"],
            "Productivity > Chat",
        ),
        (
            &["calendar", "event", "schedule", "agenda"],
            "Productivity > Calendar",
        ),
        (
            &["bookmark", "link manager", "url "],
            "Productivity > Bookmarks",
        ),
        // --- Document ---
        (&["markdown", "md "], "Utilities > Markdown"),
        (
            &["typesett", "latex", "tex ", "document", "pandoc"],
            "Utilities > Document Processing",
        ),
        (
            &["spell", "grammar", "typo", "proofread"],
            "Development > Spell Check",
        ),
        (
            &["diagram", "graph ", "chart", "plot", "visuali"],
            "Utilities > Visualization",
        ),
        // --- AI ---
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
                "machine learn",
            ],
            "AI > LLM Interaction",
        ),
        // --- Cloud & Infrastructure ---
        (&["aws ", "amazon", "s3 "], "Cloud > AWS"),
        (&["gcloud", "google cloud", "gcp"], "Cloud > GCP"),
        (&["azure", "microsoft cloud"], "Cloud > Azure"),
        (
            &[
                "terraform",
                "pulumi",
                "cloudformation",
                "infrastructure as code",
            ],
            "Cloud > IaC",
        ),
        (
            &["serverless", "lambda", "function as a service"],
            "Cloud > Serverless",
        ),
        // --- Misc ---
        (
            &["game", "entertainment", "fun ", "ascii art", "screensaver"],
            "Entertainment > Games",
        ),
        (
            &[
                "finance", "stock", "crypto", "bitcoin", "trading", "currency",
            ],
            "Utilities > Finance",
        ),
        (
            &["education", "learning", "tutorial", "flashcard"],
            "Utilities > Education",
        ),
        (&["font ", "typography"], "Utilities > Fonts"),
        (
            &["config", "dotfile", "settings", "preference"],
            "Utilities > Configuration",
        ),
        (
            &["man ", "manpage", "help ", "cheatsheet", "tldr"],
            "Utilities > Help",
        ),
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
        let library_only = ["library", "libraries", "bindings", "header files"]
            .iter()
            .any(|kw| desc.contains(kw))
            && ![" tool", "command-line", "command line", "utility"]
                .iter()
                .any(|kw| desc.contains(kw));
        if library_only || hard_exclude.iter().any(|kw| desc.contains(kw)) {
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
            github_fetched_at: None,
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
            github_fetched_at: None,
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

                let metadata = match client
                    .get(format!("https://registry.npmjs.org/{name}/latest"))
                    .send()
                    .await
                {
                    Ok(response) if response.status().is_success() => {
                        response.json::<serde_json::Value>().await.ok()
                    }
                    _ => None,
                };
                let Some(metadata) = metadata else {
                    continue;
                };
                let Some(binary) = npm_binary(&metadata) else {
                    continue;
                };
                let homepage = metadata["homepage"].as_str().map(String::from);
                let repo_url = npm_repository(&metadata).map(String::from);

                let category = auto_categorize(&desc, display_name);
                let tags = generate_tags(display_name, &desc, &category);

                let mut install = BTreeMap::new();
                install.insert("npm".to_string(), format!("npm install -g {}", name));

                existing.push(Tool {
                    name: display_name.to_string(),
                    binary: Some(binary),
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
                    github_fetched_at: None,
                });
                added += 1;
            }
        }
    }
    eprintln!("Discovered {} CLI packages from npm", added);
}

/// Discover new CLI tools from crates.io by searching categories/keywords.
/// Unlike enrich_with_crates_io (which adds `cargo install` to existing tools),
/// this function discovers tools that aren't in the index yet.
async fn discover_from_crates_io(existing: &mut Vec<Tool>, client: &reqwest::Client) {
    let existing_names: std::collections::HashSet<String> =
        existing.iter().map(|t| t.name.to_lowercase()).collect();

    let categories = ["command-line-utilities"];
    let mut added = 0;
    let per_category = 25; // top 25 per category by downloads

    for category in &categories {
        let url = format!(
            "{}?category={}&per_page={}&sort=downloads",
            CRATES_IO_API, category, per_category
        );
        let resp = match client
            .get(&url)
            .header(
                "User-Agent",
                "clidex-build/0.2 (https://github.com/syshin0116/clidex)",
            )
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            Ok(r) if r.status().as_u16() == 429 => {
                eprintln!("crates.io discovery: rate limited at category {}", category);
                break;
            }
            _ => continue,
        };

        let data: serde_json::Value = match resp.json().await {
            Ok(d) => d,
            Err(_) => continue,
        };

        if let Some(crates) = data["crates"].as_array() {
            for krate in crates {
                let name = match krate["id"].as_str() {
                    Some(n) => n,
                    None => continue,
                };

                // Use the crate name as display name, skip if already exists
                if existing_names.contains(&name.to_lowercase()) {
                    continue;
                }

                let desc = krate["description"]
                    .as_str()
                    .unwrap_or("")
                    .to_string()
                    .replace('\n', " ");
                if desc.is_empty() {
                    continue;
                }

                let metadata = match client
                    .get(format!("{CRATES_IO_API}/{name}"))
                    .header(
                        "User-Agent",
                        "clidex-build (https://github.com/syshin0116/clidex)",
                    )
                    .send()
                    .await
                {
                    Ok(response) if response.status().is_success() => {
                        response.json::<serde_json::Value>().await.ok()
                    }
                    _ => None,
                };
                let Some(metadata) = metadata else {
                    continue;
                };
                let Some(binary) = crate_binary(&metadata) else {
                    continue;
                };

                let repo = krate["repository"].as_str().map(String::from);
                let homepage = krate["homepage"].as_str().map(String::from);
                let downloads = krate["downloads"].as_u64().unwrap_or(0);

                // Skip low-download crates
                if downloads < 10000 {
                    continue;
                }

                let category_str = auto_categorize(&desc, name);
                let tags = generate_tags(name, &desc, &category_str);

                let mut install = BTreeMap::new();
                install.insert("cargo".to_string(), format!("cargo install {}", name));

                existing.push(Tool {
                    name: name.to_string(),
                    binary: Some(binary),
                    desc,
                    category: category_str,
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
                    github_fetched_at: None,
                });
                added += 1;
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    eprintln!("Discovered {} new CLI tools from crates.io", added);
}

/// Discover CLI tools from PyPI.
/// Uses a seed list of well-known Python CLI tools + searches for popular console_scripts packages.
async fn discover_from_pypi(existing: &mut Vec<Tool>, client: &reqwest::Client) {
    let existing_names: std::collections::HashSet<String> =
        existing.iter().map(|t| t.name.to_lowercase()).collect();

    // Seed list: popular Python CLI tools that might not be in other sources
    let seed_tools: &[(&str, &str)] = &[
        ("uv", "uv"),
        ("ruff", "ruff"),
        ("mypy", "mypy"),
        ("pytest", "pytest"),
        ("tox", "tox"),
        ("pre-commit", "pre-commit"),
        ("cookiecutter", "cookiecutter"),
        ("rich-cli", "rich-cli"),
        ("textual", "textual"),
        ("litecli", "litecli"),
        ("mycli", "mycli"),
        ("iredis", "iredis"),
        ("ranger-fm", "ranger-fm"),
        ("thefuck", "thefuck"),
        ("howdoi", "howdoi"),
        ("asciinema", "asciinema"),
        ("ansible", "ansible"),
        ("httpx", "httpx"),
        ("supervisor", "supervisor"),
        ("twine", "twine"),
        ("bandit", "bandit"),
        ("flake8", "flake8"),
        ("isort", "isort"),
        ("pylint", "pylint"),
        ("sphinx", "sphinx"),
        ("mkdocs", "mkdocs"),
        ("jupyterlab", "jupyterlab"),
        ("streamlit", "streamlit"),
        ("dvc", "dvc"),
        ("copier", "copier"),
        ("nox", "nox"),
        ("pdm", "pdm"),
        ("hatch", "hatch"),
        ("maturin", "maturin"),
        ("bpython", "bpython"),
        ("ptpython", "ptpython"),
        ("grip", "grip"),
        ("dooit", "dooit"),
        ("posting", "posting"),
    ];

    let mut added = 0;

    for (name, pypi_name) in seed_tools {
        if existing_names.contains(&name.to_lowercase()) {
            continue;
        }

        let url = format!("{}/{}/json", PYPI_SEARCH_URL, pypi_name);
        let resp = match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => r,
            _ => continue,
        };

        let data: serde_json::Value = match resp.json().await {
            Ok(d) => d,
            Err(_) => continue,
        };

        let info = &data["info"];
        let desc = info["summary"].as_str().unwrap_or("").to_string();
        if desc.is_empty() {
            continue;
        }

        let homepage = info["home_page"]
            .as_str()
            .filter(|h| !h.is_empty())
            .map(String::from);
        let project_url = info["project_url"].as_str().map(String::from);
        let repo = info["project_urls"]["Source"]
            .as_str()
            .or_else(|| info["project_urls"]["Repository"].as_str())
            .or_else(|| info["project_urls"]["GitHub"].as_str())
            .or_else(|| info["project_urls"]["Homepage"].as_str())
            .map(String::from)
            .or_else(|| {
                homepage
                    .as_ref()
                    .filter(|h| h.contains("github.com"))
                    .cloned()
            });

        let category = auto_categorize(&desc, name);
        let tags = generate_tags(name, &desc, &category);

        let mut install = BTreeMap::new();
        install.insert("pipx".to_string(), format!("pipx install {}", pypi_name));

        existing.push(Tool {
            name: name.to_string(),
            binary: None,
            desc,
            category,
            tags,
            install,
            stars: None,
            brew_installs_365d: None,
            links: Links {
                repo,
                homepage: homepage.or(project_url),
                docs: None,
                llms_txt: None,
            },
            last_updated: None,
            github_fetched_at: None,
        });
        added += 1;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    eprintln!("Discovered {} new CLI tools from PyPI", added);
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
        .error_for_status()?
        .text()
        .await?;
    eprintln!("Fetched {} bytes", awesome_md.len());

    let mut tools = parse_awesome_cli_apps(&awesome_md);
    eprintln!("Parsed {} tools from awesome-cli-apps", tools.len());

    // Step 1b: Fetch toolleeo/cli-apps
    eprintln!("Fetching toolleeo/cli-apps...");
    let toolleeo_apps = client
        .get(TOOLLEEO_APPS_URL)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let toolleeo_cats = client
        .get(TOOLLEEO_CATEGORIES_URL)
        .send()
        .await?
        .error_for_status()?
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
    let brew_resp = brew_client
        .get(HOMEBREW_FORMULA_URL)
        .send()
        .await?
        .error_for_status()?;
    let brew_data: Vec<BrewFormula> = brew_resp.json().await?;
    eprintln!("Fetched {} Homebrew formulae", brew_data.len());

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

    // Step 2d: Discover CLI tools from crates.io (new tools, not just enrichment)
    eprintln!("Discovering CLI tools from crates.io categories...");
    discover_from_crates_io(&mut tools, &client).await;

    // Step 2e: Discover CLI tools from PyPI
    eprintln!("Discovering CLI tools from PyPI...");
    discover_from_pypi(&mut tools, &client).await;

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

    enrich_with_homebrew(&mut tools, &brew_data);
    let brew_matched = tools
        .iter()
        .filter(|t| t.install.contains_key("brew"))
        .count();
    eprintln!("Matched {} tools with Homebrew", brew_matched);

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

    // Step 6b: Add manually curated tools not discoverable via data sources
    add_manual_tools(&mut tools);

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

    #[cfg(feature = "semantic")]
    {
        let model = model2vec_rs::model::StaticModel::from_pretrained(
            clidex::semantic::MODEL_ID,
            None,
            None,
            None,
        )?;
        let texts: Vec<_> = index
            .tools
            .iter()
            .map(clidex::semantic::embedding_text)
            .collect();
        let embeddings = model.encode(&texts);
        let emb_path = std::path::Path::new(&output_path).with_extension("embeddings.bin");
        clidex::semantic::save_tool_embeddings(&embeddings, &index.tools, &emb_path)?;
        eprintln!("Saved semantic embeddings to {}", emb_path.display());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn metadata_validation_rejects_libraries_and_preserves_stale_stars() {
        assert_eq!(npm_binary(&serde_json::json!({"name":"chalk"})), None);
        assert_eq!(
            npm_binary(&serde_json::json!({"name":"@scope/cli", "bin":{"run-cli":"index.js"}})),
            Some("run-cli".into())
        );
        assert_eq!(
            crate_binary(
                &serde_json::json!({"crate":{"max_stable_version":"1.0"},"versions":[{"num":"1.0","bin_names":[]}]})
            ),
            None
        );
        assert_eq!(
            crate_binary(
                &serde_json::json!({"crate":{"max_stable_version":"1.0"},"versions":[{"num":"1.0","bin_names":["rg"]}]})
            ),
            Some("rg".into())
        );
        assert!(same_repository(
            Some("git+https://github.com/Owner/Repo.git"),
            Some("https://github.com/owner/repo/releases/v1")
        ));
        assert!(!same_repository(
            Some("https://github.com/kislyuk/yq"),
            Some("https://github.com/mikefarah/yq")
        ));
        let mut tool: Tool = serde_json::from_value(serde_json::json!({
            "name":"yq", "desc":"YAML processor", "category":"Data",
            "links":{"repo":"https://github.com/kislyuk/yq"}
        }))
        .unwrap();
        let formula: BrewFormula = serde_json::from_value(serde_json::json!({
            "name":"yq", "homepage":"https://github.com/mikefarah/yq"
        }))
        .unwrap();
        enrich_with_homebrew(std::slice::from_mut(&mut tool), &[formula]);
        assert!(tool.install.is_empty());
        let correct: BrewFormula = serde_json::from_value(serde_json::json!({
            "name":"python-yq", "homepage":"https://github.com/kislyuk/yq"
        }))
        .unwrap();
        enrich_with_homebrew(std::slice::from_mut(&mut tool), &[correct]);
        assert_eq!(tool.install["brew"], "brew install python-yq");
        let formulae: Vec<BrewFormula> = serde_json::from_value(serde_json::json!([
            {"name":"vegeta","desc":"HTTP load testing tool and library","homepage":"https://github.com/tsenart/vegeta"},
            {"name":"libuv","desc":"Multi-platform support library","homepage":"https://github.com/libuv/libuv"}
        ])).unwrap();
        let mut discovered = Vec::new();
        discover_from_homebrew(&mut discovered, &formulae, &HashMap::new());
        assert_eq!(
            discovered
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["vegeta"]
        );

        let mut other = tool.clone();
        other.links.repo = Some("https://github.com/mikefarah/yq".into());
        let mut distinct = vec![tool.clone(), other];
        deduplicate(&mut distinct);
        assert_eq!(distinct.len(), 2);
        assert!(is_cache_fresh(Some(1000), 1001, 3));
        assert!(!is_cache_fresh(Some(1000), 1000 + 3 * 86400, 3));
        assert!(!is_cache_fresh(None, 1001, 3));
        tool.stars = Some(42);
        tool.last_updated = Some("2020-01-01T00:00:00Z".into());
        let cache = HashMap::from([(("kislyuk".into(), "yq".into()), tool.clone())]);
        tool.stars = None;
        enrich_with_github(
            std::slice::from_mut(&mut tool),
            &reqwest::Client::new(),
            0,
            &cache,
            3,
        )
        .await;
        assert_eq!(tool.stars, Some(42));
        assert_eq!(tool.last_updated.as_deref(), Some("2020-01-01T00:00:00Z"));
        assert_eq!(tool.github_fetched_at, None);
    }

    #[test]
    fn add_manual_tools_includes_required_terraform_metadata() {
        let mut tools = Vec::new();

        add_manual_tools(&mut tools);

        let terraform = tools
            .iter()
            .find(|tool| tool.name == "terraform")
            .expect("terraform should be present in every generated index");
        assert_eq!(terraform.binary.as_deref(), Some("terraform"));
        assert_eq!(
            terraform.install.get("brew").map(String::as_str),
            Some("brew install hashicorp/tap/terraform")
        );
        assert_eq!(
            terraform.links.repo.as_deref(),
            Some("https://github.com/hashicorp/terraform")
        );
    }

    #[test]
    fn add_manual_tools_does_not_duplicate_existing_terraform() {
        let mut tools = vec![Tool {
            name: "Terraform".to_string(),
            binary: Some("terraform".to_string()),
            desc: "existing".to_string(),
            category: "Cloud".to_string(),
            tags: Vec::new(),
            install: BTreeMap::new(),
            stars: None,
            brew_installs_365d: None,
            links: Links::default(),
            last_updated: None,
            github_fetched_at: None,
        }];

        add_manual_tools(&mut tools);

        assert_eq!(
            tools
                .iter()
                .filter(|tool| tool.name.eq_ignore_ascii_case("terraform"))
                .count(),
            1
        );
    }
}

use clap::{Parser, Subcommand};
use clidex::config;
use clidex::index;
use clidex::output::{self, Format};
use clidex::search;

#[derive(Parser)]
#[command(name = "clidex", version, about = "CLI tool discovery for AI agents")]
struct Cli {
    /// Search query (e.g. "csv to json")
    query: Option<String>,

    /// Force pretty-printed table output (human-friendly)
    #[arg(long)]
    pretty: bool,

    /// Force JSON output
    #[arg(long)]
    json: bool,

    /// Force YAML output (default for non-TTY)
    #[arg(long)]
    yaml: bool,

    /// Filter by category (supports fuzzy matching)
    #[arg(long)]
    category: Option<String>,

    /// List all categories
    #[arg(long)]
    categories: bool,

    /// Maximum number of results
    #[arg(short = 'n', long, default_value_t = config::DEFAULT_MAX_RESULTS)]
    max_results: usize,

    /// Include relevance scores in output
    #[arg(long)]
    score: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for CLI tools (same as positional query)
    Search {
        /// Search query
        query: String,
        /// Force pretty-printed table output
        #[arg(long)]
        pretty: bool,
        /// Force JSON output
        #[arg(long)]
        json: bool,
        /// Force YAML output
        #[arg(long)]
        yaml: bool,
        /// Filter by category
        #[arg(long)]
        category: Option<String>,
        /// Maximum number of results
        #[arg(short = 'n', long, default_value_t = config::DEFAULT_MAX_RESULTS)]
        max_results: usize,
        /// Include relevance scores
        #[arg(long)]
        score: bool,
    },
    /// Show detailed info about a tool
    Info {
        /// Tool name
        name: String,
        /// Force pretty-printed table output
        #[arg(long)]
        pretty: bool,
        /// Force JSON output
        #[arg(long)]
        json: bool,
        /// Force YAML output
        #[arg(long)]
        yaml: bool,
    },
    /// Download/update the tool index
    Update,
    /// Show index statistics
    Stats,
    /// List categories (with optional filter)
    Categories {
        /// Filter categories by name
        filter: Option<String>,
        /// Force pretty-printed table output
        #[arg(long)]
        pretty: bool,
        /// Force JSON output
        #[arg(long)]
        json: bool,
        /// Force YAML output
        #[arg(long)]
        yaml: bool,
    },
    /// Compare tools side by side
    Compare {
        /// Tool names to compare
        names: Vec<String>,
        /// Force pretty-printed table output
        #[arg(long)]
        pretty: bool,
        /// Force JSON output
        #[arg(long)]
        json: bool,
        /// Force YAML output
        #[arg(long)]
        yaml: bool,
    },
    /// Show popular tools (sorted by GitHub stars)
    Trending {
        /// Category to filter by
        #[arg(long)]
        category: Option<String>,
        /// Only include tools updated after this date (YYYY-MM-DD). Filters by repo activity, not popularity growth.
        #[arg(long)]
        updated_since: Option<String>,
        /// Maximum number of results
        #[arg(short = 'n', long, default_value_t = 20)]
        max_results: usize,
        /// Force pretty-printed table output
        #[arg(long)]
        pretty: bool,
        /// Force JSON output
        #[arg(long)]
        json: bool,
        /// Force YAML output
        #[arg(long)]
        yaml: bool,
    },
}

/// Detect output format: explicit flags > TTY detection > YAML default.
fn get_format(pretty: bool, json: bool, yaml: bool) -> Format {
    if pretty {
        Format::Pretty
    } else if json {
        Format::Json
    } else if yaml {
        Format::Yaml
    } else if atty::is(atty::Stream::Stdout) {
        Format::Pretty // TTY → human-friendly by default
    } else {
        Format::Yaml // pipe/redirect → machine-readable
    }
}

/// Load index, auto-downloading on first run if interactive TTY.
/// Non-interactive (pipes, CI) gets an error with instructions instead.
async fn load_or_download() -> Result<clidex::model::Index, String> {
    match index::load_index() {
        Ok(i) => Ok(i),
        Err(_) if !config::index_path().exists() => {
            if atty::is(atty::Stream::Stdin) {
                // Interactive terminal — auto-download
                eprintln!("Index not found. Downloading from {}...", config::INDEX_URL);
                let count = index::update_index().await?;
                eprintln!("Index downloaded: {count} tools");
                index::load_index()
            } else {
                // Non-interactive (CI, pipe) — don't make network calls silently
                Err(format!(
                    "Index not found at {}. Run `clidex update` first.",
                    config::index_path().display()
                ))
            }
        }
        Err(e) => Err(e),
    }
}

fn load_or_exit_sync() -> clidex::model::Index {
    match index::load_index() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

/// Suggest similar tools when search returns empty results.
fn suggest_on_empty(query: &str, tools: &[clidex::model::Tool]) {
    eprintln!("No tools found for: {query}");

    // Try a broader single-word search for suggestions
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.len() > 1 {
        for word in &words {
            if word.len() <= 2 {
                continue;
            }
            let partial = search::search(tools, word, 3);
            if !partial.is_empty() {
                let names: Vec<&str> = partial.iter().map(|r| r.tool.name.as_str()).collect();
                eprintln!("  Tip: try `clidex \"{word}\"` → {}", names.join(", "));
                return;
            }
        }
    }
    eprintln!("  Tip: try broader terms or `clidex --categories` to browse");
}

/// Suggest when category filter returns nothing.
fn suggest_category(category: &str, tools: &[clidex::model::Tool]) {
    eprintln!("No tools found in category: {category}");
    let cats = search::get_categories(tools);
    let cat_lower = category.to_lowercase();
    let similar: Vec<&str> = cats
        .iter()
        .filter(|(name, _)| {
            let nl = name.to_lowercase();
            nl.contains(&cat_lower) || cat_lower.split_whitespace().any(|w| nl.contains(w))
        })
        .take(5)
        .map(|(name, _)| name.as_str())
        .collect();
    if !similar.is_empty() {
        eprintln!("  Did you mean: {}", similar.join(", "));
    } else {
        eprintln!("  Tip: run `clidex --categories` to see all categories");
    }
}

fn do_search(
    search_index: &search::SearchIndex,
    query: &str,
    max_results: usize,
    format: Format,
    show_score: bool,
) {
    #[cfg(feature = "semantic")]
    let results = {
        let emb_path = clidex::config::embeddings_path();
        if emb_path.exists() {
            match clidex::semantic::load_embeddings(&emb_path) {
                Ok(embeddings) if embeddings.len() == search_index.tools().len() => {
                    match model2vec_rs::model::StaticModel::from_pretrained(
                        "minishlab/potion-base-2M",
                        None,
                        None,
                        None,
                    ) {
                        Ok(model) => {
                            let query_emb = model.encode(&[query.to_string()]);
                            if !query_emb.is_empty() {
                                search_index.hybrid_search(
                                    query,
                                    max_results,
                                    &embeddings,
                                    &query_emb[0],
                                )
                            } else {
                                search_index.search(query, max_results)
                            }
                        }
                        Err(_) => search_index.search(query, max_results),
                    }
                }
                _ => search_index.search(query, max_results),
            }
        } else {
            search_index.search(query, max_results)
        }
    };

    #[cfg(not(feature = "semantic"))]
    let results = search_index.search(query, max_results);

    if results.is_empty() {
        suggest_on_empty(query, search_index.tools());
        return; // exit 0 — empty result is not an error
    }
    output::print_search_results(&results, format, show_score);
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Update => {
                match index::update_index().await {
                    Ok(count) => eprintln!("Index updated: {count} tools"),
                    Err(e) => {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    }
                }
                return;
            }
            Commands::Stats => {
                let idx = load_or_exit_sync();
                let stats = index::index_stats(&idx);
                println!("Index version: {}", stats.version);
                println!("Generated:     {}", stats.generated);
                println!("Total tools:   {}", stats.total);
                println!("Categories:    {}", stats.categories);
                println!("With install:  {}", stats.with_install);
                println!("With stars:    {}", stats.with_stars);
                println!("With docs:     {}", stats.with_docs);
                println!("With llms.txt: {}", stats.with_llms_txt);
                return;
            }
            Commands::Info {
                name,
                pretty,
                json,
                yaml,
            } => {
                let idx = load_or_exit_sync();
                let fmt = get_format(pretty, json, yaml);
                match search::find_tool(&idx.tools, &name) {
                    Some(tool) => output::print_tool_detail(&tool, fmt),
                    None => {
                        eprintln!("Tool not found: {name}");
                        // Suggest similar
                        let results = search::search(&idx.tools, &name, 3);
                        if !results.is_empty() {
                            let names: Vec<&str> =
                                results.iter().map(|r| r.tool.name.as_str()).collect();
                            eprintln!("  Did you mean: {}", names.join(", "));
                        }
                        std::process::exit(1);
                    }
                }
                return;
            }
            Commands::Categories {
                filter,
                pretty,
                json,
                yaml,
            } => {
                let idx = load_or_exit_sync();
                let fmt = get_format(pretty, json, yaml);
                let mut cats = search::get_categories(&idx.tools);
                if let Some(ref f) = filter {
                    let fl = f.to_lowercase();
                    cats.retain(|(name, _)| {
                        let nl = name.to_lowercase();
                        nl.contains(&fl) || fl.split_whitespace().any(|w| nl.contains(w))
                    });
                }
                if cats.is_empty() {
                    if let Some(ref f) = filter {
                        eprintln!("No categories matching: {f}");
                        eprintln!("  Tip: run `clidex categories` to see all");
                    }
                    return;
                }
                output::print_categories(&cats, fmt);
                return;
            }
            Commands::Search {
                query,
                pretty,
                json,
                yaml,
                category,
                max_results,
                score,
            } => {
                let idx = match load_or_download().await {
                    Ok(i) => i,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    }
                };
                let fmt = get_format(pretty, json, yaml);
                let tools = if let Some(ref cat) = category {
                    let filtered = search::filter_by_category(&idx.tools, cat);
                    if filtered.is_empty() {
                        suggest_category(cat, &idx.tools);
                        return;
                    }
                    filtered
                } else {
                    idx.tools
                };
                let search_index = search::SearchIndex::new(tools);
                do_search(&search_index, &query, max_results, fmt, score);
                return;
            }
            Commands::Compare {
                names,
                pretty,
                json,
                yaml,
            } => {
                let idx = load_or_exit_sync();
                let fmt = get_format(pretty, json, yaml);
                let tools: Vec<_> = names
                    .iter()
                    .filter_map(|n| search::find_tool(&idx.tools, n))
                    .collect();
                if tools.is_empty() {
                    eprintln!("No matching tools found");
                    for name in &names {
                        let results = search::search(&idx.tools, name, 3);
                        if !results.is_empty() {
                            let suggestions: Vec<&str> =
                                results.iter().map(|r| r.tool.name.as_str()).collect();
                            eprintln!("  '{name}' → did you mean: {}", suggestions.join(", "));
                        }
                    }
                    std::process::exit(1);
                }
                let not_found: Vec<_> = names
                    .iter()
                    .filter(|n| search::find_tool(&idx.tools, n).is_none())
                    .collect();
                if !not_found.is_empty() {
                    eprintln!(
                        "Warning: not found: {}",
                        not_found
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                output::print_compare(&tools, fmt);
                return;
            }
            Commands::Trending {
                category,
                updated_since,
                max_results,
                pretty,
                json,
                yaml,
            } => {
                let idx = load_or_exit_sync();
                let fmt = get_format(pretty, json, yaml);
                let mut tools: Vec<_> = if let Some(ref cat) = category {
                    search::filter_by_category(&idx.tools, cat)
                } else {
                    idx.tools.clone()
                };
                tools.retain(|t| t.stars.is_some());
                if let Some(ref since_date) = updated_since {
                    tools.retain(|t| {
                        t.last_updated
                            .as_ref()
                            .map(|d| d.as_str() >= since_date.as_str())
                            .unwrap_or(false)
                    });
                }
                tools.sort_by(|a, b| b.stars.unwrap_or(0).cmp(&a.stars.unwrap_or(0)));
                tools.truncate(max_results);
                if tools.is_empty() {
                    eprintln!("No trending tools found");
                    if updated_since.is_some() {
                        eprintln!("  Tip: try a broader date range with --updated-since");
                    }
                    return; // exit 0
                }
                output::print_tools(&tools, fmt);
                return;
            }
        }
    }

    // Handle legacy --categories flag
    if cli.categories {
        let idx = load_or_exit_sync();
        let fmt = get_format(cli.pretty, cli.json, cli.yaml);
        let cats = search::get_categories(&idx.tools);
        output::print_categories(&cats, fmt);
        return;
    }

    // Handle legacy --category filter
    if let Some(ref category) = cli.category {
        let idx = load_or_exit_sync();
        let fmt = get_format(cli.pretty, cli.json, cli.yaml);
        let mut tools = search::filter_by_category(&idx.tools, category);
        tools.truncate(cli.max_results);
        if tools.is_empty() {
            suggest_category(category, &idx.tools);
            return; // exit 0
        }
        output::print_tools(&tools, fmt);
        return;
    }

    // Handle positional search query (shorthand for `clidex search`)
    if let Some(ref query) = cli.query {
        let idx = match load_or_download().await {
            Ok(i) => i,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        };
        let fmt = get_format(cli.pretty, cli.json, cli.yaml);
        let search_index = search::SearchIndex::new(idx.tools);
        do_search(&search_index, query, cli.max_results, fmt, cli.score);
        return;
    }

    // No query or command — show help
    use clap::CommandFactory;
    Cli::command().print_help().ok();
    println!();
}

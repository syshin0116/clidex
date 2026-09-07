use clap::{Parser, Subcommand};
use clidex::config;
use clidex::index;
use clidex::output::{self, Format};
use clidex::search;
use std::io::IsTerminal;

#[derive(Parser)]
#[command(name = "clidex", version, about = "CLI tool discovery for AI agents")]
struct Cli {
    /// Search query (e.g. "csv to json")
    query: Option<String>,
    /// Force pretty-printed output
    #[arg(long, global = true, conflicts_with_all = ["json", "yaml"])]
    pretty: bool,
    /// Force JSON output
    #[arg(long, global = true, conflicts_with = "yaml")]
    json: bool,
    /// Force YAML output
    #[arg(long, global = true)]
    yaml: bool,
    /// Filter by category
    #[arg(long, global = true)]
    category: Option<String>,
    /// List all categories
    #[arg(long, conflicts_with = "query")]
    categories: bool,
    /// Maximum number of results (default: 10, trending: 20)
    #[arg(short = 'n', long, global = true)]
    max_results: Option<usize>,
    /// Include relevance scores
    #[arg(long, global = true)]
    score: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for CLI tools
    Search { query: String },
    /// Search multiple queries while loading the index once
    Batch {
        #[arg(required = true)]
        queries: Vec<String>,
    },
    /// Show detailed info about a tool
    Info { name: String },
    /// Download/update the tool index
    Update,
    /// Show index statistics
    Stats,
    /// List categories
    Categories { filter: Option<String> },
    /// Compare tools side by side
    Compare {
        #[arg(required = true)]
        names: Vec<String>,
    },
    /// Show popular tools sorted by GitHub stars
    Trending {
        /// Only include tools updated on or after YYYY-MM-DD
        #[arg(long)]
        updated_since: Option<String>,
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
    } else if std::io::stdout().is_terminal() {
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
            if std::io::stdin().is_terminal() {
                // Auto-download in an interactive terminal.
                eprintln!("Index not found. Downloading from {}...", config::INDEX_URL);
                let count = index::update_index().await?;
                eprintln!("Index downloaded: {count} tools");
                index::load_index()
            } else {
                // Do not make silent network calls in CI or a pipe.
                Err(format!(
                    "Index not found at {}. Run `clidex update` first.",
                    config::index_path().display()
                ))
            }
        }
        Err(e) => Err(e),
    }
}

/// Suggest similar tools when search returns empty results.
fn suggest_on_empty(query: &str, search_index: &search::SearchIndex) {
    eprintln!("No tools found for: {query}");

    // Try a broader single-word search for suggestions
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.len() > 1 {
        for word in &words {
            if word.len() <= 2 {
                continue;
            }
            let partial = search_index.search(word, 3);
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
    #[cfg(feature = "semantic")] semantic: &Option<(
        model2vec_rs::model::StaticModel,
        Vec<Vec<f32>>,
    )>,
) -> Vec<search::SearchResult> {
    #[cfg(feature = "semantic")]
    if let Some((model, embeddings)) = semantic {
        if let Some(query_embedding) = model.encode(&[query.to_string()]).first() {
            return search_index.hybrid_search(query, max_results, embeddings, query_embedding);
        }
    }
    search_index.search(query, max_results)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let cli = Cli::parse();
    if cli.command.is_some() && (cli.query.is_some() || cli.categories) {
        return Err("Use either a query or a subcommand".into());
    }
    let format = get_format(cli.pretty, cli.json, cli.yaml);
    let max_results = cli.max_results.unwrap_or(config::DEFAULT_MAX_RESULTS);
    let command = cli
        .command
        .or_else(|| cli.query.map(|query| Commands::Search { query }));
    match command {
        Some(Commands::Update) => {
            let count = index::update_index().await?;
            eprintln!("Index updated: {count} tools");
        }
        Some(Commands::Search { .. } | Commands::Batch { .. }) => {
            let (queries, batch) = match command.unwrap() {
                Commands::Search { query } => (vec![query], false),
                Commands::Batch { queries } => (queries, true),
                _ => unreachable!(),
            };
            let mut idx = load_or_download().await?;
            #[cfg(feature = "semantic")]
            let semantic =
                clidex::semantic::load_tool_embeddings(&config::embeddings_path(), &idx.tools)
                    .ok()
                    .and_then(|embeddings| {
                        model2vec_rs::model::StaticModel::from_pretrained(
                            clidex::semantic::MODEL_ID,
                            None,
                            None,
                            None,
                        )
                        .ok()
                        .map(|model| (model, embeddings))
                    });
            let selected: Vec<_> = idx
                .tools
                .iter()
                .map(|tool| {
                    cli.category
                        .as_ref()
                        .is_none_or(|category| search::matches_category(tool, category))
                })
                .collect();
            if !selected.iter().any(|keep| *keep) {
                if let Some(ref category) = cli.category {
                    suggest_category(category, &idx.tools);
                }
            }
            #[cfg(feature = "semantic")]
            let semantic = semantic.map(|(model, embeddings)| {
                (
                    model,
                    embeddings
                        .into_iter()
                        .zip(&selected)
                        .filter_map(|(vector, keep)| keep.then_some(vector))
                        .collect(),
                )
            });
            idx.tools = idx
                .tools
                .into_iter()
                .zip(selected)
                .filter_map(|(tool, keep)| keep.then_some(tool))
                .collect();
            let engine = search::SearchIndex::new(idx.tools);
            let results: Vec<_> = queries
                .into_iter()
                .map(|query| {
                    let results = do_search(
                        &engine,
                        &query,
                        max_results,
                        #[cfg(feature = "semantic")]
                        &semantic,
                    );
                    if results.is_empty() {
                        suggest_on_empty(&query, &engine);
                    }
                    (query, results)
                })
                .collect();
            if batch {
                output::print_batch_results(&results, format, cli.score);
            } else {
                output::print_search_results(&results[0].1, format, cli.score);
            }
        }
        Some(Commands::Info { name }) => {
            let idx = index::load_index()?;
            let tool = search::find_tool(&idx.tools, &name).ok_or_else(|| {
                let results = search::search(&idx.tools, &name, 3);
                if !results.is_empty() {
                    eprintln!(
                        "  Did you mean: {}",
                        results
                            .iter()
                            .map(|r| r.tool.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                format!("Tool not found: {name}")
            })?;
            output::print_tool_detail(&tool, format);
        }
        Some(Commands::Compare { names }) => {
            let idx = index::load_index()?;
            let mut tools = Vec::new();
            let mut missing = Vec::new();
            for name in names {
                if let Some(tool) = search::find_tool(&idx.tools, &name) {
                    tools.push(tool);
                } else {
                    eprintln!("Warning: not found: {name}");
                    missing.push(name);
                }
            }
            if tools.is_empty() {
                let engine = search::SearchIndex::new(idx.tools);
                for name in missing {
                    let suggestions = engine.search(&name, 3);
                    if !suggestions.is_empty() {
                        eprintln!(
                            "  '{name}' -> did you mean: {}",
                            suggestions
                                .iter()
                                .map(|result| result.tool.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                }
                return Err("No matching tools found".into());
            }
            output::print_compare(&tools, format);
        }
        Some(Commands::Trending { updated_since }) => {
            let idx = index::load_index()?;
            let mut tools = cli
                .category
                .as_ref()
                .map(|cat| search::filter_by_category(&idx.tools, cat))
                .unwrap_or(idx.tools);
            tools.retain(|tool| {
                tool.stars.is_some()
                    && updated_since.as_ref().is_none_or(|since| {
                        tool.last_updated.as_ref().is_some_and(|date| date >= since)
                    })
            });
            tools.sort_by_key(|tool| std::cmp::Reverse(tool.stars.unwrap_or(0)));
            tools.truncate(cli.max_results.unwrap_or(20));
            if tools.is_empty() {
                eprintln!("No trending tools found");
            }
            output::print_tools(&tools, format);
        }
        Some(Commands::Stats) => {
            let stats = index::index_stats(&index::load_index()?);
            output::print_stats(&stats, format);
        }
        Some(Commands::Categories { .. }) | None
            if cli.categories || matches!(command, Some(Commands::Categories { .. })) =>
        {
            let filter = match command {
                Some(Commands::Categories { filter }) => filter,
                _ => None,
            };
            let mut categories = search::get_categories(&index::load_index()?.tools);
            if let Some(filter) = filter {
                let filter = filter.to_lowercase();
                categories.retain(|(name, _)| {
                    let name = name.to_lowercase();
                    name.contains(&filter)
                        || filter.split_whitespace().any(|word| name.contains(word))
                });
            }
            output::print_categories(&categories, format);
        }
        None if cli.category.is_some() => {
            let idx = index::load_index()?;
            let category = cli.category.as_ref().unwrap();
            let mut tools = search::filter_by_category(&idx.tools, category);
            tools.truncate(max_results);
            if tools.is_empty() {
                suggest_category(category, &idx.tools);
            }
            output::print_tools(&tools, format);
        }
        _ => {
            use clap::CommandFactory;
            Cli::command()
                .print_help()
                .map_err(|error| error.to_string())?;
            println!();
        }
    }
    Ok(())
}

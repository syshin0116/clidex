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
    #[arg(value_parser = clap::builder::NonEmptyStringValueParser::new())]
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
    #[arg(short = 'n', long, global = true, value_parser = parse_max_results)]
    max_results: Option<usize>,
    /// Include relevance scores
    #[arg(long, global = true)]
    score: bool,
    /// Only output these tool fields in JSON/YAML (comma-separated, e.g. name,install)
    #[arg(long, global = true, value_delimiter = ',', value_parser = parse_field)]
    fields: Option<Vec<String>>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for CLI tools
    Search {
        #[arg(value_parser = clap::builder::NonEmptyStringValueParser::new())]
        query: String,
    },
    /// Search multiple queries while loading the index once
    Batch {
        #[arg(required = true, value_parser = clap::builder::NonEmptyStringValueParser::new())]
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
    /// Serve the index to MCP clients over stdio
    Mcp,
    /// Show popular tools sorted by GitHub stars
    Trending {
        /// Only include tools updated on or after YYYY-MM-DD
        #[arg(long, value_parser = search::parse_date)]
        updated_since: Option<String>,
    },
}

fn parse_field(value: &str) -> Result<String, String> {
    if output::TOOL_FIELDS.contains(&value) {
        Ok(value.to_owned())
    } else {
        Err(format!(
            "expected one of: {}",
            output::TOOL_FIELDS.join(", ")
        ))
    }
}

/// Failure classes with distinct exit codes; clap reports usage errors with 2.
#[derive(Clone, Copy)]
enum ErrorKind {
    General,
    IndexUnavailable,
    NotFound,
    UpdateFailed,
}

impl ErrorKind {
    fn exit_code(self) -> i32 {
        match self {
            ErrorKind::General => 1,
            ErrorKind::IndexUnavailable => 3,
            ErrorKind::NotFound => 4,
            ErrorKind::UpdateFailed => 5,
        }
    }

    fn code(self) -> &'static str {
        match self {
            ErrorKind::General => "error",
            ErrorKind::IndexUnavailable => "index_unavailable",
            ErrorKind::NotFound => "not_found",
            ErrorKind::UpdateFailed => "update_failed",
        }
    }
}

struct CliError {
    kind: ErrorKind,
    message: String,
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        CliError {
            kind: ErrorKind::General,
            message,
        }
    }
}

fn error_as(kind: ErrorKind) -> impl FnOnce(String) -> CliError {
    move |message| CliError { kind, message }
}

/// Print the error on stderr in the same format as stdout and exit with its code.
fn report(error: CliError, format: Format) -> ! {
    #[derive(serde::Serialize)]
    struct ErrorBody<'a> {
        code: &'a str,
        message: &'a str,
        exit_code: i32,
    }
    #[derive(serde::Serialize)]
    struct ErrorOutput<'a> {
        error: ErrorBody<'a>,
    }
    let exit_code = error.kind.exit_code();
    let body = ErrorOutput {
        error: ErrorBody {
            code: error.kind.code(),
            message: &error.message,
            exit_code,
        },
    };
    match format {
        Format::Json => eprintln!(
            "{}",
            serde_json::to_string(&body).expect("JSON serialization")
        ),
        Format::Yaml => eprint!(
            "{}",
            serde_norway::to_string(&body).expect("YAML serialization")
        ),
        Format::Pretty => eprintln!("Error: {}", error.message),
    }
    std::process::exit(exit_code);
}

fn parse_max_results(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(0) | Err(_) => Err("expected a positive integer".into()),
        Ok(count) => Ok(count),
    }
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
async fn load_or_download() -> Result<clidex::model::Index, CliError> {
    load_or_download_inner()
        .await
        .map_err(error_as(ErrorKind::IndexUnavailable))
}

async fn load_or_download_inner() -> Result<clidex::model::Index, String> {
    let path = config::index_path()?;
    match index::load_index() {
        Ok(index) => {
            index::warn_if_stale(&index);
            Ok(index)
        }
        Err(_) if !path.exists() => {
            if std::io::stdin().is_terminal() {
                // Auto-download in an interactive terminal.
                eprintln!("Index not found. Downloading from {}...", config::INDEX_URL);
                index::update_index().await?;
                let index = index::load_index()?;
                eprintln!("Index downloaded: {} tools", index.tools.len());
                Ok(index)
            } else {
                // Do not make silent network calls in CI or a pipe.
                Err(format!(
                    "Index not found at {}. Run `clidex update` first.",
                    path.display()
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
    let cli = Cli::parse();
    let format = get_format(cli.pretty, cli.json, cli.yaml);
    if let Err(error) = run(cli, format).await {
        report(error, format);
    }
}

async fn run(cli: Cli, format: Format) -> Result<(), CliError> {
    if cli.command.is_some() && (cli.query.is_some() || cli.categories) {
        use clap::CommandFactory;
        Cli::command()
            .error(
                clap::error::ErrorKind::ArgumentConflict,
                "use either a query or a subcommand",
            )
            .exit();
    }
    let fields = cli.fields.as_deref();
    let max_results = cli.max_results.unwrap_or(config::DEFAULT_MAX_RESULTS);
    let command = cli
        .command
        .or_else(|| cli.query.map(|query| Commands::Search { query }));
    match command {
        Some(Commands::Update) => match index::update_index()
            .await
            .map_err(error_as(ErrorKind::UpdateFailed))?
        {
            index::UpdateOutcome::Updated(count) => eprintln!("Index updated: {count} tools"),
            index::UpdateOutcome::AlreadyCurrent(count) => {
                eprintln!("Index already up to date: {count} tools")
            }
        },
        Some(Commands::Search { .. } | Commands::Batch { .. }) => {
            let (queries, batch) = match command.unwrap() {
                Commands::Search { query } => (vec![query], false),
                Commands::Batch { queries } => (queries, true),
                _ => unreachable!(),
            };
            let mut idx = load_or_download().await?;
            #[cfg(feature = "semantic")]
            let semantic = config::embeddings_path()
                .ok()
                .and_then(|path| clidex::semantic::load_tool_embeddings(&path, &idx.tools).ok())
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
                output::print_batch_results(&results, format, cli.score, fields);
            } else {
                output::print_search_results(&results[0].1, format, cli.score, fields);
            }
        }
        Some(Commands::Info { name }) => {
            let idx = load_or_download().await?;
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
                CliError {
                    kind: ErrorKind::NotFound,
                    message: format!("Tool not found: {name}"),
                }
            })?;
            output::print_tool_detail(&tool, format, fields);
        }
        Some(Commands::Compare { names }) => {
            let idx = load_or_download().await?;
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
                return Err(CliError {
                    kind: ErrorKind::NotFound,
                    message: "No matching tools found".into(),
                });
            }
            output::print_compare(&tools, format, fields);
        }
        Some(Commands::Trending { updated_since }) => {
            let idx = load_or_download().await?;
            let tools = search::trending(
                &idx.tools,
                cli.category.as_deref(),
                updated_since.as_deref(),
                cli.max_results.unwrap_or(20),
            );
            if tools.is_empty() {
                eprintln!("No trending tools found");
            }
            output::print_tools(&tools, format, fields);
        }
        Some(Commands::Mcp) => {
            // An MCP client starts the server without a terminal, so download a
            // missing index here instead of requiring a separate `clidex update`.
            if !config::index_path()?.exists() {
                index::update_index()
                    .await
                    .map_err(error_as(ErrorKind::UpdateFailed))?;
            }
            let idx = load_or_download().await?;
            clidex::mcp::serve_stdio(idx.tools).await?;
        }
        Some(Commands::Stats) => {
            let stats = index::index_stats(&load_or_download().await?);
            output::print_stats(&stats, format);
        }
        Some(Commands::Categories { .. }) | None
            if cli.categories || matches!(command, Some(Commands::Categories { .. })) =>
        {
            let filter = match command {
                Some(Commands::Categories { filter }) => filter,
                _ => None,
            };
            let mut categories = search::get_categories(&load_or_download().await?.tools);
            if let Some(filter) = filter {
                search::filter_categories(&mut categories, &filter);
            }
            output::print_categories(&categories, format);
        }
        None if cli.category.is_some() => {
            let idx = load_or_download().await?;
            let category = cli.category.as_ref().unwrap();
            let mut tools = search::filter_by_category(&idx.tools, category);
            tools.truncate(max_results);
            if tools.is_empty() {
                suggest_category(category, &idx.tools);
            }
            output::print_tools(&tools, format, fields);
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

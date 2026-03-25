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

    /// Output as pretty-printed table (human-friendly). Default: YAML
    #[arg(long)]
    pretty: bool,

    /// Output as JSON. Default: YAML
    #[arg(long)]
    json: bool,

    /// Filter by category
    #[arg(long)]
    category: Option<String>,

    /// List all categories
    #[arg(long)]
    categories: bool,

    /// Maximum number of results
    #[arg(short = 'n', long, default_value_t = config::DEFAULT_MAX_RESULTS)]
    max_results: usize,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show detailed info about a tool
    Info {
        /// Tool name
        name: String,
        /// Output as pretty-printed table (human-friendly). Default: YAML
        #[arg(long)]
        pretty: bool,
        /// Output as JSON. Default: YAML
        #[arg(long)]
        json: bool,
    },
    /// Download/update the tool index
    Update,
    /// Show index statistics
    Stats,
    /// Compare tools side by side
    Compare {
        /// Tool names to compare
        names: Vec<String>,
        /// Output as pretty-printed table (human-friendly). Default: YAML
        #[arg(long)]
        pretty: bool,
        /// Output as JSON. Default: YAML
        #[arg(long)]
        json: bool,
    },
    /// Show trending/popular tools
    Trending {
        /// Category to filter by
        #[arg(long)]
        category: Option<String>,
        /// Maximum number of results
        #[arg(short = 'n', long, default_value_t = 20)]
        max_results: usize,
        /// Output as pretty-printed table (human-friendly). Default: YAML
        #[arg(long)]
        pretty: bool,
        /// Output as JSON. Default: YAML
        #[arg(long)]
        json: bool,
    },
}

fn get_format(pretty: bool, json: bool) -> Format {
    if pretty {
        Format::Pretty
    } else if json {
        Format::Json
    } else {
        Format::Yaml
    }
}

fn load_or_exit() -> clidex::model::Index {
    match index::load_index() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
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
                let idx = load_or_exit();
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
            Commands::Info { name, pretty, json } => {
                let idx = load_or_exit();
                match search::find_tool(&idx.tools, &name) {
                    Some(tool) => output::print_tool_detail(&tool, get_format(pretty, json)),
                    None => {
                        eprintln!("Tool not found: {name}");
                        std::process::exit(1);
                    }
                }
                return;
            }
            Commands::Compare {
                names,
                pretty,
                json,
            } => {
                let idx = load_or_exit();
                let tools: Vec<_> = names
                    .iter()
                    .filter_map(|n| search::find_tool(&idx.tools, n))
                    .collect();
                if tools.is_empty() {
                    eprintln!("No matching tools found");
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
                output::print_compare(&tools, get_format(pretty, json));
                return;
            }
            Commands::Trending {
                category,
                max_results,
                pretty,
                json,
            } => {
                let idx = load_or_exit();
                let mut tools: Vec<_> = if let Some(ref cat) = category {
                    search::filter_by_category(&idx.tools, cat)
                } else {
                    let mut all = idx.tools.clone();
                    all.sort_by(|a, b| b.stars.unwrap_or(0).cmp(&a.stars.unwrap_or(0)));
                    all
                };
                // Only show tools with stars for trending
                tools.retain(|t| t.stars.is_some());
                tools.truncate(max_results);
                if tools.is_empty() {
                    eprintln!("No trending tools found (need stars data — run build_index with GITHUB_TOKEN)");
                    std::process::exit(1);
                }
                output::print_tools(&tools, get_format(pretty, json));
                return;
            }
        }
    }

    // Handle categories listing
    if cli.categories {
        let idx = load_or_exit();
        let cats = search::get_categories(&idx.tools);
        output::print_categories(&cats, get_format(cli.pretty, cli.json));
        return;
    }

    // Handle category filter
    if let Some(ref category) = cli.category {
        let idx = load_or_exit();
        let mut tools = search::filter_by_category(&idx.tools, category);
        tools.truncate(cli.max_results);
        if tools.is_empty() {
            eprintln!("No tools found in category: {category}");
            std::process::exit(1);
        }
        output::print_tools(&tools, get_format(cli.pretty, cli.json));
        return;
    }

    // Handle search query
    if let Some(ref query) = cli.query {
        let idx = load_or_exit();

        #[cfg(feature = "semantic")]
        let results = {
            let emb_path = clidex::config::embeddings_path();
            if emb_path.exists() {
                match clidex::semantic::load_embeddings(&emb_path) {
                    Ok(embeddings) if embeddings.len() == idx.tools.len() => {
                        // Load model for query embedding
                        match model2vec_rs::model::StaticModel::from_pretrained(
                            "minishlab/potion-base-2M",
                            None,
                            None,
                            None,
                        ) {
                            Ok(model) => {
                                let query_emb = model.encode(&[query.to_string()]);
                                if !query_emb.is_empty() {
                                    search::hybrid_search(
                                        &idx.tools,
                                        query,
                                        cli.max_results,
                                        &embeddings,
                                        &query_emb[0],
                                    )
                                } else {
                                    search::search(&idx.tools, query, cli.max_results)
                                }
                            }
                            Err(_) => search::search(&idx.tools, query, cli.max_results),
                        }
                    }
                    _ => search::search(&idx.tools, query, cli.max_results),
                }
            } else {
                search::search(&idx.tools, query, cli.max_results)
            }
        };

        #[cfg(not(feature = "semantic"))]
        let results = search::search(&idx.tools, query, cli.max_results);

        if results.is_empty() {
            eprintln!("No tools found for: {query}");
            std::process::exit(1);
        }
        let tools: Vec<_> = results.into_iter().map(|r| r.tool).collect();
        output::print_tools(&tools, get_format(cli.pretty, cli.json));
        return;
    }

    // No query or command — show help
    use clap::CommandFactory;
    Cli::command().print_help().ok();
    println!();
}

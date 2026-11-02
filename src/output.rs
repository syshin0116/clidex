use crate::model::Tool;
use crate::search::SearchResult;
use colored::Colorize;

fn term_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(120)
}

/// Truncate a string to `max_width` characters (not bytes), appending "..." if truncated.
fn truncate_str(s: &str, max_width: usize) -> String {
    if s.chars().count() > max_width {
        let truncated: String = s.chars().take(max_width - 1).collect();
        format!("{truncated}…")
    } else {
        s.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Pretty,
    Yaml,
    Json,
}

/// Stable search result schema: score wraps tool and is never injected into Tool.
#[derive(serde::Serialize)]
struct SearchResultOutput<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f64>,
    #[serde(flatten)]
    tool: &'a Tool,
}

/// Version of the JSON/YAML output contract, reported by `stats`.
pub const SCHEMA_VERSION: u32 = 1;

/// Tool fields accepted by `--fields`, in index order.
pub const TOOL_FIELDS: &[&str] = &[
    "name",
    "binary",
    "desc",
    "category",
    "tags",
    "install",
    "stars",
    "brew_installs_365d",
    "links",
    "last_updated",
    "github_fetched_at",
    "examples",
    "repology_fetched_at",
];

/// A JSON object that serializes its keys in insertion order.
struct OrderedObject(Vec<(String, serde_json::Value)>);

impl serde::Serialize for OrderedObject {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

/// Keep `score` and the requested fields of one serialized tool, in the requested order.
fn project_tool(tool: serde_json::Value, fields: &[String]) -> OrderedObject {
    let serde_json::Value::Object(mut object) = tool else {
        return OrderedObject(Vec::new());
    };
    let mut kept = Vec::new();
    for key in std::iter::once("score").chain(fields.iter().map(String::as_str)) {
        if let Some(value) = object.remove(key) {
            kept.push((key.to_owned(), value));
        }
    }
    OrderedObject(kept)
}

fn write_structured<T: serde::Serialize + ?Sized>(value: &T, format: Format) {
    match format {
        Format::Json => println!(
            "{}",
            serde_json::to_string_pretty(value).expect("JSON serialization")
        ),
        _ => print!(
            "{}",
            serde_norway::to_string(value).expect("YAML serialization")
        ),
    }
}

/// Print one tool or a list of tools as JSON/YAML, keeping only `fields` when given.
fn emit_tools<T: serde::Serialize>(items: &[T], format: Format, fields: Option<&[String]>) {
    match fields {
        None => write_structured(items, format),
        Some(fields) => {
            let projected: Vec<OrderedObject> = items
                .iter()
                .map(|item| {
                    project_tool(
                        serde_json::to_value(item).expect("JSON serialization"),
                        fields,
                    )
                })
                .collect();
            write_structured(&projected, format);
        }
    }
}

/// Print search results. With --score, output uses `{score, ...tool}` wrapper.
/// Without --score, output is plain `[Tool]` (same schema as info/compare/trending).
pub fn print_search_results(
    results: &[SearchResult],
    format: Format,
    show_score: bool,
    fields: Option<&[String]>,
) {
    match format {
        Format::Yaml | Format::Json => {
            let items: Vec<SearchResultOutput> = results
                .iter()
                .map(|r| SearchResultOutput {
                    score: show_score.then_some(r.score),
                    tool: &r.tool,
                })
                .collect();
            emit_tools(&items, format, fields);
        }
        Format::Pretty => {
            let width = term_width();
            for (i, result) in results.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                print_search_result_pretty(result, show_score, width);
            }
        }
    }
}

pub fn print_batch_results(
    results: &[(String, Vec<SearchResult>)],
    format: Format,
    show_score: bool,
    fields: Option<&[String]>,
) {
    #[derive(serde::Serialize)]
    struct QueryResults<'a, T> {
        query: &'a str,
        results: Vec<T>,
    }
    if format == Format::Pretty {
        for (query, results) in results {
            println!("{}", query.bold());
            print_search_results(results, format, show_score, None);
        }
        return;
    }
    fn scored(results: &[SearchResult], show_score: bool) -> Vec<SearchResultOutput<'_>> {
        results
            .iter()
            .map(|result| SearchResultOutput {
                score: show_score.then_some(result.score),
                tool: &result.tool,
            })
            .collect()
    }
    match fields {
        None => {
            let items: Vec<_> = results
                .iter()
                .map(|(query, results)| QueryResults {
                    query,
                    results: scored(results, show_score),
                })
                .collect();
            write_structured(&items, format);
        }
        Some(fields) => {
            let items: Vec<_> = results
                .iter()
                .map(|(query, results)| QueryResults {
                    query,
                    results: scored(results, show_score)
                        .iter()
                        .map(|item| {
                            project_tool(
                                serde_json::to_value(item).expect("JSON serialization"),
                                fields,
                            )
                        })
                        .collect(),
                })
                .collect();
            write_structured(&items, format);
        }
    }
}

pub fn print_stats(stats: &crate::index::IndexStats, format: Format) {
    match format {
        Format::Yaml | Format::Json => write_structured(stats, format),
        Format::Pretty => {
            println!("Index version: {}", stats.version);
            println!("Generated:     {}", stats.generated);
            println!("Total tools:   {}", stats.total);
            println!("Categories:    {}", stats.categories);
            println!("With install:  {}", stats.with_install);
            println!("With stars:    {}", stats.with_stars);
            println!("With docs:     {}", stats.with_docs);
            println!("With llms.txt: {}", stats.with_llms_txt);
        }
    }
}

fn print_search_result_pretty(result: &SearchResult, show_score: bool, width: usize) {
    let tool = &result.tool;

    let stars_str = match tool.stars {
        Some(s) if s >= 1000 => format!("★ {:.1}k", s as f64 / 1000.0),
        Some(s) => format!("★ {s}"),
        None => String::new(),
    };

    let install_str = tool
        .install
        .iter()
        .next()
        .map(|(_, cmd)| cmd.as_str())
        .unwrap_or("");

    let score_str = if show_score {
        format!(" [{:.4}]", result.score)
    } else {
        String::new()
    };

    println!(
        "  {:16} {:>8}  {}{}",
        tool.name.bold(),
        stars_str.yellow(),
        tool.category.cyan(),
        score_str.dimmed(),
    );

    // Truncate the description to the terminal width.
    let desc_max = width.saturating_sub(4); // 2 indent + margin
    let desc = truncate_str(&tool.desc, desc_max);
    println!("  {}", desc.dimmed());

    // Install command (first available)
    if !install_str.is_empty() {
        println!("  {}", format!("$ {install_str}").green());
    }
}

pub fn print_tools(tools: &[Tool], format: Format, fields: Option<&[String]>) {
    match format {
        Format::Yaml | Format::Json => emit_tools(tools, format, fields),
        Format::Pretty => {
            let width = term_width();
            for (i, tool) in tools.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                print_tool_pretty(tool, width);
            }
        }
    }
}

pub fn print_tool_detail(tool: &Tool, format: Format, fields: Option<&[String]>) {
    match format {
        Format::Yaml | Format::Json => match fields {
            None => write_structured(tool, format),
            Some(fields) => write_structured(
                &project_tool(
                    serde_json::to_value(tool).expect("JSON serialization"),
                    fields,
                ),
                format,
            ),
        },
        Format::Pretty => {
            print_tool_detail_pretty(tool);
        }
    }
}

pub fn print_categories(cats: &[(String, usize)], format: Format) {
    match format {
        Format::Yaml | Format::Json => {
            let map: Vec<serde_json::Value> = cats
                .iter()
                .map(|(name, count)| serde_json::json!({ "name": name, "count": count }))
                .collect();
            write_structured(&map, format);
        }
        Format::Pretty => {
            for (name, count) in cats {
                println!(
                    "  {} {}",
                    format!("{name:30}").bold(),
                    format!("({count})").dimmed()
                );
            }
        }
    }
}

pub fn print_compare(tools: &[Tool], format: Format, fields: Option<&[String]>) {
    match format {
        Format::Yaml | Format::Json => print_tools(tools, format, fields),
        Format::Pretty => print_compare_pretty(tools),
    }
}

fn print_compare_pretty(tools: &[Tool]) {
    if tools.is_empty() {
        return;
    }

    let width = term_width();
    let label_width = 14;
    let available = width.saturating_sub(label_width);
    // Adaptive column width: split available space among tools
    let col_width = if !tools.is_empty() {
        (available / tools.len()).clamp(20, 40)
    } else {
        30
    };

    // Warn if too many tools for terminal width
    let needed = label_width + tools.len() * (col_width + 2);
    if needed > width {
        eprintln!(
            "  Note: {} tools may not fit in {}-column terminal. Try fewer tools or widen terminal.",
            tools.len(),
            width,
        );
    }

    // Header
    print!("{:label_width$}", "");
    for tool in tools {
        print!("  {:col_width$}", tool.name.bold().to_string());
    }
    println!();

    // Separator
    print!("{:label_width$}", "");
    for _ in tools {
        print!("  {:─<col_width$}", "");
    }
    println!();

    // Description
    print!("{:label_width$}", "Description".dimmed().to_string());
    for tool in tools {
        let desc = truncate_str(&tool.desc, col_width);
        print!("  {:col_width$}", desc);
    }
    println!();

    // Category
    print!("{:label_width$}", "Category".dimmed().to_string());
    for tool in tools {
        let cat = tool.category.split(" > ").last().unwrap_or(&tool.category);
        let cat = truncate_str(cat, col_width);
        print!("  {:col_width$}", cat.cyan().to_string());
    }
    println!();

    // Stars
    print!("{:label_width$}", "Stars".dimmed().to_string());
    for tool in tools {
        let s = match tool.stars {
            Some(s) if s >= 1000 => format!("★ {:.1}k", s as f64 / 1000.0),
            Some(s) => format!("★ {s}"),
            None => "-".to_string(),
        };
        print!("  {:col_width$}", s.yellow().to_string());
    }
    println!();

    // Install
    print!("{:label_width$}", "Install".dimmed().to_string());
    for tool in tools {
        let install = if let Some(cmd) = tool.install.get("brew") {
            cmd.clone()
        } else if let Some((_, cmd)) = tool.install.iter().next() {
            cmd.clone()
        } else {
            "-".to_string()
        };
        let install = truncate_str(&install, col_width);
        print!("  {:col_width$}", install.green().to_string());
    }
    println!();

    // Last updated
    print!("{:label_width$}", "Updated".dimmed().to_string());
    for tool in tools {
        let updated = tool
            .last_updated
            .as_deref()
            .map(|s| s.split('T').next().unwrap_or(s))
            .unwrap_or("-");
        print!("  {:col_width$}", updated);
    }
    println!();

    // Links
    print!("{:label_width$}", "Repo".dimmed().to_string());
    for tool in tools {
        let repo = tool.links.repo.as_deref().unwrap_or("-");
        let repo = truncate_str(repo, col_width);
        print!("  {:col_width$}", repo.blue().to_string());
    }
    println!();
}

fn print_tool_pretty(tool: &Tool, width: usize) {
    let stars_str = match tool.stars {
        Some(s) if s >= 1000 => format!("★ {:.1}k", s as f64 / 1000.0),
        Some(s) => format!("★ {s}"),
        None => String::new(),
    };

    let install_str = tool
        .install
        .iter()
        .next()
        .map(|(_, cmd)| cmd.as_str())
        .unwrap_or("");

    println!(
        "  {:16} {:>8}  {}",
        tool.name.bold(),
        stars_str.yellow(),
        tool.category.cyan(),
    );

    let desc_max = width.saturating_sub(4);
    let desc = truncate_str(&tool.desc, desc_max);
    println!("  {}", desc.dimmed());

    if !install_str.is_empty() {
        println!("  {}", format!("$ {install_str}").green());
    } else if let Some(ref repo) = tool.links.repo {
        println!("  {}", format!("repo: {repo}").blue());
    }
}

fn print_tool_detail_pretty(tool: &Tool) {
    println!("  {} {}", "Name:".bold(), tool.name);
    if let Some(ref bin) = tool.binary {
        println!("  {} {}", "Binary:".bold(), bin);
    }
    println!("  {} {}", "Description:".bold(), tool.desc);
    println!("  {} {}", "Category:".bold(), tool.category.cyan());

    if !tool.tags.is_empty() {
        println!("  {} {}", "Tags:".bold(), tool.tags.join(", "));
    }

    if let Some(s) = tool.stars {
        println!("  {} {}", "Stars:".bold(), format!("{s}").yellow());
    }
    if let Some(b) = tool.brew_installs_365d {
        println!("  {} {}", "Brew installs (365d):".bold(), b);
    }

    if !tool.install.is_empty() {
        println!("  {}", "Install:".bold());
        for (registry, cmd) in &tool.install {
            println!("    {}: {}", registry.green(), cmd);
        }
    }

    println!("  {}", "Links:".bold());
    if let Some(ref repo) = tool.links.repo {
        println!("    {}: {}", "repo".dimmed(), repo.blue());
    }
    if let Some(ref docs) = tool.links.docs {
        println!("    {}: {}", "docs".dimmed(), docs.blue());
    }
    if let Some(ref llms) = tool.links.llms_txt {
        println!("    {}: {}", "llms.txt".dimmed(), llms.blue());
    }
    if let Some(ref hp) = tool.links.homepage {
        println!("    {}: {}", "homepage".dimmed(), hp.blue());
    }
}

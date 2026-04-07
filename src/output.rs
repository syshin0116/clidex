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

/// Stable search result schema — score wraps tool, never injected into Tool.
#[derive(serde::Serialize)]
struct SearchResultOutput<'a> {
    score: f64,
    #[serde(flatten)]
    tool: &'a Tool,
}

/// Print search results. With --score, output uses `{score, ...tool}` wrapper.
/// Without --score, output is plain `[Tool]` (same schema as info/compare/trending).
pub fn print_search_results(results: &[SearchResult], format: Format, show_score: bool) {
    match format {
        Format::Yaml => {
            if show_score {
                let items: Vec<SearchResultOutput> = results
                    .iter()
                    .map(|r| SearchResultOutput {
                        score: (r.score * 10.0).round() / 10.0,
                        tool: &r.tool,
                    })
                    .collect();
                let yaml = serde_yaml::to_string(&items).expect("Failed to serialize to YAML");
                print!("{yaml}");
            } else {
                let tools: Vec<&Tool> = results.iter().map(|r| &r.tool).collect();
                let yaml = serde_yaml::to_string(&tools).expect("Failed to serialize to YAML");
                print!("{yaml}");
            }
        }
        Format::Json => {
            if show_score {
                let items: Vec<SearchResultOutput> = results
                    .iter()
                    .map(|r| SearchResultOutput {
                        score: (r.score * 10.0).round() / 10.0,
                        tool: &r.tool,
                    })
                    .collect();
                let json =
                    serde_json::to_string_pretty(&items).expect("Failed to serialize to JSON");
                println!("{json}");
            } else {
                let tools: Vec<&Tool> = results.iter().map(|r| &r.tool).collect();
                let json =
                    serde_json::to_string_pretty(&tools).expect("Failed to serialize to JSON");
                println!("{json}");
            }
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
        format!(" [{:.1}]", result.score)
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

    // Description — truncate to terminal width
    let desc_max = width.saturating_sub(4); // 2 indent + margin
    let desc = truncate_str(&tool.desc, desc_max);
    println!("  {}", desc.dimmed());

    // Install command (first available)
    if !install_str.is_empty() {
        println!("  {}", format!("$ {install_str}").green());
    }
}

pub fn print_tools(tools: &[Tool], format: Format) {
    match format {
        Format::Yaml => {
            let yaml = serde_yaml::to_string(tools).expect("Failed to serialize to YAML");
            print!("{yaml}");
        }
        Format::Json => {
            let json = serde_json::to_string_pretty(tools).expect("Failed to serialize to JSON");
            println!("{json}");
        }
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

pub fn print_tool_detail(tool: &Tool, format: Format) {
    match format {
        Format::Yaml => {
            let yaml = serde_yaml::to_string(tool).expect("Failed to serialize to YAML");
            print!("{yaml}");
        }
        Format::Json => {
            let json = serde_json::to_string_pretty(tool).expect("Failed to serialize to JSON");
            println!("{json}");
        }
        Format::Pretty => {
            print_tool_detail_pretty(tool);
        }
    }
}

pub fn print_categories(cats: &[(String, usize)], format: Format) {
    match format {
        Format::Yaml => {
            let map: Vec<serde_json::Value> = cats
                .iter()
                .map(|(name, count)| serde_json::json!({ "name": name, "count": count }))
                .collect();
            let yaml = serde_yaml::to_string(&map).expect("Failed to serialize to YAML");
            print!("{yaml}");
        }
        Format::Json => {
            let map: Vec<serde_json::Value> = cats
                .iter()
                .map(|(name, count)| serde_json::json!({ "name": name, "count": count }))
                .collect();
            let json = serde_json::to_string_pretty(&map).expect("Failed to serialize to JSON");
            println!("{json}");
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

pub fn print_compare(tools: &[Tool], format: Format) {
    match format {
        Format::Yaml | Format::Json => print_tools(tools, format),
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
            None => "—".to_string(),
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
            "—".to_string()
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
            .unwrap_or("—");
        print!("  {:col_width$}", updated);
    }
    println!();

    // Links
    print!("{:label_width$}", "Repo".dimmed().to_string());
    for tool in tools {
        let repo = tool.links.repo.as_deref().unwrap_or("—");
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

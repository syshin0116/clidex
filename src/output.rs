use crate::model::Tool;
use colored::Colorize;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Pretty,
    Yaml,
    Json,
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
            for (i, tool) in tools.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                print_tool_pretty(tool);
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

    let col_width = 30;
    let label_width = 14;

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
        let desc = if tool.desc.len() > col_width {
            format!("{}…", &tool.desc[..col_width - 1])
        } else {
            tool.desc.clone()
        };
        print!("  {:col_width$}", desc);
    }
    println!();

    // Category
    print!("{:label_width$}", "Category".dimmed().to_string());
    for tool in tools {
        let cat = tool.category.split(" > ").last().unwrap_or(&tool.category);
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
        let install = if install.len() > col_width {
            format!("{}…", &install[..col_width - 1])
        } else {
            install
        };
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
        let repo = if repo.len() > col_width {
            format!("{}…", &repo[..col_width - 1])
        } else {
            repo.to_string()
        };
        print!("  {:col_width$}", repo.blue().to_string());
    }
    println!();
}

fn print_tool_pretty(tool: &Tool) {
    let stars_str = match tool.stars {
        Some(s) if s >= 1000 => format!("★ {:.1}k", s as f64 / 1000.0),
        Some(s) => format!("★ {s}"),
        None => String::new(),
    };

    let installers: Vec<&str> = tool.install.keys().map(|s| s.as_str()).collect();
    let install_str = installers.join("/");

    println!(
        "  {:16} {:>8}  {:12}  {}",
        tool.name.bold(),
        stars_str.yellow(),
        install_str.dimmed(),
        tool.category.cyan()
    );
    println!("  {}", tool.desc.dimmed());

    if let Some(ref docs) = tool.links.docs {
        println!("  {}", format!("docs: {docs}").blue());
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
    if let Some(b) = tool.brew_installs_30d {
        println!("  {} {}", "Brew installs (30d):".bold(), b);
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

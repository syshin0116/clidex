//! Model Context Protocol server over stdio (`clidex mcp`).
//!
//! The index is loaded and the search engine built once at startup; each tool
//! call then reuses them. Results are returned as `structuredContent` with an
//! `outputSchema`, so clients can validate them without parsing text.

use crate::model::Tool;
use crate::search::{self, SearchIndex};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{tool, tool_handler, tool_router, Json, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const DEFAULT_MAX_RESULTS: usize = 5;
/// Caps every list result so one call cannot flood the client's context.
const MAX_RESULTS_LIMIT: usize = 50;

#[derive(Deserialize, JsonSchema)]
pub struct SearchParams {
    /// Short description of the task, e.g. "convert csv to json".
    pub query: String,
    /// Number of results (default 5, at most 50).
    pub max_results: Option<usize>,
    /// Only search this category, e.g. "Data" or "Development > Git".
    pub category: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct InfoParams {
    /// Tool name or binary name, e.g. "ripgrep" or "rg".
    pub name: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CompareParams {
    /// Two or more tool or binary names.
    pub names: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct TrendingParams {
    /// Only include this category.
    pub category: Option<String>,
    /// Only include tools updated on or after this date (YYYY-MM-DD).
    pub updated_since: Option<String>,
    /// Number of results (default 5, at most 50).
    pub max_results: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
pub struct CategoriesParams {
    /// Only categories whose name contains this text or any of its words.
    pub filter: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ScoredTool {
    /// Relevance score; higher is better. Only comparable within one search.
    pub score: f64,
    #[serde(flatten)]
    pub tool: Tool,
}

#[derive(Serialize, JsonSchema)]
pub struct SearchOutput {
    pub results: Vec<ScoredTool>,
}

#[derive(Serialize, JsonSchema)]
pub struct ToolsOutput {
    pub tools: Vec<Tool>,
    /// Requested names that matched no tool.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_found: Vec<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct Category {
    pub name: String,
    pub count: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct CategoriesOutput {
    pub categories: Vec<Category>,
}

fn limit(max_results: Option<usize>) -> usize {
    max_results
        .unwrap_or(DEFAULT_MAX_RESULTS)
        .clamp(1, MAX_RESULTS_LIMIT)
}

#[derive(Clone)]
pub struct ClidexServer {
    engine: Arc<SearchIndex>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl ClidexServer {
    pub fn new(tools: Vec<Tool>) -> Self {
        Self {
            engine: Arc::new(SearchIndex::new(tools)),
            tool_router: Self::tool_router(),
        }
    }

    /// Find command-line tools for a task. Returns ranked tools with install
    /// commands per package manager and repository/documentation links.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn search_tools(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<Json<SearchOutput>, String> {
        let query = params.query.trim();
        if query.is_empty() {
            return Err("query must not be empty".into());
        }
        let max_results = limit(params.max_results);
        let results = match params.category.as_deref() {
            None => self.engine.search(query, max_results),
            Some(category) => {
                let tools = self
                    .engine
                    .tools()
                    .iter()
                    .filter(|tool| search::matches_category(tool, category))
                    .cloned()
                    .collect();
                SearchIndex::new(tools).search(query, max_results)
            }
        };
        Ok(Json(SearchOutput {
            results: results
                .into_iter()
                .map(|result| ScoredTool {
                    score: result.score,
                    tool: result.tool,
                })
                .collect(),
        }))
    }

    /// Show the full index entry for one tool, looked up by name or binary.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn tool_info(
        &self,
        Parameters(params): Parameters<InfoParams>,
    ) -> Result<Json<Tool>, String> {
        search::find_tool(self.engine.tools(), &params.name)
            .map(Json)
            .ok_or_else(|| {
                let suggestions: Vec<_> = self
                    .engine
                    .search(&params.name, 3)
                    .into_iter()
                    .map(|result| result.tool.name)
                    .collect();
                if suggestions.is_empty() {
                    format!("Tool not found: {}", params.name)
                } else {
                    format!(
                        "Tool not found: {}. Did you mean: {}",
                        params.name,
                        suggestions.join(", ")
                    )
                }
            })
    }

    /// Compare several tools side by side by name or binary.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn compare_tools(
        &self,
        Parameters(params): Parameters<CompareParams>,
    ) -> Result<Json<ToolsOutput>, String> {
        let mut output = ToolsOutput {
            tools: Vec::new(),
            not_found: Vec::new(),
        };
        for name in params.names.into_iter().take(MAX_RESULTS_LIMIT) {
            match search::find_tool(self.engine.tools(), &name) {
                Some(tool) => output.tools.push(tool),
                None => output.not_found.push(name),
            }
        }
        if output.tools.is_empty() {
            return Err("No matching tools found".into());
        }
        Ok(Json(output))
    }

    /// List the most starred tools, optionally by category or recent activity.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn trending_tools(
        &self,
        Parameters(params): Parameters<TrendingParams>,
    ) -> Result<Json<ToolsOutput>, String> {
        let updated_since = params
            .updated_since
            .as_deref()
            .map(search::parse_date)
            .transpose()?;
        Ok(Json(ToolsOutput {
            tools: search::trending(
                self.engine.tools(),
                params.category.as_deref(),
                updated_since.as_deref(),
                limit(params.max_results),
            ),
            not_found: Vec::new(),
        }))
    }

    /// List tool categories with their tool counts, largest first.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn list_categories(
        &self,
        Parameters(params): Parameters<CategoriesParams>,
    ) -> Result<Json<CategoriesOutput>, String> {
        let mut categories = search::get_categories(self.engine.tools());
        if let Some(filter) = params.filter.as_deref() {
            search::filter_categories(&mut categories, filter);
        }
        Ok(Json(CategoriesOutput {
            categories: categories
                .into_iter()
                .map(|(name, count)| Category { name, count })
                .collect(),
        }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ClidexServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("clidex", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Use search_tools when a task would benefit from a specialized CLI. \
                 Show the user the exact install command and ask before installing.",
            )
    }
}

/// Serve MCP over stdin/stdout until the client disconnects.
pub async fn serve_stdio(tools: Vec<Tool>) -> Result<(), String> {
    let service = ClidexServer::new(tools)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|error| format!("MCP initialization failed: {error}"))?;
    service
        .waiting()
        .await
        .map_err(|error| format!("MCP server stopped: {error}"))?;
    Ok(())
}

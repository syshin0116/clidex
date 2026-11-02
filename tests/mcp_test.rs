use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn mcp_server_lists_tools_and_returns_structured_results() {
    let path = std::env::temp_dir().join(format!("clidex-mcp-{}.yaml", std::process::id()));
    std::fs::write(
        &path,
        r#"
version: 1
generated: '2026-09-07T00:00:00Z'
tools:
  - name: jq
    desc: JSON processor
    category: Data
    stars: 100
    install:
      brew: brew install jq
  - name: ripgrep
    binary: rg
    desc: Search files
    category: Files
    stars: 200
"#,
    )
    .unwrap();
    let mut server = Command::new(env!("CARGO_BIN_EXE_clidex"))
        .arg("mcp")
        .env("CLIDEX_INDEX_PATH", &path)
        .env("CLIDEX_NO_STALE_WARNING", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = server.stdin.take().unwrap();
    let mut stdout = BufReader::new(server.stdout.take().unwrap());
    let mut request = |message: Value| -> Value {
        writeln!(stdin, "{message}").unwrap();
        if message.get("id").is_none() {
            return Value::Null;
        }
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };

    let init = request(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": {"name": "test", "version": "0"}}}),
    );
    assert_eq!(init["result"]["serverInfo"]["name"], "clidex");
    request(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let list = request(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    let tools = list["result"]["tools"].as_array().unwrap();
    let mut names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "compare_tools",
            "list_categories",
            "search_tools",
            "tool_info",
            "trending_tools"
        ]
    );
    assert!(tools
        .iter()
        .all(|tool| tool["outputSchema"]["type"] == "object"));

    let mut call = |id: u64, name: &str, arguments: Value| -> Value {
        request(json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": name, "arguments": arguments}}))["result"]
            .clone()
    };
    let search = call(3, "search_tools", json!({"query": "json processor"}));
    assert_eq!(search["structuredContent"]["results"][0]["name"], "jq");
    assert!(
        search["structuredContent"]["results"][0]["score"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    let info = call(4, "tool_info", json!({"name": "rg"}));
    assert_eq!(info["structuredContent"]["name"], "ripgrep");
    let missing = call(5, "tool_info", json!({"name": "qzxjkvblorp"}));
    assert_eq!(missing["isError"], true);
    let compare = call(6, "compare_tools", json!({"names": ["jq", "nope"]}));
    assert_eq!(compare["structuredContent"]["not_found"], json!(["nope"]));
    let trending = call(7, "trending_tools", json!({"max_results": 1}));
    assert_eq!(trending["structuredContent"]["tools"][0]["name"], "ripgrep");
    assert_eq!(
        call(8, "trending_tools", json!({"updated_since": "2026-8"}))["isError"],
        true
    );
    let categories = call(9, "list_categories", json!({"filter": "data"}));
    assert_eq!(
        categories["structuredContent"]["categories"],
        json!([{"name": "Data", "count": 1}])
    );

    drop(stdin);
    assert!(server.wait().unwrap().success());
    std::fs::remove_file(path).unwrap();
}

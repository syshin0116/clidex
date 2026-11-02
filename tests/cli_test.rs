use std::process::Command;

#[test]
fn cli_preserves_filters_formats_and_empty_results() {
    let path = std::env::temp_dir().join(format!("clidex-cli-{}.yaml", std::process::id()));
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
  - name: csvkit
    desc: CSV processor
    category: Data
    stars: 200
  - name: rg
    desc: Search files
    category: Files
"#,
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_clidex"))
            .env("CLIDEX_INDEX_PATH", &path)
            .args(args)
            .output()
            .unwrap()
    };
    for args in [
        vec!["qzxjkvblorp", "--json"],
        vec!["qzxjkvblorp", "--category", "data", "--json"],
        vec!["search", "qzxjkvblorp", "--category", "data", "--json"],
        vec!["categories", "qzxjkvblorp", "--json"],
        vec!["trending", "--category", "qzxjkvblorp", "--json"],
    ] {
        let output = run(&args);
        assert!(output.status.success(), "{args:?}: {:?}", output.stderr);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            serde_json::json!([]),
            "{args:?}"
        );
    }
    let before = run(&["--json", "info", "jq"]);
    let after = run(&["info", "jq", "--json"]);
    assert_eq!(before.stdout, after.stdout);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&before.stdout).unwrap()["name"],
        "jq"
    );
    let shorthand = run(&["jq", "--category", "data", "--json", "-n", "1"]);
    let explicit = run(&["search", "jq", "--category", "data", "--json", "-n", "1"]);
    assert_eq!(shorthand.stdout, explicit.stdout);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&shorthand.stdout).unwrap()[0]["name"],
        "jq"
    );
    assert!(!run(&["info", "jq", "--json", "--yaml"]).status.success());
    let batch = run(&["batch", "jq", "qzxjkvblorp", "--json", "--score", "-n", "1"]);
    assert!(batch.status.success(), "{:?}", batch.stderr);
    let batch: serde_json::Value = serde_json::from_slice(&batch.stdout).unwrap();
    assert_eq!(batch[0]["query"], "jq");
    assert_eq!(batch[0]["results"][0]["name"], "jq");
    assert!(batch[0]["results"][0]["score"].as_f64().unwrap() > 0.0);
    assert_eq!(batch[1]["results"], serde_json::json!([]));
    let stats = run(&["stats", "--json"]);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&stats.stdout).unwrap()["total"],
        3
    );
    for args in [
        vec!["jq", "-n", "0"],
        vec!["search", ""],
        vec!["batch", "jq", ""],
        vec!["trending", "--updated-since", "yesterday"],
        vec!["trending", "--updated-since", "2026-8-01"],
        vec!["trending", "--updated-since", "2026-13-01"],
    ] {
        assert_eq!(
            run(&args).status.code(),
            Some(2),
            "{args:?} must be a usage error"
        );
    }
    // Keys keep the requested order after score, and missing fields are omitted.
    let raw = run(&["jq", "--json", "--score", "--fields", "stars,install,name"]).stdout;
    let raw = String::from_utf8(raw).unwrap();
    let positions: Vec<_> = ["\"score\"", "\"stars\"", "\"name\""]
        .iter()
        .map(|key| raw.find(key).unwrap())
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{raw}");
    let projected: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(projected[0].as_object().unwrap().len(), 3, "{raw}");
    let info = run(&["info", "jq", "--yaml", "--fields", "name,stars"]);
    assert_eq!(
        String::from_utf8(info.stdout).unwrap(),
        "name: jq\nstars: 100\n"
    );
    let batch = run(&["batch", "jq", "--json", "--fields", "name"]);
    let batch: serde_json::Value = serde_json::from_slice(&batch.stdout).unwrap();
    assert_eq!(
        batch,
        serde_json::json!([{"query": "jq", "results": [{"name": "jq"}]}])
    );
    assert_eq!(run(&["jq", "--fields", "nope"]).status.code(), Some(2));

    let missing = run(&["info", "qzxjkvblorp", "--json"]);
    assert_eq!(missing.status.code(), Some(4));
    let error: serde_json::Value = serde_json::from_str(
        String::from_utf8(missing.stderr)
            .unwrap()
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(error["error"]["code"], "not_found");
    assert_eq!(error["error"]["exit_code"], 4);
    assert_eq!(
        run(&["compare", "qzxjkvblorp", "--json"]).status.code(),
        Some(4)
    );
    let no_index = Command::new(env!("CARGO_BIN_EXE_clidex"))
        .env("CLIDEX_INDEX_PATH", path.with_extension("missing.yaml"))
        .args(["stats", "--json"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(no_index.status.code(), Some(3));
    let error: serde_json::Value = serde_json::from_str(
        String::from_utf8(no_index.stderr)
            .unwrap()
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(error["error"]["code"], "index_unavailable");

    let since = run(&["trending", "--updated-since", "2026-01-01", "--json"]);
    assert!(since.status.success(), "{:?}", since.stderr);
    let stale = String::from_utf8(run(&["stats", "--json"]).stderr).unwrap();
    assert!(stale.contains("days old"), "{stale}");
    let quiet = Command::new(env!("CARGO_BIN_EXE_clidex"))
        .env("CLIDEX_INDEX_PATH", &path)
        .env("CLIDEX_NO_STALE_WARNING", "1")
        .args(["stats", "--json"])
        .output()
        .unwrap();
    assert!(quiet.stderr.is_empty(), "{:?}", quiet.stderr);
    std::fs::write(&path, "version: 1\ngenerated: test\ntools: []\n").unwrap();
    assert!(!run(&["jq", "--json"]).status.success());
    std::fs::write(&path, "not valid: [").unwrap();
    assert_eq!(run(&["stats", "--json"]).status.code(), Some(3));
    std::fs::remove_file(path).unwrap();
}

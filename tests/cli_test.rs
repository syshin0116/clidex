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
    std::fs::write(&path, "version: 1\ngenerated: test\ntools: []\n").unwrap();
    assert!(!run(&["jq", "--json"]).status.success());
    std::fs::write(&path, "not valid: [").unwrap();
    assert!(!run(&["stats", "--json"]).status.success());
    std::fs::remove_file(path).unwrap();
}

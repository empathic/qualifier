use std::path::Path;
use std::process::Command;

/// Get the path to the qualifier binary (built by cargo test).
fn qualifier_bin() -> String {
    // cargo test builds binaries in the target/debug directory
    let mut path = std::env::current_exe()
        .unwrap()
        .parent() // deps/
        .unwrap()
        .parent() // debug/
        .unwrap()
        .to_path_buf();
    path.push("qualifier");
    path.to_string_lossy().into_owned()
}

/// Run qualifier in a given directory with args, return (stdout, stderr, exit code).
fn run_qualifier(dir: &Path, args: &[&str]) -> (String, String, i32) {
    let output = Command::new(qualifier_bin())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run qualifier binary");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

// --- qualifier init ---

#[test]
fn test_init_creates_graph_file() {
    let dir = tempfile::tempdir().unwrap();
    let (stdout, _, code) = run_qualifier(dir.path(), &["init"]);

    assert_eq!(code, 0, "init should succeed");
    assert!(stdout.contains("qualifier.graph.jsonl"));

    let graph_path = dir.path().join("qualifier.graph.jsonl");
    assert!(graph_path.exists(), "graph file should be created");
}

#[test]
fn test_init_creates_gitattributes_in_git_repo() {
    let dir = tempfile::tempdir().unwrap();

    // Make it a git repo
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["init"]);

    assert_eq!(code, 0);
    assert!(stdout.contains(".gitattributes") || stdout.contains("merge=union"));

    let gitattributes = dir.path().join(".gitattributes");
    assert!(gitattributes.exists(), ".gitattributes should be created");

    let content = std::fs::read_to_string(&gitattributes).unwrap();
    assert!(content.contains("*.qual merge=union"));
}

#[test]
fn test_init_idempotent() {
    let dir = tempfile::tempdir().unwrap();

    // Run init twice
    let (_, _, code1) = run_qualifier(dir.path(), &["init"]);
    let (stdout2, _, code2) = run_qualifier(dir.path(), &["init"]);

    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert!(stdout2.contains("already exists"));
}

// --- qualifier attest + show round-trip ---

#[test]
fn test_attest_and_show_roundtrip() {
    let dir = tempfile::tempdir().unwrap();

    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "lib.rs",
            "--kind",
            "praise",
            "--score",
            "40",
            "--summary",
            "Well structured code",
            "--author",
            "test@test.com",
        ],
    );

    assert_eq!(code, 0, "attest should succeed: {stdout}");
    assert!(stdout.contains("[+40]") || stdout.contains("[40]"));
    assert!(stdout.contains("lib.rs"));

    // Verify the qual file was created (directory-level .qual for root artifacts)
    let qual_path = dir.path().join(".qual");
    assert!(qual_path.exists(), "qual file should be created");

    // Show it back
    let (show_stdout, _, show_code) = run_qualifier(dir.path(), &["show", "lib.rs"]);

    assert_eq!(show_code, 0, "show should succeed");
    assert!(show_stdout.contains("lib.rs"));
    assert!(show_stdout.contains("40"));
}

#[test]
fn test_attest_requires_summary() {
    let dir = tempfile::tempdir().unwrap();

    let (_, stderr, code) = run_qualifier(dir.path(), &["attest", "foo.rs", "--kind", "pass"]);

    assert_ne!(code, 0, "attest without summary should fail");
    assert!(
        stderr.contains("summary") || stderr.contains("required"),
        "error should mention summary: {stderr}"
    );
}

// --- qualifier score --format json ---

#[test]
fn test_score_json_output_structure() {
    let dir = tempfile::tempdir().unwrap();

    // Create an attestation first
    run_qualifier(
        dir.path(),
        &[
            "attest",
            "mod.rs",
            "--kind",
            "praise",
            "--score",
            "50",
            "--summary",
            "nice",
            "--author",
            "test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["score", "--format", "json"]);

    assert_eq!(code, 0, "score should succeed");

    // Parse JSON output
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("score --format json should produce valid JSON: {e}\ngot: {stdout}")
    });

    assert!(parsed.is_array(), "JSON output should be an array");
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty(), "should have at least one entry");

    let entry = &arr[0];
    assert!(
        entry.get("subject").is_some(),
        "entry should have 'subject'"
    );
    assert!(
        entry.get("raw_score").is_some(),
        "entry should have 'raw_score'"
    );
    assert!(
        entry.get("effective_score").is_some(),
        "entry should have 'effective_score'"
    );
    assert!(entry.get("status").is_some(), "entry should have 'status'");
}

#[test]
fn test_score_empty_project() {
    let dir = tempfile::tempdir().unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["score"]);

    assert_eq!(code, 0);
    assert!(
        stdout.contains("No qualified artifacts") || stdout.is_empty() || stdout.trim().is_empty(),
        "empty project should show no artifacts message: {stdout}"
    );
}

// --- qualifier check exit codes ---

#[test]
fn test_check_passes_with_good_scores() {
    let dir = tempfile::tempdir().unwrap();

    // Create a positive attestation
    run_qualifier(
        dir.path(),
        &[
            "attest",
            "good.rs",
            "--kind",
            "praise",
            "--score",
            "50",
            "--summary",
            "excellent",
            "--author",
            "test@test.com",
        ],
    );

    let (_, _, code) = run_qualifier(dir.path(), &["check", "--min-score", "0"]);
    assert_eq!(code, 0, "check should pass when all scores above threshold");
}

#[test]
fn test_check_fails_with_bad_scores() {
    let dir = tempfile::tempdir().unwrap();

    // Create a negative attestation
    run_qualifier(
        dir.path(),
        &[
            "attest",
            "bad.rs",
            "--kind",
            "blocker",
            "--score=-50",
            "--summary",
            "critical issue",
            "--author",
            "test@test.com",
        ],
    );

    let (_, stderr, code) = run_qualifier(dir.path(), &["check", "--min-score", "0"]);
    assert_eq!(code, 1, "check should fail when scores below threshold");
    assert!(
        stderr.contains("FAIL") || stderr.contains("below minimum"),
        "stderr should mention failure: {stderr}"
    );
}

#[test]
fn test_check_passes_empty_project() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(dir.path(), &["check"]);
    assert_eq!(code, 0, "check on empty project should pass (no artifacts)");
}

// --- qualifier attest --kind blocker uses default score ---

#[test]
fn test_attest_blocker_uses_default_score() {
    let dir = tempfile::tempdir().unwrap();

    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "vuln.rs",
            "--kind",
            "blocker",
            "--summary",
            "security vulnerability",
            "--author",
            "test@test.com",
        ],
    );

    assert_eq!(code, 0, "attest should succeed");
    // The default score for blocker is -50
    assert!(
        stdout.contains("[-50]"),
        "blocker should default to score -50: {stdout}"
    );
}

#[test]
fn test_attest_pass_uses_default_score() {
    let dir = tempfile::tempdir().unwrap();

    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "ok.rs",
            "--kind",
            "pass",
            "--summary",
            "looks good",
            "--author",
            "test@test.com",
        ],
    );

    assert_eq!(code, 0, "attest should succeed");
    // The default score for pass is +20
    assert!(
        stdout.contains("[+20]") || stdout.contains("[20]"),
        "pass should default to score +20: {stdout}"
    );
}

#[test]
fn test_attest_concern_uses_default_score() {
    let dir = tempfile::tempdir().unwrap();

    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "meh.rs",
            "--kind",
            "concern",
            "--summary",
            "could be better",
            "--author",
            "test@test.com",
        ],
    );

    assert_eq!(code, 0, "attest should succeed");
    // The default score for concern is -10
    assert!(
        stdout.contains("[-10]"),
        "concern should default to score -10: {stdout}"
    );
}

// --- qualifier show --format json ---

#[test]
fn test_show_json_output() {
    let dir = tempfile::tempdir().unwrap();

    run_qualifier(
        dir.path(),
        &[
            "attest",
            "api.rs",
            "--kind",
            "praise",
            "--score",
            "30",
            "--summary",
            "clean API",
            "--author",
            "test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["show", "api.rs", "--format", "json"]);

    assert_eq!(code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("show --format json should produce valid JSON: {e}\ngot: {stdout}")
    });

    assert_eq!(parsed["subject"], "api.rs");
    assert_eq!(parsed["raw_score"], 30);
    assert_eq!(parsed["effective_score"], 30);
    assert!(parsed["records"].is_array());
    assert_eq!(parsed["records"].as_array().unwrap().len(), 1);
}

// --- qualifier show nonexistent artifact ---

#[test]
fn test_show_nonexistent_artifact() {
    let dir = tempfile::tempdir().unwrap();

    let (_, stderr, code) = run_qualifier(dir.path(), &["show", "nonexistent.rs"]);

    assert_ne!(code, 0, "show nonexistent artifact should fail");
    assert!(
        stderr.contains("No .qual file") || stderr.contains("nonexistent"),
        "error should mention missing qual file: {stderr}"
    );
}

// --- multiple attestations on same artifact ---

#[test]
fn test_multiple_attestations_accumulate() {
    let dir = tempfile::tempdir().unwrap();

    // Add two attestations to the same artifact
    run_qualifier(
        dir.path(),
        &[
            "attest",
            "lib.rs",
            "--kind",
            "praise",
            "--score",
            "30",
            "--summary",
            "good structure",
            "--author",
            "test@test.com",
        ],
    );
    run_qualifier(
        dir.path(),
        &[
            "attest",
            "lib.rs",
            "--kind",
            "concern",
            "--score=-10",
            "--summary",
            "needs docs",
            "--author",
            "test@test.com",
        ],
    );

    // Score should reflect both (30 + -10 = 20)
    let (stdout, _, code) = run_qualifier(dir.path(), &["score", "--format", "json"]);
    assert_eq!(code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    let entry = arr.iter().find(|e| e["subject"] == "lib.rs").unwrap();
    assert_eq!(
        entry["raw_score"], 20,
        "scores should accumulate: 30 + -10 = 20"
    );
}

// --- flexible .qual file layout ---

#[test]
fn test_attest_writes_to_directory_qual_by_default() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "src/foo.rs",
            "--kind",
            "pass",
            "--summary",
            "looks good",
            "--author",
            "test@test.com",
        ],
    );

    assert_eq!(code, 0, "attest should succeed");

    // Should write to src/.qual, NOT src/foo.rs.qual
    let dir_qual = dir.path().join("src/.qual");
    let one_to_one = dir.path().join("src/foo.rs.qual");
    assert!(dir_qual.exists(), "should create directory-level .qual");
    assert!(!one_to_one.exists(), "should NOT create 1:1 .qual file");
}

#[test]
fn test_attest_respects_existing_1to1_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();

    // Pre-create a 1:1 .qual file
    std::fs::write(dir.path().join("src/foo.rs.qual"), "").unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "src/foo.rs",
            "--kind",
            "pass",
            "--summary",
            "looks good",
            "--author",
            "test@test.com",
        ],
    );

    assert_eq!(code, 0);

    // Should write to existing 1:1 file
    let content = std::fs::read_to_string(dir.path().join("src/foo.rs.qual")).unwrap();
    assert!(
        !content.is_empty(),
        "should have written to existing 1:1 file"
    );

    // Directory .qual should NOT be created
    let dir_qual = dir.path().join("src/.qual");
    assert!(
        !dir_qual.exists(),
        "should not create dir .qual when 1:1 exists"
    );
}

#[test]
fn test_attest_file_flag_override() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "src/foo.rs",
            "--kind",
            "praise",
            "--summary",
            "nice",
            "--author",
            "test@test.com",
            "--file",
            "custom.qual",
        ],
    );

    assert_eq!(code, 0);

    let custom = dir.path().join("custom.qual");
    assert!(custom.exists(), "--file should write to specified path");

    // Neither default paths should exist
    assert!(!dir.path().join("src/.qual").exists());
    assert!(!dir.path().join("src/foo.rs.qual").exists());
}

#[test]
fn test_show_finds_attestation_in_directory_qual() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();

    // Attest writes to src/.qual by default
    run_qualifier(
        dir.path(),
        &[
            "attest",
            "src/bar.rs",
            "--kind",
            "praise",
            "--score",
            "30",
            "--summary",
            "clean code",
            "--author",
            "test@test.com",
        ],
    );

    // Show should find it via discovery
    let (stdout, _, code) = run_qualifier(dir.path(), &["show", "src/bar.rs"]);

    assert_eq!(code, 0, "show should find attestation in directory .qual");
    assert!(stdout.contains("src/bar.rs"));
    assert!(stdout.contains("30"));
}

#[test]
fn test_score_accumulates_across_layouts() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();

    // First attestation → goes to src/.qual
    run_qualifier(
        dir.path(),
        &[
            "attest",
            "src/mixed.rs",
            "--kind",
            "praise",
            "--score",
            "40",
            "--summary",
            "good",
            "--author",
            "test@test.com",
        ],
    );

    // Pre-create a 1:1 file and write a second attestation via --file
    run_qualifier(
        dir.path(),
        &[
            "attest",
            "src/mixed.rs",
            "--kind",
            "concern",
            "--score=-10",
            "--summary",
            "needs work",
            "--author",
            "test@test.com",
            "--file",
            "src/mixed.rs.qual",
        ],
    );

    // Score should see both (40 + -10 = 30)
    let (stdout, _, code) = run_qualifier(dir.path(), &["score", "--format", "json"]);
    assert_eq!(code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    let entry = arr.iter().find(|e| e["subject"] == "src/mixed.rs").unwrap();
    assert_eq!(
        entry["raw_score"], 30,
        "scores should accumulate across layouts: 40 + -10 = 30"
    );
}

#[test]
fn test_attest_creates_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();

    // src/deep/ doesn't exist yet
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "src/deep/module.rs",
            "--kind",
            "pass",
            "--summary",
            "ok",
            "--author",
            "test@test.com",
        ],
    );

    assert_eq!(code, 0, "attest should create parent dirs as needed");
    assert!(dir.path().join("src/deep/.qual").exists());
}

// --- qualifier ls ---

#[test]
fn test_ls_basic_listing() {
    let dir = tempfile::tempdir().unwrap();

    run_qualifier(
        dir.path(),
        &[
            "attest",
            "foo.rs",
            "--kind",
            "praise",
            "--score",
            "50",
            "--summary",
            "great",
            "--author",
            "test@test.com",
        ],
    );
    run_qualifier(
        dir.path(),
        &[
            "attest",
            "bar.rs",
            "--kind",
            "concern",
            "--score=-20",
            "--summary",
            "meh",
            "--author",
            "test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["ls"]);
    assert_eq!(code, 0, "ls should succeed");
    assert!(stdout.contains("foo.rs"), "ls should list foo.rs");
    assert!(stdout.contains("bar.rs"), "ls should list bar.rs");
}

#[test]
fn test_ls_below_filter() {
    let dir = tempfile::tempdir().unwrap();

    run_qualifier(
        dir.path(),
        &[
            "attest",
            "good.rs",
            "--kind",
            "praise",
            "--score",
            "50",
            "--summary",
            "nice",
            "--author",
            "test@test.com",
        ],
    );
    run_qualifier(
        dir.path(),
        &[
            "attest",
            "bad.rs",
            "--kind",
            "blocker",
            "--score=-50",
            "--summary",
            "broken",
            "--author",
            "test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["ls", "--below", "0"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("bad.rs"), "below filter should show bad.rs");
    assert!(
        !stdout.contains("good.rs"),
        "below filter should hide good.rs"
    );
}

#[test]
fn test_ls_kind_filter() {
    let dir = tempfile::tempdir().unwrap();

    run_qualifier(
        dir.path(),
        &[
            "attest",
            "a.rs",
            "--kind",
            "blocker",
            "--summary",
            "bad",
            "--author",
            "test@test.com",
        ],
    );
    run_qualifier(
        dir.path(),
        &[
            "attest",
            "b.rs",
            "--kind",
            "praise",
            "--score",
            "30",
            "--summary",
            "good",
            "--author",
            "test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["ls", "--kind", "blocker"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("a.rs"), "kind filter should show blocker");
    assert!(!stdout.contains("b.rs"), "kind filter should hide praise");
}

// --- qualifier praise ---

#[test]
fn test_praise_shows_records() {
    let dir = tempfile::tempdir().unwrap();

    run_qualifier(
        dir.path(),
        &[
            "attest",
            "foo.rs",
            "--kind",
            "praise",
            "--score",
            "40",
            "--summary",
            "Well structured code",
            "--author",
            "alice@example.com",
        ],
    );

    run_qualifier(
        dir.path(),
        &[
            "attest",
            "foo.rs",
            "--kind",
            "concern",
            "--score=-10",
            "--summary",
            "Missing error handling",
            "--author",
            "bob@example.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["praise", "foo.rs"]);
    assert_eq!(code, 0, "praise should succeed");
    assert!(
        stdout.contains("foo.rs"),
        "should show artifact name: {stdout}"
    );
    assert!(
        stdout.contains("2 records"),
        "should show record count: {stdout}"
    );
    assert!(
        stdout.contains("[+40]"),
        "should show praise score: {stdout}"
    );
    assert!(
        stdout.contains("[-10]"),
        "should show concern score: {stdout}"
    );
    assert!(
        stdout.contains("alice@example.com"),
        "should show author: {stdout}"
    );
    assert!(
        stdout.contains("bob@example.com"),
        "should show second author: {stdout}"
    );
    assert!(
        stdout.contains("Well structured code"),
        "should show summary: {stdout}"
    );
}

#[test]
fn test_praise_blame_alias() {
    let dir = tempfile::tempdir().unwrap();

    run_qualifier(
        dir.path(),
        &[
            "attest",
            "foo.rs",
            "--kind",
            "pass",
            "--summary",
            "ok",
            "--author",
            "test@test.com",
        ],
    );

    let (stdout, stderr, code) = run_qualifier(dir.path(), &["blame", "foo.rs"]);
    assert_eq!(code, 0, "blame alias should succeed");
    assert!(
        stderr.contains("hint") && stderr.contains("praise"),
        "should print hint about praise: {stderr}"
    );
    assert!(
        stdout.contains("foo.rs"),
        "should still produce output: {stdout}"
    );
}

#[test]
fn test_praise_vcs_without_vcs() {
    let dir = tempfile::tempdir().unwrap();

    run_qualifier(
        dir.path(),
        &[
            "attest",
            "foo.rs",
            "--kind",
            "pass",
            "--summary",
            "ok",
            "--author",
            "test@test.com",
        ],
    );

    let (_, stderr, code) = run_qualifier(dir.path(), &["praise", "foo.rs", "--vcs"]);
    assert_ne!(code, 0, "praise --vcs should fail without VCS");
    assert!(
        stderr.contains("No VCS") || stderr.contains("--vcs"),
        "should mention VCS: {stderr}"
    );
}

// --- qualifier graph ---

#[test]
fn test_graph_dot_output() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("qualifier.graph.jsonl"),
        "{\"subject\":\"app\",\"depends_on\":[\"lib\"]}\n",
    )
    .unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["graph", "--format", "dot"]);
    assert_eq!(code, 0, "graph dot should succeed");
    assert!(stdout.contains("digraph"), "should be DOT format");
    assert!(stdout.contains("app"), "should contain app");
    assert!(stdout.contains("lib"), "should contain lib");
}

#[test]
fn test_graph_json_output() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("qualifier.graph.jsonl"),
        "{\"subject\":\"app\",\"depends_on\":[\"lib\"]}\n",
    )
    .unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["graph", "--format", "json"]);
    assert_eq!(code, 0, "graph json should succeed");
    assert!(stdout.contains("app"), "should contain app");
    assert!(stdout.contains("lib"), "should contain lib");
}

#[test]
fn test_graph_missing_file() {
    let dir = tempfile::tempdir().unwrap();

    let (_, stderr, code) = run_qualifier(dir.path(), &["graph"]);
    assert_ne!(code, 0, "graph should fail without graph file");
    assert!(
        stderr.contains("not found") || stderr.contains("Graph file"),
        "should mention missing file: {stderr}"
    );
}

// --- score overflow ---

#[test]
fn test_score_overflow_clamped() {
    let dir = tempfile::tempdir().unwrap();

    // Create 5 attestations each with score +100
    for i in 0..5 {
        run_qualifier(
            dir.path(),
            &[
                "attest",
                "big.rs",
                "--kind",
                "praise",
                "--score",
                "100",
                "--summary",
                &format!("praise {i}"),
                "--author",
                "test@test.com",
            ],
        );
    }

    let (stdout, _, code) = run_qualifier(dir.path(), &["score", "--format", "json"]);
    assert_eq!(code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    let entry = arr.iter().find(|e| e["subject"] == "big.rs").unwrap();
    assert_eq!(
        entry["raw_score"], 100,
        "raw score should be clamped to 100, not 500"
    );
}

// --- batch validation ---

#[test]
fn test_attest_batch_validates() {
    let dir = tempfile::tempdir().unwrap();

    // Pipe invalid JSONL (empty summary) into batch mode
    let invalid_json = serde_json::json!({
        "subject": "test.rs",
        "body": {
            "kind": "pass",
            "score": 10,
            "summary": ""
        },
        "author": "test@test.com",
        "created_at": "2026-01-01T00:00:00Z"
    });

    let output = std::process::Command::new(qualifier_bin())
        .args(["attest", "--stdin"])
        .current_dir(dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                writeln!(stdin, "{}", invalid_json).ok();
            }
            child.wait_with_output()
        })
        .expect("failed to run batch mode");

    assert!(
        !output.status.success(),
        "batch mode should reject invalid attestation (empty summary)"
    );
}

// --- metabox format tests ---

#[test]
fn test_attest_with_author_type() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "lib.rs",
            "--kind",
            "praise",
            "--score",
            "30",
            "--summary",
            "Clean code",
            "--author",
            "test@test.com",
            "--author-type",
            "human",
        ],
    );

    assert_eq!(code, 0, "attest with --author-type should succeed");

    // Read the .qual file and verify author_type is present
    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"author_type\":\"human\""),
        "attestation should contain author_type: {content}"
    );
}

#[test]
fn test_attest_with_ref() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "lib.rs",
            "--kind",
            "pass",
            "--score",
            "20",
            "--summary",
            "Looks good",
            "--author",
            "test@test.com",
            "--ref",
            "git:3aba500",
        ],
    );

    assert_eq!(code, 0, "attest with --ref should succeed");

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"ref\":\"git:3aba500\""),
        "attestation should contain ref: {content}"
    );
}

#[test]
fn test_new_attestations_are_metabox() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "mod.rs",
            "--kind",
            "praise",
            "--score",
            "50",
            "--summary",
            "nice",
            "--author",
            "test@test.com",
        ],
    );

    assert_eq!(code, 0);

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"metabox\":\"1\""),
        "new attestations should be metabox format: {content}"
    );
    assert!(
        content.contains("\"type\":\"attestation\""),
        "new attestations should have type field: {content}"
    );
}

#[test]
fn test_attest_invalid_author_type() {
    let dir = tempfile::tempdir().unwrap();

    let (_, stderr, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "lib.rs",
            "--kind",
            "pass",
            "--summary",
            "ok",
            "--author",
            "test@test.com",
            "--author-type",
            "banana",
        ],
    );

    assert_ne!(code, 0, "invalid author_type should fail");
    assert!(
        stderr.contains("author_type") || stderr.contains("banana"),
        "error should mention invalid author_type: {stderr}"
    );
}

// --- span tests ---

#[test]
fn test_attest_with_span() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "lib.rs",
            "--kind",
            "concern",
            "--score=-10",
            "--summary",
            "Problematic function",
            "--author",
            "test@test.com",
            "--span",
            "42:58",
        ],
    );

    assert_eq!(code, 0, "attest with --span should succeed");

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"span\""),
        "attestation should contain span: {content}"
    );
    assert!(
        content.contains("\"line\":42"),
        "span should contain start line: {content}"
    );
    assert!(
        content.contains("\"line\":58"),
        "span should contain end line: {content}"
    );
}

#[test]
fn test_attest_with_span_and_columns() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "attest",
            "lib.rs",
            "--kind",
            "concern",
            "--score=-10",
            "--summary",
            "Bad code",
            "--author",
            "test@test.com",
            "--span",
            "10.5:20.80",
        ],
    );

    assert_eq!(code, 0, "attest with --span line.col should succeed");

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"col\":5"),
        "span should contain start col: {content}"
    );
    assert!(
        content.contains("\"col\":80"),
        "span should contain end col: {content}"
    );
}

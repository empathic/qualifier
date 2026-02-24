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

    // Verify the qual file was created
    let qual_path = dir.path().join("lib.rs.qual");
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
        entry.get("artifact").is_some(),
        "entry should have 'artifact'"
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

    assert_eq!(parsed["artifact"], "api.rs");
    assert_eq!(parsed["raw_score"], 30);
    assert_eq!(parsed["effective_score"], 30);
    assert!(parsed["attestations"].is_array());
    assert_eq!(parsed["attestations"].as_array().unwrap().len(), 1);
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
    let entry = arr.iter().find(|e| e["artifact"] == "lib.rs").unwrap();
    assert_eq!(
        entry["raw_score"], 20,
        "scores should accumulate: 30 + -10 = 20"
    );
}

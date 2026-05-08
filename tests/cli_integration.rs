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

// --- qualifier record + show round-trip ---

#[test]
fn test_record_and_show_roundtrip() {
    let dir = tempfile::tempdir().unwrap();

    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "praise",
            "lib.rs",
            "Well structured code",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    assert_eq!(code, 0, "record should succeed: {stdout}");
    assert!(stdout.contains("lib.rs"));

    // Verify the qual file was created (directory-level .qual for root artifacts)
    let qual_path = dir.path().join(".qual");
    assert!(qual_path.exists(), "qual file should be created");

    // Show it back
    let (show_stdout, _, show_code) = run_qualifier(dir.path(), &["show", "lib.rs"]);

    assert_eq!(show_code, 0, "show should succeed");
    assert!(show_stdout.contains("lib.rs"));
}

#[test]
fn test_record_requires_message() {
    let dir = tempfile::tempdir().unwrap();

    let (_, stderr, code) = run_qualifier(dir.path(), &["record", "pass", "foo.rs"]);

    assert_ne!(code, 0, "record without message should fail");
    assert!(
        stderr.contains("message") || stderr.contains("required"),
        "error should mention message: {stderr}"
    );
}

// --- qualifier show --format json ---

#[test]
fn test_show_json_output() {
    let dir = tempfile::tempdir().unwrap();

    run_qualifier(
        dir.path(),
        &[
            "record",
            "praise",
            "api.rs",
            "clean API",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["show", "api.rs", "--format", "json"]);

    assert_eq!(code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("show --format json should produce valid JSON: {e}\ngot: {stdout}")
    });

    assert_eq!(parsed["subject"], "api.rs");
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

// --- flexible .qual file layout ---

#[test]
fn test_record_writes_to_directory_qual_by_default() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "pass",
            "src/foo.rs",
            "looks good",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    assert_eq!(code, 0, "record should succeed");

    // Should write to src/.qual, NOT src/foo.rs.qual
    let dir_qual = dir.path().join("src/.qual");
    let one_to_one = dir.path().join("src/foo.rs.qual");
    assert!(dir_qual.exists(), "should create directory-level .qual");
    assert!(!one_to_one.exists(), "should NOT create 1:1 .qual file");
}

#[test]
fn test_record_respects_existing_1to1_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();

    // Pre-create a 1:1 .qual file
    std::fs::write(dir.path().join("src/foo.rs.qual"), "").unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "pass",
            "src/foo.rs",
            "looks good",
            "--issuer",
            "mailto:test@test.com",
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
fn test_record_file_flag_override() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "praise",
            "src/foo.rs",
            "nice",
            "--issuer",
            "mailto:test@test.com",
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
fn test_show_finds_annotation_in_directory_qual() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();

    // Record writes to src/.qual by default
    run_qualifier(
        dir.path(),
        &[
            "record",
            "praise",
            "src/bar.rs",
            "clean code",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    // Show should find it via discovery
    let (stdout, _, code) = run_qualifier(dir.path(), &["show", "src/bar.rs"]);

    assert_eq!(code, 0, "show should find annotation in directory .qual");
    assert!(stdout.contains("src/bar.rs"));
}

#[test]
fn test_record_creates_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();

    // src/deep/ doesn't exist yet
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "pass",
            "src/deep/module.rs",
            "ok",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    assert_eq!(code, 0, "record should create parent dirs as needed");
    assert!(dir.path().join("src/deep/.qual").exists());
}

// --- qualifier ls ---

#[test]
fn test_ls_basic_listing() {
    let dir = tempfile::tempdir().unwrap();

    run_qualifier(
        dir.path(),
        &[
            "record",
            "praise",
            "foo.rs",
            "great",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "bar.rs",
            "meh",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["ls"]);
    assert_eq!(code, 0, "ls should succeed");
    assert!(stdout.contains("foo.rs"), "ls should list foo.rs");
    assert!(stdout.contains("bar.rs"), "ls should list bar.rs");
}

#[test]
fn test_ls_kind_filter() {
    let dir = tempfile::tempdir().unwrap();

    run_qualifier(
        dir.path(),
        &[
            "record",
            "blocker",
            "a.rs",
            "bad",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    run_qualifier(
        dir.path(),
        &[
            "record",
            "praise",
            "b.rs",
            "good",
            "--issuer",
            "mailto:test@test.com",
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
            "record",
            "praise",
            "foo.rs",
            "Well structured code",
            "--issuer",
            "mailto:alice@example.com",
        ],
    );

    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "foo.rs",
            "Missing error handling",
            "--issuer",
            "mailto:bob@example.com",
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
        stdout.contains("alice@example.com"),
        "should show issuer: {stdout}"
    );
    assert!(
        stdout.contains("bob@example.com"),
        "should show second issuer: {stdout}"
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
            "record",
            "pass",
            "foo.rs",
            "ok",
            "--issuer",
            "mailto:test@test.com",
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
            "record",
            "pass",
            "foo.rs",
            "ok",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let (_, stderr, code) = run_qualifier(dir.path(), &["praise", "foo.rs", "--vcs"]);
    assert_ne!(code, 0, "praise --vcs should fail without VCS");
    assert!(
        stderr.contains("No VCS") || stderr.contains("--vcs"),
        "should mention VCS: {stderr}"
    );
}

// --- batch validation ---

#[test]
fn test_record_batch_validates() {
    let dir = tempfile::tempdir().unwrap();

    // Pipe an invalid overrides line (empty message) into batch mode.
    let invalid_json = serde_json::json!({
        "kind": "pass",
        "location": "test.rs",
        "message": ""
    });

    let output = std::process::Command::new(qualifier_bin())
        .args(["record", "--stdin"])
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
        "batch mode should reject invalid annotation (empty summary)"
    );
}

#[test]
fn test_record_batch_full_record_form() {
    let dir = tempfile::tempdir().unwrap();

    // A complete record (envelope + body) — the older batch form.
    let invalid_json = serde_json::json!({
        "subject": "test.rs",
        "body": {
            "kind": "pass",
            "summary": ""
        },
        "issuer": "mailto:test@test.com",
        "created_at": "2026-01-01T00:00:00Z"
    });

    let output = std::process::Command::new(qualifier_bin())
        .args(["record", "--stdin"])
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
        "batch mode should reject full record with empty summary"
    );
}

// --- metabox format tests ---

#[test]
fn test_record_with_issuer_type() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "praise",
            "lib.rs",
            "Clean code",
            "--issuer",
            "mailto:test@test.com",
            "--issuer-type",
            "human",
        ],
    );

    assert_eq!(code, 0, "record with --issuer-type should succeed");

    // Read the .qual file and verify issuer_type is present
    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"issuer_type\":\"human\""),
        "annotation should contain issuer_type: {content}"
    );
}

#[test]
fn test_record_with_ref() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "pass",
            "lib.rs",
            "Looks good",
            "--issuer",
            "mailto:test@test.com",
            "--ref",
            "git:3aba500",
        ],
    );

    assert_eq!(code, 0, "record with --ref should succeed");

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"ref\":\"git:3aba500\""),
        "annotation should contain ref: {content}"
    );
}

#[test]
fn test_new_annotations_are_metabox() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "praise",
            "mod.rs",
            "nice",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    assert_eq!(code, 0);

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"metabox\":\"1\""),
        "new annotations should be metabox format: {content}"
    );
    assert!(
        content.contains("\"type\":\"annotation\""),
        "new annotations should have type field: {content}"
    );
}

#[test]
fn test_record_invalid_issuer_type() {
    let dir = tempfile::tempdir().unwrap();

    let (_, stderr, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "pass",
            "lib.rs",
            "ok",
            "--issuer",
            "mailto:test@test.com",
            "--issuer-type",
            "banana",
        ],
    );

    assert_ne!(code, 0, "invalid issuer_type should fail");
    assert!(
        stderr.contains("issuer_type") || stderr.contains("banana"),
        "error should mention invalid issuer_type: {stderr}"
    );
}

// --- span tests ---

#[test]
fn test_record_with_span() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "lib.rs",
            "Problematic function",
            "--issuer",
            "mailto:test@test.com",
            "--span",
            "42:58",
        ],
    );

    assert_eq!(code, 0, "record with --span should succeed");

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"span\""),
        "annotation should contain span: {content}"
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
fn test_record_span_via_location() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "lib.rs:42:58",
            "Problematic function",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    assert_eq!(code, 0, "record with location-encoded span should succeed");

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
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
fn test_record_with_span_and_columns() {
    let dir = tempfile::tempdir().unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "lib.rs",
            "Bad code",
            "--issuer",
            "mailto:test@test.com",
            "--span",
            "10.5:20.80",
        ],
    );

    assert_eq!(code, 0, "record with --span line.col should succeed");

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

// --- show --pretty tests ---

#[test]
fn test_show_pretty_shows_source() {
    let dir = tempfile::tempdir().unwrap();

    // Create a source file with known content
    let src = dir.path().join("example.rs");
    std::fs::write(
        &src,
        "fn main() {\n    let x = 1;\n    let y = 2;\n    let z = 3;\n    println!(\"{}\", x + y + z);\n}\n",
    )
    .unwrap();

    // Record with a span via location syntax
    run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "example.rs:3",
            "needs a better name",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["show", "example.rs", "--pretty"]);

    assert_eq!(code, 0, "show --pretty should succeed: {stdout}");
    assert!(
        stdout.contains("let y = 2"),
        "pretty output should show source line: {stdout}"
    );
    assert!(
        stdout.contains(">"),
        "pretty output should have > marker: {stdout}"
    );
    assert!(
        stdout.contains("example.rs"),
        "pretty output should show file path: {stdout}"
    );
}

#[test]
fn test_show_pretty_json() {
    let dir = tempfile::tempdir().unwrap();

    let src = dir.path().join("example.rs");
    std::fs::write(
        &src,
        "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\n",
    )
    .unwrap();

    // Record with a span
    run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "example.rs:3",
            "check this",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &["show", "example.rs", "--format", "json", "--pretty"],
    );

    assert_eq!(code, 0, "show --pretty --format json should succeed");

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("should produce valid JSON: {e}\ngot: {stdout}"));

    let records = parsed["records"]
        .as_array()
        .expect("should have records array");
    assert!(!records.is_empty(), "should have at least one record");

    let rec = &records[0];
    assert!(
        rec.get("context").is_some(),
        "record should have context field: {stdout}"
    );
    let context = &rec["context"];
    assert!(
        context["lines"].is_array(),
        "context should have lines array: {stdout}"
    );

    let lines = context["lines"].as_array().unwrap();
    let span_line = lines.iter().find(|l| l["in_span"] == true);
    assert!(span_line.is_some(), "should have an in_span line: {stdout}");
    assert_eq!(span_line.unwrap()["line"], 3);
}

#[test]
fn test_show_pretty_file_not_found() {
    let dir = tempfile::tempdir().unwrap();

    // Record on a nonexistent file with a span
    run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "nonexistent.rs:5",
            "some comment",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["show", "nonexistent.rs", "--pretty"]);

    assert_eq!(
        code, 0,
        "show --pretty should succeed even without source file"
    );
    assert!(
        stdout.contains("note:"),
        "should show a note about missing file: {stdout}"
    );
}

#[test]
fn test_show_pretty_no_span() {
    let dir = tempfile::tempdir().unwrap();

    let src = dir.path().join("example.rs");
    std::fs::write(&src, "fn main() {}\n").unwrap();

    // Record without span
    run_qualifier(
        dir.path(),
        &[
            "record",
            "pass",
            "example.rs",
            "general comment",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["show", "example.rs", "--pretty"]);

    assert_eq!(code, 0, "show --pretty without span should succeed");
    // Should not show source context markers
    assert!(
        !stdout.contains("> "),
        "no span means no source context markers: {stdout}"
    );
}

// --- references tests ---

#[test]
fn test_record_with_references() {
    let dir = tempfile::tempdir().unwrap();

    // Create an initial annotation to reference
    let (stdout1, _, code1) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "lib.rs",
            "Needs improvement",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(code1, 0, "first record should succeed: {stdout1}");

    // Extract the ID from the output (line: "  id: <hash>")
    let id = stdout1
        .lines()
        .find(|l| l.contains("id:"))
        .and_then(|l| l.split("id:").nth(1))
        .map(|s| s.trim().to_string())
        .expect("should find id in output");

    // Create a referencing annotation
    let (_, _, code2) = run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "lib.rs",
            "Addressed in latest refactor",
            "--issuer",
            "mailto:test@test.com",
            "--references",
            &id,
        ],
    );
    assert_eq!(code2, 0, "record with --references should succeed");

    // Verify the .qual file contains the references field
    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains(&format!("\"references\":\"{id}\"")),
        "annotation should contain references field: {content}"
    );
}

#[test]
fn test_show_displays_references() {
    let dir = tempfile::tempdir().unwrap();

    // Create an initial annotation
    let (stdout1, _, _) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "lib.rs",
            "Needs work",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let id = stdout1
        .lines()
        .find(|l| l.contains("id:"))
        .and_then(|l| l.split("id:").nth(1))
        .map(|s| s.trim().to_string())
        .expect("should find id in output");

    // Create a referencing annotation
    run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "lib.rs",
            "This was fixed",
            "--issuer",
            "mailto:test@test.com",
            "--references",
            &id,
        ],
    );

    // Show should thread the referencing record under its parent
    let (stdout, _, code) = run_qualifier(dir.path(), &["show", "lib.rs"]);
    assert_eq!(code, 0, "show should succeed");
    assert!(
        stdout.contains("This was fixed"),
        "show output should contain referencing record: {stdout}"
    );
    // The referencing record should appear after and indented under the parent
    let lines: Vec<&str> = stdout.lines().collect();
    let parent_line = lines.iter().position(|l| l.contains("Needs work"));
    let ref_line = lines.iter().position(|l| l.contains("This was fixed"));
    assert!(
        parent_line.is_some() && ref_line.is_some(),
        "should find both parent and referencing record: {stdout}"
    );
    assert!(
        ref_line.unwrap() > parent_line.unwrap(),
        "referencing record should appear after parent: {stdout}"
    );
}

// --- reply command tests ---

#[test]
fn test_reply_basic() {
    let dir = tempfile::tempdir().unwrap();

    // Create an initial annotation to reply to
    let (stdout1, _, code1) = run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "lib.rs",
            "needs improvement",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(code1, 0, "initial comment should succeed: {stdout1}");

    let id = stdout1
        .lines()
        .find(|l| l.contains("id:"))
        .and_then(|l| l.split("id:").nth(1))
        .map(|s| s.trim().to_string())
        .expect("should find id in output");

    // Reply using short prefix (first 8 chars)
    let prefix = &id[..8];
    let (stdout2, _, code2) = run_qualifier(
        dir.path(),
        &[
            "reply",
            prefix,
            "fixed this",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(code2, 0, "reply should succeed: {stdout2}");
    assert!(
        stdout2.contains("re:"),
        "reply output should show re: line: {stdout2}"
    );
    assert!(
        stdout2.contains(prefix),
        "reply output should show referenced prefix: {stdout2}"
    );

    // Verify the .qual file contains the references field
    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains(&format!("\"references\":\"{id}\"")),
        "reply should set references to full target ID: {content}"
    );

    // Show should thread the reply under the parent (indented)
    let (show_stdout, _, show_code) = run_qualifier(dir.path(), &["show", "lib.rs"]);
    assert_eq!(show_code, 0);
    assert!(
        show_stdout.contains("fixed this"),
        "show output should contain reply text: {show_stdout}"
    );
    // Reply should appear indented under its parent
    let lines: Vec<&str> = show_stdout.lines().collect();
    let parent_line = lines.iter().position(|l| l.contains("needs improvement"));
    let reply_line = lines.iter().position(|l| l.contains("fixed this"));
    assert!(
        parent_line.is_some() && reply_line.is_some(),
        "should find both parent and reply in output: {show_stdout}"
    );
    assert!(
        reply_line.unwrap() > parent_line.unwrap(),
        "reply should appear after parent: {show_stdout}"
    );
}

#[test]
fn test_reply_inherits_subject() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();

    // Create an annotation on src/parser.rs
    let (stdout1, _, code1) = run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "src/parser.rs",
            "needs refactoring",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(code1, 0);

    let id = stdout1
        .lines()
        .find(|l| l.contains("id:"))
        .and_then(|l| l.split("id:").nth(1))
        .map(|s| s.trim().to_string())
        .expect("should find id in output");

    // Reply — should inherit src/parser.rs as subject
    let (stdout2, _, code2) = run_qualifier(
        dir.path(),
        &[
            "reply",
            &id[..8],
            "refactored",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(code2, 0, "reply should succeed: {stdout2}");
    assert!(
        stdout2.contains("src/parser.rs"),
        "reply should inherit subject from target: {stdout2}"
    );

    // Show src/parser.rs should find both records
    let (show_stdout, _, show_code) =
        run_qualifier(dir.path(), &["show", "src/parser.rs", "--format", "json"]);
    assert_eq!(show_code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&show_stdout).unwrap();
    let records = parsed["records"].as_array().unwrap();
    assert_eq!(
        records.len(),
        2,
        "should have 2 records for src/parser.rs: {show_stdout}"
    );
}

#[test]
fn test_reply_not_found() {
    let dir = tempfile::tempdir().unwrap();

    let (_, stderr, code) = run_qualifier(
        dir.path(),
        &[
            "reply",
            "deadbeef",
            "hello",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_ne!(code, 0, "reply to nonexistent ID should fail");
    assert!(
        stderr.contains("no record found"),
        "error should mention no record found: {stderr}"
    );
}

#[test]
fn test_reply_with_kind_override() {
    let dir = tempfile::tempdir().unwrap();

    // Create initial annotation
    let (stdout1, _, _) = run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "lib.rs",
            "issue here",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let id = stdout1
        .lines()
        .find(|l| l.contains("id:"))
        .and_then(|l| l.split("id:").nth(1))
        .map(|s| s.trim().to_string())
        .expect("should find id in output");

    // Reply with kind override
    let (stdout2, _, code2) = run_qualifier(
        dir.path(),
        &[
            "reply",
            &id[..8],
            "approved the fix",
            "--kind",
            "pass",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(
        code2, 0,
        "reply with --kind override should succeed: {stdout2}"
    );
    assert!(
        stdout2.contains("pass"),
        "reply should use overridden kind: {stdout2}"
    );
}

#[test]
fn test_reply_by_location() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/auth.rs"),
        "fn login() {}\nfn logout() {}\n",
    )
    .unwrap();

    // Create an annotation at src/auth.rs:1
    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "src/auth.rs:1",
            "needs validation",
            "--issuer",
            "mailto:alice@test.com",
        ],
    );

    // Reply by location (no id-prefix needed)
    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &[
            "reply",
            "src/auth.rs:1",
            "added validation",
            "--issuer",
            "mailto:bob@test.com",
        ],
    );
    assert_eq!(code, 0, "reply by location should succeed: {stdout}");
    assert!(
        stdout.contains("re:"),
        "reply output should show re: line: {stdout}"
    );
}

#[test]
fn test_reply_by_location_ambiguous() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/auth.rs"),
        "fn login() {}\nfn logout() {}\n",
    )
    .unwrap();

    // Create two annotations on the same subject (no spans).
    // They have the same created_at second resolution and the same span
    // (both whole-file), so the resolver should return the first match
    // when there's no span filter — but two records means more than one
    // candidate. Both will share the same timestamp tier => ambiguous.
    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "src/auth.rs",
            "first concern",
            "--issuer",
            "mailto:alice@test.com",
        ],
    );
    // Sleep just a hair to nudge timestamps... but also create at a different
    // location so this is simply two distinct records.
    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "src/auth.rs",
            "second concern",
            "--issuer",
            "mailto:bob@test.com",
        ],
    );

    // Reply by bare subject path. The resolver should succeed by picking
    // the most-recent record by created_at. (If timestamps tie, it would
    // surface a disambiguation error — that's acceptable behavior here too.)
    let (stdout, stderr, code) = run_qualifier(
        dir.path(),
        &[
            "reply",
            "src/auth.rs",
            "responding",
            "--issuer",
            "mailto:carol@test.com",
        ],
    );
    // Either succeeded by picking newest, or surfaced an ambiguity hint.
    if code != 0 {
        assert!(
            stderr.contains("ambiguous"),
            "should either succeed or surface ambiguity: stdout={stdout} stderr={stderr}"
        );
    } else {
        assert!(stdout.contains("re:"), "should show re: line: {stdout}");
    }
}

// --- show threading tests ---

#[test]
fn test_show_threads_replies_under_parent() {
    let dir = tempfile::tempdir().unwrap();

    // Create two independent comments
    let (stdout1, _, _) = run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "lib.rs",
            "first issue",
            "--issuer",
            "mailto:alice@test.com",
        ],
    );
    let id1 = stdout1
        .lines()
        .find(|l| l.contains("id:"))
        .and_then(|l| l.split("id:").nth(1))
        .map(|s| s.trim().to_string())
        .expect("should find id");

    run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "lib.rs",
            "second issue",
            "--issuer",
            "mailto:bob@test.com",
        ],
    );

    // Reply to the first comment
    run_qualifier(
        dir.path(),
        &[
            "reply",
            &id1[..8],
            "fixed first issue",
            "--issuer",
            "mailto:bob@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["show", "lib.rs"]);
    assert_eq!(code, 0);

    let lines: Vec<&str> = stdout.lines().collect();
    let first_pos = lines
        .iter()
        .position(|l| l.contains("first issue") && !l.contains("fixed"));
    let reply_pos = lines.iter().position(|l| l.contains("fixed first issue"));
    let second_pos = lines.iter().position(|l| l.contains("second issue"));

    assert!(first_pos.is_some(), "should find first issue: {stdout}");
    assert!(reply_pos.is_some(), "should find reply: {stdout}");
    assert!(second_pos.is_some(), "should find second issue: {stdout}");

    // Reply should appear between the first and second issues (threaded under first)
    let first = first_pos.unwrap();
    let reply = reply_pos.unwrap();
    let second = second_pos.unwrap();
    assert!(
        reply > first && reply < second,
        "reply should be threaded between first ({first}) and second ({second}), got reply at {reply}: {stdout}"
    );

    // Reply line should have a tree-drawing character
    let reply_line_text = lines[reply];
    assert!(
        reply_line_text.contains('\u{2514}') || reply_line_text.contains('\u{251c}'),
        "reply should have tree branch character: {reply_line_text}"
    );
}

// --- resolve command tests ---

#[test]
fn test_resolve_basic() {
    let dir = tempfile::tempdir().unwrap();

    // Create an initial annotation to resolve
    let (stdout1, _, code1) = run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "lib.rs",
            "needs improvement",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(code1, 0, "initial comment should succeed: {stdout1}");

    let id = stdout1
        .lines()
        .find(|l| l.contains("id:"))
        .and_then(|l| l.split("id:").nth(1))
        .map(|s| s.trim().to_string())
        .expect("should find id in output");

    // Resolve using short prefix
    let prefix = &id[..8];
    let (stdout2, _, code2) = run_qualifier(
        dir.path(),
        &[
            "resolve",
            prefix,
            "fixed in PR #42",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(code2, 0, "resolve should succeed: {stdout2}");
    assert!(
        stdout2.contains("resolve"),
        "output should show resolve kind: {stdout2}"
    );
    assert!(
        stdout2.contains("supersedes:"),
        "output should show supersedes line: {stdout2}"
    );

    // Show should hide both the original (superseded) and the tombstone by default
    let (show_stdout, _, show_code) = run_qualifier(dir.path(), &["show", "lib.rs"]);
    assert_eq!(show_code, 0);
    assert!(
        !show_stdout.contains("needs improvement"),
        "superseded record should be hidden from show: {show_stdout}"
    );
    assert!(
        !show_stdout.contains("fixed in PR #42"),
        "resolve tombstone should be hidden by default: {show_stdout}"
    );
    assert!(
        show_stdout.contains("Records (0)"),
        "no active records should remain: {show_stdout}"
    );

    // Show --all should display the tombstone
    let (show_all_stdout, _, show_all_code) =
        run_qualifier(dir.path(), &["show", "lib.rs", "--all"]);
    assert_eq!(show_all_code, 0);
    assert!(
        show_all_stdout.contains("resolve"),
        "tombstone should appear with --all: {show_all_stdout}"
    );
    assert!(
        show_all_stdout.contains("needs improvement"),
        "superseded record should appear with --all: {show_all_stdout}"
    );
}

#[test]
fn test_resolve_default_message() {
    let dir = tempfile::tempdir().unwrap();

    let (stdout1, _, _) = run_qualifier(
        dir.path(),
        &[
            "record",
            "comment",
            "lib.rs",
            "some issue",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let id = stdout1
        .lines()
        .find(|l| l.contains("id:"))
        .and_then(|l| l.split("id:").nth(1))
        .map(|s| s.trim().to_string())
        .expect("should find id in output");

    // Resolve without message — should default to "Resolved"
    let (stdout2, _, code2) = run_qualifier(
        dir.path(),
        &["resolve", &id[..8], "--issuer", "mailto:test@test.com"],
    );
    assert_eq!(
        code2, 0,
        "resolve without message should succeed: {stdout2}"
    );
    assert!(
        stdout2.contains("Resolved"),
        "default message should be 'Resolved': {stdout2}"
    );
}

#[test]
fn test_resolve_not_found() {
    let dir = tempfile::tempdir().unwrap();

    let (_, stderr, code) = run_qualifier(
        dir.path(),
        &["resolve", "deadbeef", "--issuer", "mailto:test@test.com"],
    );
    assert_ne!(code, 0, "resolve with nonexistent ID should fail");
    assert!(
        stderr.contains("no record found"),
        "error should mention no record found: {stderr}"
    );
}

#[test]
fn test_resolve_by_location() {
    let dir = tempfile::tempdir().unwrap();

    // Create a concern at a specific span.
    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "lib.rs:42",
            "buggy",
            "--issuer",
            "mailto:alice@test.com",
        ],
    );

    // Resolve by location — no id-prefix needed.
    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &[
            "resolve",
            "lib.rs:42",
            "fixed",
            "--issuer",
            "mailto:bob@test.com",
        ],
    );
    assert_eq!(code, 0, "resolve by location should succeed: {stdout}");
    assert!(stdout.contains("resolve"), "should show resolve kind");
    assert!(
        stdout.contains("supersedes:"),
        "should show supersedes line: {stdout}"
    );
}

// --- content_hash auto-population tests ---

#[test]
fn test_record_concern_auto_populates_content_hash() {
    let dir = tempfile::tempdir().unwrap();

    // Create a source file
    let src = dir.path().join("example.rs");
    std::fs::write(
        &src,
        "fn main() {\n    let x = 1;\n    let y = 2;\n    println!(\"{}\", x + y);\n}\n",
    )
    .unwrap();

    // Record concern with a span via location
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "example.rs:2",
            "needs a better name",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(code, 0, "record concern should succeed");

    // Read the .qual file and verify content_hash is present
    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"content_hash\":"),
        "annotation should contain content_hash: {content}"
    );
}

#[test]
fn test_record_suggestion_auto_populates_content_hash() {
    let dir = tempfile::tempdir().unwrap();

    let src = dir.path().join("lib.rs");
    std::fs::write(&src, "fn foo() {}\nfn bar() {}\nfn baz() {}\n").unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "suggestion",
            "lib.rs:1:2",
            "Consider combining these",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(code, 0, "record suggestion should succeed");

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"content_hash\":"),
        "annotation should contain content_hash: {content}"
    );
}

#[test]
fn test_record_span_flag_auto_populates_content_hash() {
    let dir = tempfile::tempdir().unwrap();

    let src = dir.path().join("lib.rs");
    std::fs::write(&src, "line 1\nline 2\nline 3\n").unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "lib.rs",
            "issue here",
            "--issuer",
            "mailto:test@test.com",
            "--span",
            "2",
        ],
    );
    assert_eq!(code, 0, "record with --span should succeed");

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"content_hash\":"),
        "annotation should contain content_hash: {content}"
    );
}

#[test]
fn test_no_content_hash_when_file_missing() {
    let dir = tempfile::tempdir().unwrap();

    // Record on a nonexistent file with a span
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "nonexistent.rs:5",
            "some concern",
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(code, 0, "record should succeed even without file");

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        !content.contains("\"content_hash\":"),
        "should not have content_hash when file doesn't exist: {content}"
    );
}

// --- qualifier review (freshness) tests ---

#[test]
fn test_review_empty_project() {
    let dir = tempfile::tempdir().unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["review"]);
    assert_eq!(code, 0, "review on empty project should succeed");
    assert!(
        stdout.contains("No .qual files") || stdout.contains("No annotations"),
        "should indicate nothing to check: {stdout}"
    );
}

#[test]
fn test_review_fresh() {
    let dir = tempfile::tempdir().unwrap();

    let src = dir.path().join("example.rs");
    std::fs::write(&src, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "example.rs:2",
            "consider logging instead",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["review"]);
    assert_eq!(code, 0, "review should succeed");
    assert!(
        stdout.contains("FRESH"),
        "unchanged code should be FRESH: {stdout}"
    );
    assert!(
        stdout.contains("1 fresh"),
        "summary should show 1 fresh: {stdout}"
    );
}

#[test]
fn test_review_drifted() {
    let dir = tempfile::tempdir().unwrap();

    let src = dir.path().join("example.rs");
    std::fs::write(&src, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "example.rs:2",
            "consider logging",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    // Modify the file
    std::fs::write(&src, "fn main() {\n    eprintln!(\"changed\");\n}\n").unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["review"]);
    assert_eq!(code, 0, "review should succeed");
    assert!(
        stdout.contains("DRIFTED"),
        "changed code should be DRIFTED: {stdout}"
    );
    assert!(
        stdout.contains("1 drifted"),
        "summary should show 1 drifted: {stdout}"
    );
}

#[test]
fn test_review_missing() {
    let dir = tempfile::tempdir().unwrap();

    let src = dir.path().join("example.rs");
    std::fs::write(&src, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "example.rs:2",
            "consider logging",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    // Delete the file
    std::fs::remove_file(&src).unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["review"]);
    assert_eq!(code, 0, "review should succeed");
    assert!(
        stdout.contains("MISSING"),
        "deleted file should be MISSING: {stdout}"
    );
    assert!(
        stdout.contains("1 missing"),
        "summary should show 1 missing: {stdout}"
    );
}

#[test]
fn test_review_json_output() {
    let dir = tempfile::tempdir().unwrap();

    let src = dir.path().join("example.rs");
    std::fs::write(&src, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "example.rs:2",
            "consider logging",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["review", "--format", "json"]);
    assert_eq!(code, 0, "review --format json should succeed");

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("review --format json should produce valid JSON: {e}\ngot: {stdout}")
    });

    assert!(parsed.is_array(), "JSON output should be an array");
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1, "should have one result");

    let entry = &arr[0];
    assert_eq!(entry["status"], "fresh");
    assert_eq!(entry["kind"], "concern");
    assert!(entry["subject"].as_str().unwrap().contains("example.rs"));
}

#[test]
fn test_review_subject_filter() {
    let dir = tempfile::tempdir().unwrap();

    let src1 = dir.path().join("a.rs");
    std::fs::write(&src1, "fn a() {}\n").unwrap();

    let src2 = dir.path().join("b.rs");
    std::fs::write(&src2, "fn b() {}\n").unwrap();

    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "a.rs:1",
            "issue in a",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "b.rs:1",
            "issue in b",
            "--issuer",
            "mailto:test@test.com",
        ],
    );

    // Review only a.rs
    let (stdout, _, code) = run_qualifier(dir.path(), &["review", "a.rs"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("1 annotations checked"),
        "should only check 1 annotation: {stdout}"
    );
}

// --- qualifier emit (raw record write) ---

#[test]
fn test_emit_annotation() {
    let dir = tempfile::tempdir().unwrap();

    let body = serde_json::json!({
        "kind": "pass",
        "summary": "looks good"
    });

    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &[
            "emit",
            "annotation",
            "lib.rs",
            "--body",
            &body.to_string(),
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_eq!(code, 0, "emit annotation should succeed: {stdout}");
    assert!(stdout.contains("annotation"), "output should mention type");

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"type\":\"annotation\""),
        "should emit annotation type: {content}"
    );
    assert!(
        content.contains("\"kind\":\"pass\""),
        "should preserve body: {content}"
    );
}

#[test]
fn test_emit_unknown_type_roundtrips() {
    let dir = tempfile::tempdir().unwrap();

    let body = serde_json::json!({"foo": "bar", "version": 1});

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "emit",
            "https://example.com/custom/v1",
            "widget.rs",
            "--body",
            &body.to_string(),
            "--issuer",
            "https://ci.example.com",
        ],
    );
    assert_eq!(code, 0, "emit custom type should succeed");

    let qual_path = dir.path().join(".qual");
    let content = std::fs::read_to_string(&qual_path).unwrap();
    assert!(
        content.contains("\"type\":\"https://example.com/custom/v1\""),
        "should preserve custom type URI: {content}"
    );
    assert!(
        content.contains("\"foo\":\"bar\""),
        "should preserve body verbatim: {content}"
    );
}

#[test]
fn test_emit_annotation_validates_body() {
    let dir = tempfile::tempdir().unwrap();

    // Body missing required `kind` field for an annotation
    let body = serde_json::json!({"summary": "missing kind"});

    let (_, stderr, code) = run_qualifier(
        dir.path(),
        &[
            "emit",
            "annotation",
            "lib.rs",
            "--body",
            &body.to_string(),
            "--issuer",
            "mailto:test@test.com",
        ],
    );
    assert_ne!(
        code, 0,
        "emit annotation with invalid body should fail: stderr={stderr}"
    );
}

// --- substrate / unknown record types (spec §2.5, §3.5) ---

/// Helper: write a .qual file containing a single Unknown-type record
/// alongside a normal annotation, returning the (custom_id, annotation_id).
fn write_qual_with_unknown(dir: &Path, qual_rel_path: &str, subject: &str) -> (String, String) {
    let custom_id = "f".repeat(64);
    let custom_record = serde_json::json!({
        "metabox": "1",
        "type": "https://example.com/custom/v1",
        "subject": subject,
        "issuer": "https://ci.example.com",
        "created_at": "2026-04-01T00:00:00Z",
        "id": custom_id,
        "body": {"foo": "bar"}
    });

    // Also write an ordinary annotation alongside the unknown record.
    let qual_path = dir.join(qual_rel_path);
    if let Some(parent) = qual_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(
        &qual_path,
        format!("{}\n", serde_json::to_string(&custom_record).unwrap()),
    )
    .unwrap();

    // Append a real annotation via the CLI.
    let (_, _, code) = run_qualifier(
        dir,
        &[
            "record",
            "praise",
            subject,
            "looks good",
            "--issuer",
            "mailto:test@test.com",
            "--file",
            qual_rel_path,
        ],
    );
    assert_eq!(code, 0, "record helper should succeed");

    // Read the annotation id back from the file (the second JSONL line).
    let contents = std::fs::read_to_string(&qual_path).unwrap();
    let annotation_line = contents
        .lines()
        .find(|l| l.contains("\"type\":\"annotation\""))
        .expect("expected an annotation line in the qual file");
    let annotation_value: serde_json::Value = serde_json::from_str(annotation_line).unwrap();
    let annotation_id = annotation_value["id"].as_str().unwrap().to_string();

    (custom_id, annotation_id)
}

#[test]
fn test_show_preserves_unknown_record_type() {
    let dir = tempfile::tempdir().unwrap();
    let (_custom_id, _annotation_id) = write_qual_with_unknown(dir.path(), "lib.rs.qual", "lib.rs");

    // Human format must not crash and must include the subject + the
    // recognized annotation. The Unknown record shows up as a brief line.
    let (stdout, stderr, code) = run_qualifier(dir.path(), &["show", "lib.rs"]);
    assert_eq!(
        code, 0,
        "show must not crash on unknown record type. stderr={stderr}"
    );
    assert!(
        stdout.contains("lib.rs"),
        "stdout should mention subject: {stdout}"
    );
    assert!(
        stdout.contains("looks good") || stdout.contains("praise"),
        "annotation should still be displayed: {stdout}"
    );
    assert!(
        stdout.contains("https://example.com/custom/v1"),
        "unknown type string should be visible in human output: {stdout}"
    );

    // JSON format must round-trip the Unknown record verbatim.
    let (json_stdout, _, json_code) =
        run_qualifier(dir.path(), &["show", "lib.rs", "--format", "json"]);
    assert_eq!(json_code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&json_stdout).unwrap();
    let records = parsed["records"].as_array().expect("records array");
    assert_eq!(
        records.len(),
        2,
        "both records should be present: {json_stdout}"
    );

    let unknown = records
        .iter()
        .find(|r| r["type"].as_str() == Some("https://example.com/custom/v1"))
        .expect("unknown record should round-trip through JSON");
    assert_eq!(unknown["body"]["foo"], "bar");
    assert_eq!(unknown["subject"], "lib.rs");
}

#[test]
fn test_show_omits_dependency_records_in_human_output() {
    // `qualifier show <subject>` is for surfacing quality signals
    // (annotations, epochs). Dependency records are graph metadata and
    // should be silently skipped from human output (their prior behavior).
    // They MUST still round-trip through `--format json` for callers that
    // need the full record set.
    let dir = tempfile::tempdir().unwrap();

    // Hand-write a .qual file containing a dependency record. The id
    // doesn't have to be canonical for this test — `qualifier show`
    // doesn't recompute IDs, it just renders.
    let dep_record = serde_json::json!({
        "metabox": "1",
        "type": "dependency",
        "subject": "app.rs",
        "issuer": "https://ci.example.com",
        "created_at": "2026-04-01T00:00:00Z",
        "id": "d".repeat(64),
        "body": {"depends_on": ["lib.rs"]}
    });
    let qual_path = dir.path().join("app.rs.qual");
    std::fs::write(
        &qual_path,
        format!("{}\n", serde_json::to_string(&dep_record).unwrap()),
    )
    .unwrap();

    // Append a real annotation via the CLI so the subject has a quality
    // signal to render alongside the dependency.
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "praise",
            "app.rs",
            "ships clean",
            "--issuer",
            "mailto:test@test.com",
            "--file",
            "app.rs.qual",
        ],
    );
    assert_eq!(code, 0, "record helper should succeed");

    // Human output: annotation summary must appear, but the literal
    // string "dependency" (the type label that would be emitted by the
    // unknown/extension fallback) must NOT.
    let (stdout, stderr, code) = run_qualifier(dir.path(), &["show", "app.rs"]);
    assert_eq!(
        code, 0,
        "show must succeed on a subject with mixed records. stderr={stderr}"
    );
    assert!(
        stdout.contains("ships clean") || stdout.contains("praise"),
        "annotation should be rendered in human output: {stdout}"
    );
    assert!(
        !stdout.contains("dependency"),
        "dependency records must not appear in human `show` output: {stdout}"
    );
    assert!(
        !stdout.contains("[---]"),
        "dependency should be silently skipped, not rendered with the unknown-fallback marker: {stdout}"
    );

    // JSON output: dependency record MUST still round-trip — graph
    // metadata is part of the full record set.
    let (json_stdout, _, json_code) =
        run_qualifier(dir.path(), &["show", "app.rs", "--format", "json"]);
    assert_eq!(json_code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&json_stdout).unwrap();
    let records = parsed["records"].as_array().expect("records array");
    let dep = records
        .iter()
        .find(|r| r["type"].as_str() == Some("dependency"))
        .expect("dependency record should round-trip through JSON output");
    assert_eq!(dep["subject"], "app.rs");
    assert_eq!(dep["body"]["depends_on"][0], "lib.rs");
}

#[test]
fn test_ls_preserves_unknown_record_type() {
    let dir = tempfile::tempdir().unwrap();
    let _ = write_qual_with_unknown(dir.path(), "widget.rs.qual", "widget.rs");

    let (stdout, stderr, code) = run_qualifier(dir.path(), &["ls"]);
    assert_eq!(
        code, 0,
        "ls must not crash on unknown record type: {stderr}"
    );
    assert!(
        stdout.contains("widget.rs"),
        "ls output should list subject: {stdout}"
    );

    let (json_stdout, _, json_code) = run_qualifier(dir.path(), &["ls", "--format", "json"]);
    assert_eq!(json_code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&json_stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    let _widget_entry = arr
        .iter()
        .find(|e| e["subject"] == "widget.rs")
        .expect("widget.rs should appear in ls output");
}

#[test]
fn test_show_filters_by_record_type() {
    let dir = tempfile::tempdir().unwrap();
    let (custom_id, annotation_id) =
        write_qual_with_unknown(dir.path(), "thing.rs.qual", "thing.rs");

    // No --type: see both records.
    let (stdout, _, code) = run_qualifier(dir.path(), &["show", "thing.rs", "--format", "json"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["records"].as_array().unwrap().len(), 2);

    // --type annotation: only the annotation.
    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &[
            "show",
            "thing.rs",
            "--format",
            "json",
            "--type",
            "annotation",
        ],
    );
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let records = parsed["records"].as_array().unwrap();
    assert_eq!(
        records.len(),
        1,
        "filter should keep only annotations: {stdout}"
    );
    assert_eq!(records[0]["id"], annotation_id);
    assert_eq!(records[0]["type"], "annotation");

    // --type <custom URI>: only the unknown record.
    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &[
            "show",
            "thing.rs",
            "--format",
            "json",
            "--type",
            "https://example.com/custom/v1",
        ],
    );
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let records = parsed["records"].as_array().unwrap();
    assert_eq!(
        records.len(),
        1,
        "filter should keep only the unknown type: {stdout}"
    );
    assert_eq!(records[0]["id"], custom_id);
    assert_eq!(records[0]["type"], "https://example.com/custom/v1");

    // --type with no matches: empty list, still exits 0.
    let (stdout, _, code) = run_qualifier(
        dir.path(),
        &[
            "show",
            "thing.rs",
            "--format",
            "json",
            "--type",
            "no-such-type",
        ],
    );
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["records"].as_array().unwrap().len(), 0);
}

#[test]
fn test_compact_preserves_unknown_record_type() {
    let dir = tempfile::tempdir().unwrap();
    let (custom_id, first_annotation_id) =
        write_qual_with_unknown(dir.path(), "keep.rs.qual", "keep.rs");

    // Add a second annotation that supersedes the first so prune has work
    // to do (otherwise compact short-circuits with "nothing to compact").
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "praise",
            "keep.rs",
            "even better",
            "--issuer",
            "mailto:test@test.com",
            "--supersedes",
            &first_annotation_id,
            "--file",
            "keep.rs.qual",
        ],
    );
    assert_eq!(code, 0, "supersedes record should succeed");

    // prune: should remove the superseded record but keep the Unknown.
    let (_, _, code) = run_qualifier(dir.path(), &["compact", "keep.rs"]);
    assert_eq!(code, 0);

    let after_prune = std::fs::read_to_string(dir.path().join("keep.rs.qual")).unwrap();
    assert!(
        after_prune.contains(&custom_id),
        "prune compaction must preserve unknown records (spec §3.3.1):\n{after_prune}"
    );
    assert!(
        after_prune.contains("https://example.com/custom/v1"),
        "unknown type string should survive prune"
    );
    // The superseded annotation should be gone as its own record (its ID may
    // still appear as the `supersedes` pointer of the survivor — that's fine).
    let pruned_id_pattern = format!("\"id\":\"{first_annotation_id}\"");
    assert!(
        !after_prune.contains(&pruned_id_pattern),
        "superseded annotation should have been pruned:\n{after_prune}"
    );

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "praise",
            "keep.rs",
            "minor extra",
            "--issuer",
            "mailto:test@test.com",
            "--file",
            "keep.rs.qual",
        ],
    );
    assert_eq!(code, 0);

    let (_, _, code) = run_qualifier(dir.path(), &["compact", "keep.rs", "--snapshot"]);
    assert_eq!(code, 0);

    let after_snapshot = std::fs::read_to_string(dir.path().join("keep.rs.qual")).unwrap();
    assert!(
        after_snapshot.contains(&custom_id),
        "snapshot compaction must preserve unknown records (spec §3.3.1):\n{after_snapshot}"
    );
    assert!(
        after_snapshot.contains("https://example.com/custom/v1"),
        "unknown type string should survive snapshot"
    );
    assert!(
        after_snapshot.contains("\"type\":\"epoch\""),
        "snapshot should produce an epoch record:\n{after_snapshot}"
    );
}

// --- qualifier agents ---

#[test]
fn test_agents_bare_invocation_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let (stdout, stderr, code) = run_qualifier(dir.path(), &["agents"]);
    assert_eq!(code, 0, "agents should succeed: stderr={stderr}");
    assert!(!stdout.is_empty(), "agents should print something");
}

#[test]
fn test_agents_unknown_topic_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let (_stdout, stderr, code) = run_qualifier(dir.path(), &["agents", "bogus-topic"]);
    assert_eq!(code, 2, "unknown topic should exit 2: stderr={stderr}");
    assert!(
        stderr.contains("no such topic"),
        "stderr should explain: {stderr}"
    );
    assert!(
        stderr.contains("bogus-topic"),
        "stderr should name the bad topic: {stderr}"
    );
}

#[test]
fn test_agents_all_registered_topics_render() {
    // Each topic in the registry must produce non-empty stdout with exit 0.
    // If you add a topic, add it here too.
    let topics = [
        "concepts",
        "workflows",
        "pitfalls",
        "record",
        "reply",
        "resolve",
        "emit",
        "show",
        "ls",
        "praise",
        "review",
        "compact",
    ];
    let dir = tempfile::tempdir().unwrap();
    for topic in topics {
        let (stdout, stderr, code) = run_qualifier(dir.path(), &["agents", topic]);
        assert_eq!(code, 0, "agents {topic} should succeed: stderr={stderr}");
        assert!(!stdout.is_empty(), "agents {topic} should print body");
    }
}

#[test]
fn test_agents_overview_renders_topics_index() {
    let dir = tempfile::tempdir().unwrap();
    let (stdout, _stderr, code) = run_qualifier(dir.path(), &["agents"]);
    assert_eq!(code, 0);
    // Every registered topic name should appear in the rendered overview.
    for topic in [
        "concepts",
        "workflows",
        "pitfalls",
        "record",
        "reply",
        "resolve",
        "emit",
        "show",
        "ls",
        "praise",
        "review",
        "compact",
    ] {
        assert!(
            stdout.contains(topic),
            "overview should mention topic '{topic}': {stdout}"
        );
    }
    // The literal sentinel must not leak through.
    assert!(
        !stdout.contains("{{TOPICS}}"),
        "sentinel should be substituted: {stdout}"
    );
}

#[test]
fn test_agents_orientation_summaries_match_pages() {
    // Lock in the contract that the orientation page renders the topic
    // index from frontmatter summaries (rather than hard-coded ones in
    // mod.rs). Each topic's summary must appear in bare-agents output.
    let dir = tempfile::tempdir().unwrap();
    let (stdout, _stderr, code) = run_qualifier(dir.path(), &["agents"]);
    assert_eq!(code, 0);
    for needle in [
        "Annotation model, kinds, supersession",      // concepts
        "Worked recipes for common tasks",            // workflows
        "Common mistakes agents make with qualifier", // pitfalls
        "Record a new annotation",                    // record
    ] {
        assert!(
            stdout.contains(needle),
            "orientation should include summary '{needle}': {stdout}"
        );
    }
}

// --- qualifier record --stdin: per-record output, line-numbered errors ---

/// Pipe `input` to `qualifier <args>` and return (stdout, stderr, code).
fn run_qualifier_stdin(dir: &Path, args: &[&str], input: &str) -> (String, String, i32) {
    use std::io::Write;
    let mut child = Command::new(qualifier_bin())
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn qualifier");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().expect("wait_with_output");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn test_record_stdin_emits_per_record_human_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = r#"{"kind":"concern","location":"foo.rs:1","message":"first issue"}
{"kind":"suggestion","location":"foo.rs:2","message":"second"}
"#;
    let (stdout, stderr, code) = run_qualifier_stdin(
        dir.path(),
        &["record", "--stdin", "--issuer", "mailto:probe@example.com"],
        input,
    );
    assert_eq!(code, 0, "stdin should succeed: stderr={stderr}");

    // One stdout line per record, with id-prefix.
    let stdout_lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        stdout_lines.len(),
        2,
        "expected one stdout line per record, got: {stdout}"
    );
    assert!(
        stdout_lines[0].contains("concern") && stdout_lines[0].contains("first issue"),
        "first line: {}",
        stdout_lines[0]
    );
    assert!(
        stdout_lines[1].contains("suggestion") && stdout_lines[1].contains("second"),
        "second line: {}",
        stdout_lines[1]
    );

    // Trailing summary count goes to stderr (not stdout) so JSON pipes stay clean.
    assert!(
        stderr.contains("Recorded 2 of 2"),
        "summary should be on stderr: {stderr}"
    );
    assert!(
        !stdout.contains("Recorded 2"),
        "summary should NOT be on stdout: {stdout}"
    );
}

#[test]
fn test_record_stdin_json_format_emits_records() {
    let dir = tempfile::tempdir().unwrap();
    let input = r#"{"kind":"pass","location":"x.rs","message":"ok","issuer":"mailto:a@b.com"}"#
        .to_string()
        + "\n";
    let (stdout, _stderr, code) = run_qualifier_stdin(
        dir.path(),
        &["record", "--stdin", "--format", "json"],
        &input,
    );
    assert_eq!(code, 0);

    // Each stdout line should be a valid JSON record.
    for line in stdout.lines() {
        let v: serde_json::Value =
            serde_json::from_str(line).expect("each stdout line should be valid JSON");
        assert_eq!(v["type"], "annotation");
        assert_eq!(v["body"]["kind"], "pass");
        assert!(v["id"].as_str().unwrap().len() == 64);
    }
}

#[test]
fn test_record_stdin_error_includes_line_number() {
    let dir = tempfile::tempdir().unwrap();
    // Line 2 is malformed (missing 'message').
    let input = r#"{"kind":"pass","location":"a.rs","message":"first"}
{"kind":"concern","location":"b.rs"}
"#;
    let (_, stderr, code) = run_qualifier_stdin(
        dir.path(),
        &["record", "--stdin", "--issuer", "mailto:a@b.com"],
        input,
    );
    assert_ne!(code, 0, "should fail on the bad line");
    assert!(
        stderr.contains("stdin line 2"),
        "error should name the offending line number: {stderr}"
    );
}

// --- qualifier diff ---

/// Initialize a git repo in `dir` with name+email config.
fn git_init(dir: &Path) {
    let run = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?} failed");
    };
    run(&["init", "-q", "--initial-branch=main"]);
    run(&["config", "user.email", "probe@example.com"]);
    run(&["config", "user.name", "probe"]);
    run(&["config", "commit.gpgsign", "false"]);
}

fn git_commit_all(dir: &Path, msg: &str) {
    let run = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?} failed");
    };
    run(&["add", "-A"]);
    run(&["commit", "-q", "-m", msg]);
}

#[test]
fn test_diff_added_resolved_drifted() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());

    // Baseline on main: an annotated file with a span that has a content_hash.
    std::fs::write(dir.path().join("main.rs"), "fn alpha() {}\nfn beta() {}\n").unwrap();
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "main.rs:2",
            "look at beta",
            "--issuer",
            "mailto:probe@example.com",
        ],
    );
    assert_eq!(code, 0);
    git_commit_all(dir.path(), "baseline");

    // Branch off, add a new concern, resolve the old one, and mutate beta.
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "main.rs:1",
            "look at alpha too",
            "--issuer",
            "mailto:probe@example.com",
        ],
    );
    assert_eq!(code, 0);

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "resolve",
            "main.rs:2",
            "fixed",
            "--issuer",
            "mailto:probe@example.com",
        ],
    );
    assert_eq!(code, 0);

    // Mutate beta to drift the original record's span content. Note that
    // record-resolve happened first; the resolved record won't show drift,
    // but a *fresh* concern at a span that pinned a hash on the post-mutation
    // file content is also recorded so we can check the drift category by
    // mutating after that pin.
    std::fs::write(
        dir.path().join("main.rs"),
        "fn alpha() {}\nfn beta() { /* changed */ }\n",
    )
    .unwrap();

    let (stdout, stderr, code) = run_qualifier(dir.path(), &["diff", "main"]);
    assert_eq!(code, 0, "diff should succeed: stderr={stderr}");

    assert!(
        stdout.contains("Added on this branch (1)"),
        "should report one added record: {stdout}"
    );
    assert!(
        stdout.contains("look at alpha too"),
        "added record summary should appear: {stdout}"
    );
    assert!(
        stdout.contains("Resolved on this branch (1)"),
        "should report one resolved record: {stdout}"
    );
    assert!(
        stdout.contains("look at beta"),
        "resolved record's original summary should appear: {stdout}"
    );
    // The resolve record should NOT show up under Added — it's the closer.
    assert!(
        !stdout.contains("Added on this branch (2)"),
        "resolve-kind record must not double-count under Added: {stdout}"
    );
}

#[test]
fn test_diff_no_changes() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let (_, _, code) = run_qualifier(
        dir.path(),
        &["record", "pass", "a.rs", "ok", "--issuer", "mailto:a@b.com"],
    );
    assert_eq!(code, 0);
    git_commit_all(dir.path(), "baseline");

    let (stdout, _, code) = run_qualifier(dir.path(), &["diff", "main"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("no annotation changes"),
        "no-op diff should say so: {stdout}"
    );
}

#[test]
fn test_diff_bad_ref() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    git_commit_all(dir.path(), "init");

    let (_, stderr, code) = run_qualifier(dir.path(), &["diff", "no-such-ref"]);
    assert_ne!(code, 0);
    assert!(
        stderr.contains("not found"),
        "should report unknown ref: {stderr}"
    );
}

#[test]
fn test_diff_requires_git() {
    let dir = tempfile::tempdir().unwrap();
    // No git init.
    let (_, stderr, code) = run_qualifier(dir.path(), &["diff", "main"]);
    assert_ne!(code, 0);
    assert!(
        stderr.contains("git") || stderr.contains("VCS"),
        "should report missing git repo: {stderr}"
    );
}

#[test]
fn test_diff_json_format() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    git_commit_all(dir.path(), "baseline");

    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();

    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "a.rs",
            "smell",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    assert_eq!(code, 0);

    let (stdout, _, code) = run_qualifier(dir.path(), &["diff", "main", "--format", "json"]);
    assert_eq!(code, 0);

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("diff --format json");
    assert_eq!(v["ref"], "main");
    assert!(v["added"].is_array());
    assert_eq!(v["added"].as_array().unwrap().len(), 1);
    assert!(v["resolved"].is_array());
    assert!(v["drifted"].is_array());
}

// --- record --stdin: --continue-on-error / --dry-run / JSON errors ---

#[test]
fn test_record_stdin_continue_on_error_keeps_valid_lines() {
    let dir = tempfile::tempdir().unwrap();
    let input = r#"{"kind":"pass","location":"a.rs","message":"first"}
{"kind":"oops","location":"b.rs"}
{"kind":"concern","location":"c.rs","message":"third"}
"#;
    let (stdout, stderr, code) = run_qualifier_stdin(
        dir.path(),
        &[
            "record",
            "--stdin",
            "--continue-on-error",
            "--issuer",
            "mailto:a@b.com",
        ],
        input,
    );
    assert_ne!(code, 0, "should exit non-zero when any line fails");

    // The two valid lines should each have a stdout line.
    let stdout_lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        stdout_lines.len(),
        2,
        "two valid lines should each emit one stdout line: {stdout}"
    );
    assert!(stdout_lines[0].contains("first"));
    assert!(stdout_lines[1].contains("third"));

    // The invalid line should be reported on stderr with its line number AND
    // its offending input echoed back to the user.
    assert!(
        stderr.contains("stdin line 2"),
        "error should name the line: {stderr}"
    );
    assert!(
        stderr.contains("missing 'message'"),
        "error should describe the failure: {stderr}"
    );
    assert!(
        stderr.contains("\"kind\":\"oops\""),
        "error should echo the offending input: {stderr}"
    );

    // The valid lines must have actually been written.
    let qual = std::fs::read_to_string(dir.path().join(".qual")).unwrap();
    assert!(qual.contains("first"));
    assert!(qual.contains("third"));
    assert!(!qual.contains("oops"));
}

#[test]
fn test_record_stdin_dry_run_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let input = r#"{"kind":"pass","location":"a.rs","message":"hi"}
"#;
    let (stdout, stderr, code) = run_qualifier_stdin(
        dir.path(),
        &[
            "record",
            "--stdin",
            "--dry-run",
            "--issuer",
            "mailto:a@b.com",
        ],
        input,
    );
    assert_eq!(code, 0);
    assert!(
        stdout.contains("would-record"),
        "dry-run output should use 'would-record' verb: {stdout}"
    );
    assert!(
        stderr.contains("dry run"),
        "summary should mention dry run: {stderr}"
    );
    // No .qual file should exist anywhere under the tempdir.
    assert!(
        !dir.path().join(".qual").exists(),
        "dry-run must not write the .qual file"
    );
}

#[test]
fn test_record_stdin_dry_run_still_validates() {
    let dir = tempfile::tempdir().unwrap();
    // Empty summary: invalid annotation. Dry-run should still surface this.
    let input = r#"{"kind":"pass","location":"a.rs","message":""}
"#;
    let (_, stderr, code) = run_qualifier_stdin(
        dir.path(),
        &[
            "record",
            "--stdin",
            "--dry-run",
            "--issuer",
            "mailto:a@b.com",
        ],
        input,
    );
    assert_ne!(code, 0, "dry-run must still report validation errors");
    assert!(
        stderr.contains("summary"),
        "error should reference the empty summary: {stderr}"
    );
    assert!(
        !dir.path().join(".qual").exists(),
        "no file should be written even on validation success — let alone failure"
    );
}

#[test]
fn test_record_stdin_json_errors_are_structured() {
    let dir = tempfile::tempdir().unwrap();
    let input = r#"{"kind":"pass","location":"a.rs","message":"ok"}
{"kind":"oops","location":"b.rs"}
"#;
    let (stdout, stderr, code) = run_qualifier_stdin(
        dir.path(),
        &[
            "record",
            "--stdin",
            "--continue-on-error",
            "--format",
            "json",
            "--issuer",
            "mailto:a@b.com",
        ],
        input,
    );
    assert_ne!(code, 0);

    // stdout: each line a valid JSONL record.
    for line in stdout.lines() {
        let v: serde_json::Value =
            serde_json::from_str(line).expect("stdout line should be valid JSON");
        assert_eq!(v["type"], "annotation");
    }

    // stderr: every line should also be a valid JSON object — error first,
    // then a `summary` trailer. NO free-form text.
    let stderr_lines: Vec<&str> = stderr.lines().collect();
    assert!(stderr_lines.len() >= 2, "expected error+summary: {stderr}");

    let err_obj: serde_json::Value =
        serde_json::from_str(stderr_lines[0]).expect("first stderr line should be JSON");
    assert_eq!(err_obj["line"], 2);
    assert!(err_obj["error"].as_str().unwrap().contains("message"));
    assert!(err_obj["input"].as_str().unwrap().contains("oops"));

    // Last stderr line is the summary trailer.
    let summary: serde_json::Value =
        serde_json::from_str(stderr_lines.last().unwrap()).expect("trailer should be JSON");
    assert_eq!(summary["summary"]["recorded"], 1);
    assert_eq!(summary["summary"]["failed"], 1);
    assert_eq!(summary["summary"]["total"], 2);
}

#[test]
fn test_record_stdin_default_aborts_on_first_error_with_input_echoed() {
    let dir = tempfile::tempdir().unwrap();
    let input = r#"{"kind":"pass","location":"a.rs","message":"ok"}
{"kind":"oops","location":"b.rs"}
{"kind":"concern","location":"c.rs","message":"third"}
"#;
    let (_, stderr, code) = run_qualifier_stdin(
        dir.path(),
        &["record", "--stdin", "--issuer", "mailto:a@b.com"],
        input,
    );
    assert_ne!(code, 0);
    // Without --continue-on-error, line 2 aborts the batch — the error
    // message must include the offending input so the user can see what
    // they sent without re-piping.
    assert!(
        stderr.contains("stdin line 2"),
        "error should name the failing line: {stderr}"
    );
    assert!(
        stderr.contains("\"kind\":\"oops\""),
        "error should echo the offending input on the abort path too: {stderr}"
    );

    // Line 1 was written (sequential semantics); line 3 was not.
    let qual = std::fs::read_to_string(dir.path().join(".qual")).unwrap();
    assert!(qual.contains("ok"));
    assert!(!qual.contains("third"));
}

// --- diff: merge-base default ---

/// Set up a repo where `main` advances *after* a feature branch has been
/// created. This exercises the merge-base default: a record that landed on
/// main after the branch must NOT show up as "Added" when diffing the
/// feature branch against main.
fn diff_merge_base_setup(dir: &Path) {
    git_init(dir);

    // Initial commit on main with a baseline file. Use --allow-empty so the
    // merge-base is well-defined even if there's nothing else to commit yet.
    Command::new("git")
        .args(["commit", "-q", "--allow-empty", "-m", "init"])
        .current_dir(dir)
        .status()
        .unwrap();

    // Branch off here.
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir)
        .status()
        .unwrap();

    // On the feature branch: record one new concern.
    let (_, _, code) = run_qualifier(
        dir,
        &[
            "record",
            "concern",
            "feat.rs",
            "feature finding",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    assert_eq!(code, 0);
    git_commit_all(dir, "feat record");

    // Now go back to main and add a record that didn't exist when feat
    // forked. The feature branch must not see this as "Added".
    Command::new("git")
        .args(["checkout", "-q", "main"])
        .current_dir(dir)
        .status()
        .unwrap();
    let (_, _, code) = run_qualifier(
        dir,
        &[
            "record",
            "concern",
            "main.rs",
            "post-fork main finding",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    assert_eq!(code, 0);
    git_commit_all(dir, "main record after fork");

    // Return to feat for the diff.
    Command::new("git")
        .args(["checkout", "-q", "feat"])
        .current_dir(dir)
        .status()
        .unwrap();
}

#[test]
fn test_diff_default_uses_merge_base() {
    let dir = tempfile::tempdir().unwrap();
    diff_merge_base_setup(dir.path());

    let (stdout, _, code) = run_qualifier(dir.path(), &["diff", "main"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("merge-base"),
        "default header should mention merge-base: {stdout}"
    );
    assert!(
        stdout.contains("feature finding"),
        "feature record should be Added: {stdout}"
    );
    assert!(
        !stdout.contains("post-fork main finding"),
        "main-only record after fork must not show up under merge-base default: {stdout}"
    );
}

#[test]
fn test_diff_from_tip_includes_post_fork_main_records() {
    let dir = tempfile::tempdir().unwrap();
    diff_merge_base_setup(dir.path());

    let (stdout, _, code) = run_qualifier(dir.path(), &["diff", "main", "--from-tip"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("(tip)"),
        "header should reflect --from-tip: {stdout}"
    );
    // Under --from-tip, the post-fork main record IS in <ref> but missing
    // from HEAD — it appears as Resolved/removed.
    assert!(
        stdout.contains("post-fork main finding"),
        "from-tip diff should surface main-only records: {stdout}"
    );
}

// --- diff: --fail-on / --fail-on-drift ---

#[test]
fn test_diff_fail_on_kind_exits_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    Command::new("git")
        .args(["commit", "-q", "--allow-empty", "-m", "init"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "blocker",
            "x.rs",
            "ship-stop",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    assert_eq!(code, 0);

    let (stdout, stderr, code) =
        run_qualifier(dir.path(), &["diff", "main", "--fail-on", "blocker"]);
    assert_ne!(code, 0, "should fail when a blocker is added");
    assert!(
        stdout.contains("blocker"),
        "diff should still print the offending record before failing: {stdout}"
    );
    assert!(
        stderr.contains("--fail-on"),
        "error message should reference the flag that triggered it: {stderr}"
    );

    // A non-matching --fail-on should pass.
    let (_, _, code) = run_qualifier(dir.path(), &["diff", "main", "--fail-on", "fail"]);
    assert_eq!(code, 0, "no `fail` records added; should pass");
}

#[test]
fn test_diff_fail_on_multiple_kinds() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    Command::new("git")
        .args(["commit", "-q", "--allow-empty", "-m", "init"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "fail",
            "x.rs",
            "broke",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    assert_eq!(code, 0);

    let (_, _, code) = run_qualifier(dir.path(), &["diff", "main", "--fail-on", "blocker,fail"]);
    assert_ne!(code, 0, "comma-separated list should match `fail` records");
}

#[test]
fn test_diff_fail_on_drift() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    std::fs::write(dir.path().join("m.rs"), "a\nb\nc\n").unwrap();
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "m.rs:2",
            "look",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    assert_eq!(code, 0);
    git_commit_all(dir.path(), "baseline");
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    // Mutate the spanned line to drift the content_hash.
    std::fs::write(dir.path().join("m.rs"), "a\nB\nc\n").unwrap();

    let (_, stderr, code) = run_qualifier(dir.path(), &["diff", "main", "--fail-on-drift"]);
    assert_ne!(code, 0, "should exit non-zero when drift is present");
    assert!(
        stderr.contains("drifted") || stderr.contains("drift"),
        "error should reference drift: {stderr}"
    );
}

// --- diff: filter flags ---

#[test]
fn test_diff_kind_filter() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    Command::new("git")
        .args(["commit", "-q", "--allow-empty", "-m", "init"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    let (_, _, _) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "a.rs",
            "concern1",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    let (_, _, _) = run_qualifier(
        dir.path(),
        &[
            "record",
            "blocker",
            "a.rs",
            "blocker1",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    let (_, _, _) = run_qualifier(
        dir.path(),
        &[
            "record",
            "suggestion",
            "a.rs",
            "suggestion1",
            "--issuer",
            "mailto:a@b.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["diff", "main", "--kind", "concern"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("concern1"),
        "concern should appear: {stdout}"
    );
    assert!(
        !stdout.contains("blocker1"),
        "blocker should be filtered out: {stdout}"
    );
    assert!(
        !stdout.contains("suggestion1"),
        "suggestion should be filtered out: {stdout}"
    );
    assert!(
        stdout.contains("Added on this branch (1)"),
        "should show only one match: {stdout}"
    );
}

#[test]
fn test_diff_issuer_type_filter() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    Command::new("git")
        .args(["commit", "-q", "--allow-empty", "-m", "init"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    let (_, _, _) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "a.rs",
            "from-ai",
            "--issuer",
            "mailto:bot@example.com",
            "--issuer-type",
            "ai",
        ],
    );
    let (_, _, _) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "a.rs",
            "from-human",
            "--issuer",
            "mailto:dev@example.com",
            "--issuer-type",
            "human",
        ],
    );

    let (stdout, _, _) = run_qualifier(dir.path(), &["diff", "main", "--issuer-type", "ai"]);
    assert!(
        stdout.contains("from-ai"),
        "ai record should appear: {stdout}"
    );
    assert!(
        !stdout.contains("from-human"),
        "human record should be filtered: {stdout}"
    );
}

#[test]
fn test_diff_subjects_only() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    Command::new("git")
        .args(["commit", "-q", "--allow-empty", "-m", "init"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    for (subject, summary) in [("a.rs", "1"), ("b.rs", "2"), ("a.rs", "3")] {
        let (_, _, code) = run_qualifier(
            dir.path(),
            &[
                "record",
                "concern",
                subject,
                summary,
                "--issuer",
                "mailto:a@b.com",
            ],
        );
        assert_eq!(code, 0);
    }

    let (stdout, _, code) = run_qualifier(dir.path(), &["diff", "main", "--subjects-only"]);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines,
        vec!["a.rs", "b.rs"],
        "should be deduped + sorted: {stdout}"
    );
}

// --- diff: human output polish ---

#[test]
fn test_diff_resolved_inlines_closer_summary() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    std::fs::write(dir.path().join("x.rs"), "fn x() {}\n").unwrap();
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "x.rs:1",
            "needs work",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    assert_eq!(code, 0);
    git_commit_all(dir.path(), "baseline");
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "resolve",
            "x.rs:1",
            "fixed in PR #42",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    assert_eq!(code, 0);

    let (stdout, _, code) = run_qualifier(dir.path(), &["diff", "main"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("resolved by"),
        "should mention closer: {stdout}"
    );
    assert!(
        stdout.contains("fixed in PR #42"),
        "closer summary should be inlined: {stdout}"
    );
}

#[test]
fn test_diff_drift_includes_span_snippet() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    std::fs::write(
        dir.path().join("m.rs"),
        "fn alpha() {}\nfn beta() {}\nfn gamma() {}\n",
    )
    .unwrap();
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "m.rs:2",
            "watch beta",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    assert_eq!(code, 0);
    git_commit_all(dir.path(), "baseline");
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    std::fs::write(
        dir.path().join("m.rs"),
        "fn alpha() {}\nfn beta() { /* changed */ }\nfn gamma() {}\n",
    )
    .unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["diff", "main"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("watch beta"),
        "should print original summary: {stdout}"
    );
    assert!(
        stdout.contains("fn beta() { /* changed */ }"),
        "should print current span content as a snippet: {stdout}"
    );
    // Compiler-style marker for the drifted line.
    assert!(
        stdout.contains("> 2"),
        "snippet should mark line 2: {stdout}"
    );
}

// --- diff: 80-col friendliness ---

/// Run qualifier with `COLUMNS` set to override stdout width detection.
fn run_qualifier_with_columns(dir: &Path, args: &[&str], columns: usize) -> (String, String, i32) {
    let output = Command::new(qualifier_bin())
        .args(args)
        .current_dir(dir)
        .env("COLUMNS", columns.to_string())
        .output()
        .expect("failed to run qualifier binary");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn test_diff_human_output_fits_80_cols() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());

    // Create a deeply-nested path and a long summary — the worst case for
    // line-width budgeting.
    let nested = dir.path().join("src/cli/commands/agents/pages");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("record.md"), "line one\nline two\nline three\n").unwrap();
    let long_summary = "AnnotationBody field declaration order silently determines MCF canonical IDs — \
         make the invariant load-bearing in code or in a doc-test";
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "suggestion",
            "src/cli/commands/agents/pages/record.md:2",
            long_summary,
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    assert_eq!(code, 0);
    git_commit_all(dir.path(), "baseline");
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();

    // A second record on the feature branch with a long summary too — this
    // ends up under Added.
    let (_, _, code) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "src/cli/commands/agents/pages/record.md:1",
            "this is a deliberately long summary intended to overflow the single-line budget on \
             any narrow terminal so the wrapping path is exercised",
            "--issuer",
            "mailto:a@b.com",
        ],
    );
    assert_eq!(code, 0);

    // Mutate the spanned line to cause drift on the baseline record; this
    // exercises the Drifted bucket with a snippet, the longest path so far.
    std::fs::write(
        nested.join("record.md"),
        "line one\nDIFFERENT line two\nline three\n",
    )
    .unwrap();

    let (stdout, _, code) = run_qualifier_with_columns(dir.path(), &["diff", "main"], 80);
    assert_eq!(code, 0);

    for line in stdout.lines() {
        let chars = line.chars().count();
        assert!(
            chars <= 80,
            "line exceeds 80-col budget ({chars} chars): {line:?}"
        );
    }

    // Sanity: the summary still appears (truncated or wrapped, but present
    // enough that a reviewer can identify the record).
    assert!(stdout.contains("AnnotationBody field declaration order"));
    assert!(stdout.contains("deliberately long summary"));
}

// --- diff: JSON shape stability ---

#[test]
fn test_diff_json_includes_base_and_from_tip() {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    Command::new("git")
        .args(["commit", "-q", "--allow-empty", "-m", "init"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    Command::new("git")
        .args(["checkout", "-q", "-b", "feat"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    let (_, _, _) = run_qualifier(
        dir.path(),
        &[
            "record",
            "concern",
            "x.rs",
            "y",
            "--issuer",
            "mailto:a@b.com",
        ],
    );

    let (stdout, _, code) = run_qualifier(dir.path(), &["diff", "main", "--format", "json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ref"], "main");
    assert_eq!(v["from_tip"], false);
    let base = v["base"].as_str().expect("base should be a sha string");
    assert_eq!(base.len(), 40, "base should be a full sha: {base}");
    assert!(v["added"].is_array());
}

#[test]
fn test_top_level_help_shows_agents_group() {
    let dir = tempfile::tempdir().unwrap();
    let (stdout, _stderr, code) = run_qualifier(dir.path(), &["--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("For AI agents:"),
        "help should show the agents group header: {stdout}"
    );
    // An imperative directive sits above the subcommand list so an agent
    // scanning the verb table doesn't mistake it for flavor text.
    assert!(
        stdout.contains("If you are an AI coding agent, run `qualifier agents` first"),
        "help should print the agent directive above the subcommand list: {stdout}"
    );
    assert!(
        stdout.contains("agents") && stdout.contains("Read this before recording annotations"),
        "help should mention the agents subcommand with imperative description: {stdout}"
    );
}

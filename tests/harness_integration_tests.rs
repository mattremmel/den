//! End-to-end integration tests demonstrating the test harness.
//!
//! These tests exercise the CLI through the harness API, showing how to
//! set up test environments, add notes, build indexes, and make assertions.

mod common;

use common::harness::{TestEnv, TestNote};
use predicates::prelude::*;

// ===========================================
// Phase 6: End-to-End Integration Tests
// ===========================================

#[test]
fn test_ls_empty_returns_no_notes() {
    let env = TestEnv::new();

    // Build an empty index
    env.build_index().expect("Should build index");

    // ls should succeed but show no notes
    env.cmd()
        .ls()
        .assert()
        .success()
        .stdout(predicate::str::is_empty().or(predicate::str::contains("No notes found")));
}

#[test]
fn test_ls_lists_added_notes() {
    let env = TestEnv::new();

    let note = TestNote::new("Architecture Decisions")
        .topic("software/architecture")
        .tag("adr");
    env.add_note(&note);
    env.build_index().expect("Should build index");

    env.cmd()
        .ls()
        .assert()
        .success()
        .stdout(predicate::str::contains("Architecture Decisions"));
}

#[test]
fn test_ls_json_format() {
    let env = TestEnv::new();

    let note = TestNote::new("JSON Test Note")
        .id("01HQ3K5M7NXJK4QZPW8V2R6T9Y")
        .tag("json-test");
    env.add_note(&note);
    env.build_index().expect("Should build index");

    let output: serde_json::Value = env.cmd().ls().format_json().output_json();

    // Verify JSON structure
    assert!(output.is_object(), "Output should be a JSON object");
    let data = output.get("data").expect("Should have 'data' field");
    let notes = data.as_array().expect("data should be an array");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["title"], "JSON Test Note");
}

#[test]
fn test_index_builds_from_notes() {
    let env = TestEnv::new();

    // Add notes without building index
    let note1 = TestNote::new("Note One");
    let note2 = TestNote::new("Note Two");
    env.add_note(&note1);
    env.add_note(&note2);

    // Run the index command
    env.cmd().index().assert().success();

    // Now ls should show both notes
    env.cmd()
        .ls()
        .assert()
        .success()
        .stdout(predicate::str::contains("Note One"))
        .stdout(predicate::str::contains("Note Two"));
}

#[test]
fn test_show_displays_note() {
    let env = TestEnv::new();

    let note = TestNote::new("Display Test")
        .id("01HQ3K5M7NXJK4QZPW8V2R6T9Y")
        .description("A note for display testing")
        .topic("testing")
        .body("# Display Test\n\nThis is the body content.");
    env.add_note(&note);
    env.build_index().expect("Should build index");

    // Show by ID prefix
    env.cmd()
        .show("01HQ3K5M7N")
        .assert()
        .success()
        .stdout(predicate::str::contains("Display Test"))
        .stdout(predicate::str::contains("display testing"));
}

#[test]
fn test_tags_lists_all_tags() {
    let env = TestEnv::new();

    let note1 = TestNote::new("Tagged Note 1").tag("rust").tag("testing");
    let note2 = TestNote::new("Tagged Note 2").tag("rust").tag("cli");
    env.add_note(&note1);
    env.add_note(&note2);
    env.build_index().expect("Should build index");

    env.cmd()
        .tags()
        .assert()
        .success()
        .stdout(predicate::str::contains("rust"))
        .stdout(predicate::str::contains("testing"))
        .stdout(predicate::str::contains("cli"));
}

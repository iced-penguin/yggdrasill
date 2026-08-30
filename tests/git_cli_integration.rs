//! Integration tests for GitCliRepository with real Git operations.
//!
//! These tests require Git to be installed. They create temporary repositories
//! for testing purposes and clean up after themselves.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

// We would need to expose these from the main crate for integration testing
// For now, these tests demonstrate the intended structure

fn create_temp_git_repo() -> Result<(TempDir, PathBuf), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path().to_path_buf();

    // Initialize a git repository
    Command::new("git")
        .arg("init")
        .current_dir(&repo_path)
        .output()?;

    // Configure git for tests
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()?;

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&repo_path)
        .output()?;

    // Create an initial commit
    fs::write(repo_path.join("README.md"), "# Test Repository")?;
    Command::new("git")
        .arg("add")
        .arg("README.md")
        .current_dir(&repo_path)
        .output()?;

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&repo_path)
        .output()?;

    Ok((temp_dir, repo_path))
}

#[test]
#[ignore] // Run with `cargo test -- --ignored` to include these
fn lists_branches_from_real_repository() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, repo_path) = create_temp_git_repo()?;

    // Create a branch
    Command::new("git")
        .args(["checkout", "-b", "feature"])
        .current_dir(&repo_path)
        .output()?;

    // Verify we can list branches
    let output = Command::new("git")
        .args(["branch", "--list"])
        .current_dir(&repo_path)
        .output()?;

    let branches_output = String::from_utf8(output.stdout)?;
    assert!(branches_output.contains("main") || branches_output.contains("master"));
    assert!(branches_output.contains("feature"));

    Ok(())
}

#[test]
#[ignore] // Run with `cargo test -- --ignored` to include these
fn can_create_and_checkout_branches() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, repo_path) = create_temp_git_repo()?;

    // Create a new branch
    Command::new("git")
        .args(["checkout", "-b", "feature/test"])
        .current_dir(&repo_path)
        .output()?;

    // Verify the branch exists
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()?;

    let current_branch = String::from_utf8(output.stdout)?.trim().to_string();
    assert_eq!(current_branch, "feature/test");

    // Switch back to main
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&repo_path)
        .output()?;

    // Verify we're back on main
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()?;

    let current_branch = String::from_utf8(output.stdout)?.trim().to_string();
    assert!(current_branch == "main" || current_branch == "master");

    Ok(())
}

#[test]
#[ignore] // Run with `cargo test -- --ignored` to include these
fn can_create_and_list_worktrees() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, repo_path) = create_temp_git_repo()?;

    let default_branch = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(&repo_path)
        .output()?;
    assert!(default_branch.status.success());
    let default_branch = String::from_utf8(default_branch.stdout)?.trim().to_owned();

    // Create a feature branch first
    let output = Command::new("git")
        .args(["checkout", "-b", "feature/wt"])
        .current_dir(&repo_path)
        .status()?;
    assert!(output.success());

    let output = Command::new("git")
        .args(["checkout", &default_branch])
        .current_dir(&repo_path)
        .status()?;
    assert!(output.success());

    let worktree_path = repo_path.parent().unwrap().join("repo-feature-wt");

    // Create a worktree
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            worktree_path.to_str().unwrap(),
            "feature/wt",
        ])
        .current_dir(&repo_path)
        .output()?;
    assert!(
        output.status.success(),
        "git worktree add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // List worktrees
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo_path)
        .output()?;

    let worktree_output = String::from_utf8(output.stdout)?;
    assert!(worktree_output.contains(repo_path.to_str().unwrap()));
    assert!(worktree_output.contains(worktree_path.to_str().unwrap()));

    // Clean up worktree
    let output = Command::new("git")
        .args(["worktree", "remove", worktree_path.to_str().unwrap()])
        .current_dir(&repo_path)
        .output()?;
    assert!(
        output.status.success(),
        "git worktree remove failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

#[test]
#[ignore] // Run with `cargo test -- --ignored` to include these
fn can_get_commit_information() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, repo_path) = create_temp_git_repo()?;

    // Get commit info for main/master
    let output = Command::new("git")
        .args(["log", "-1", "--format=%h%x00%s"])
        .current_dir(&repo_path)
        .output()?;

    let commit_info = String::from_utf8(output.stdout)?;
    let parts: Vec<&str> = commit_info.trim().split('\0').collect();

    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].len(), 7); // Short hash is typically 7 characters
    assert!(parts[1].contains("Initial commit"));

    Ok(())
}

#[test]
#[ignore] // Run with `cargo test -- --ignored` to include these
fn can_detect_dirty_worktree() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, repo_path) = create_temp_git_repo()?;

    // Initially clean
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&repo_path)
        .output()?;

    let status = String::from_utf8(output.stdout)?;
    assert!(status.is_empty());

    // Make a change
    fs::write(repo_path.join("test.txt"), "test content")?;

    // Now it should be dirty
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&repo_path)
        .output()?;

    let status = String::from_utf8(output.stdout)?;
    assert!(!status.is_empty());
    assert!(status.contains("test.txt"));

    Ok(())
}

#[test]
#[ignore] // Run with `cargo test -- --ignored` to include these
fn can_show_diff_stat() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, repo_path) = create_temp_git_repo()?;

    // Make a change
    fs::write(repo_path.join("changes.txt"), "line1\nline2\nline3")?;

    // Add and commit
    Command::new("git")
        .arg("add")
        .arg("changes.txt")
        .current_dir(&repo_path)
        .output()?;

    Command::new("git")
        .args(["commit", "-m", "Add changes"])
        .current_dir(&repo_path)
        .output()?;

    // Get diff stat
    let output = Command::new("git")
        .args(["diff", "--stat", "HEAD~1"])
        .current_dir(&repo_path)
        .output()?;

    let diff_output = String::from_utf8(output.stdout)?;
    assert!(diff_output.contains("changes.txt"));
    assert!(diff_output.contains("insertion"));

    Ok(())
}

#[test]
#[ignore] // Run with `cargo test -- --ignored` to include these
fn can_rename_branches() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, repo_path) = create_temp_git_repo()?;

    // Create a branch
    Command::new("git")
        .args(["checkout", "-b", "old-name"])
        .current_dir(&repo_path)
        .output()?;

    // Rename it
    Command::new("git")
        .args(["branch", "-m", "old-name", "new-name"])
        .current_dir(&repo_path)
        .output()?;

    // Verify the rename
    let output = Command::new("git")
        .args(["branch", "--list"])
        .current_dir(&repo_path)
        .output()?;

    let branches = String::from_utf8(output.stdout)?;
    assert!(!branches.contains("old-name"));
    assert!(branches.contains("new-name"));

    Ok(())
}

#[test]
#[ignore] // Run with `cargo test -- --ignored` to include these
fn can_delete_branches() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, repo_path) = create_temp_git_repo()?;

    // Create a branch
    Command::new("git")
        .args(["checkout", "-b", "to-delete"])
        .current_dir(&repo_path)
        .output()?;

    // Go back to main to delete the branch
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&repo_path)
        .output()?;

    // Delete it
    Command::new("git")
        .args(["branch", "-d", "to-delete"])
        .current_dir(&repo_path)
        .output()?;

    // Verify deletion
    let output = Command::new("git")
        .args(["branch", "--list"])
        .current_dir(&repo_path)
        .output()?;

    let branches = String::from_utf8(output.stdout)?;
    assert!(!branches.contains("to-delete"));

    Ok(())
}

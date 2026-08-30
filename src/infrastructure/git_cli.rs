use std::{
    path::Path,
    process::{Command, Stdio},
};

use crate::app::port::{GitRepositoryPort, RepositoryResult};
use crate::domain::{BranchRecord, HeadState, SyncStatus, WorkingTreeStatus, WorktreeInfo};

pub struct GitCliRepository {
    git_repo: gix::Repository,
}

impl GitCliRepository {
    pub fn open(path: &str) -> RepositoryResult<Self> {
        let git_repo = gix::discover(path)?;
        Ok(Self { git_repo })
    }
}

impl GitRepositoryPort for GitCliRepository {
    fn list_branches(&self, _fetch: bool) -> RepositoryResult<Vec<BranchRecord>> {
        let current_branch = self.git_repo.head_name()?;
        let default_branch = self.get_default_branch()?;
        let mut worktree_entries = Vec::new();
        for (branch, path) in self.get_worktree_list()? {
            let is_dirty = is_worktree_dirty(Path::new(&path))?;
            let (commit_hash, commit_subject) = get_worktree_commit(&path)?;
            worktree_entries.push((branch, path, is_dirty, commit_hash, commit_subject));
        }

        let mut records = Vec::new();
        for branch in self.git_repo.references()?.local_branches()? {
            let branch = branch?;
            let name = branch.name();
            let short_name = name.shorten().to_string();

            let worktree = worktree_entries
                .iter()
                .find(|(branch_name, _, _, _, _)| {
                    branch_name.as_deref() == Some(short_name.as_str())
                })
                .map(|(_, path, is_dirty, _, _)| (path.clone(), *is_dirty));

            let is_current = current_branch
                .as_ref()
                .is_some_and(|current| current.as_ref() == name);

            let (commit_hash, commit_subject) = get_branch_commit(&short_name)?;
            let sync_status = get_branch_sync_status(&short_name, default_branch.as_deref())?;

            records.push(BranchRecord::new_branch(
                short_name,
                is_current,
                worktree.as_ref().map(|(path, is_dirty)| WorktreeInfo {
                    path: path.clone(),
                    head: HeadState::Branch(name.shorten().to_string()),
                    status: if *is_dirty {
                        WorkingTreeStatus::Modified
                    } else {
                        WorkingTreeStatus::Clean
                    },
                    commit_hash: commit_hash.clone(),
                    commit_subject: commit_subject.clone(),
                }),
                commit_hash,
                commit_subject,
                sync_status,
            ));
        }

        // Add detached/unborn entries
        for (branch, path, is_dirty, commit_hash, commit_subject) in worktree_entries {
            if branch.is_none() {
                let is_unborn = commit_hash == "-";
                let head_state = if is_unborn {
                    HeadState::Unborn
                } else {
                    HeadState::Detached(commit_hash.clone())
                };

                let is_current = current_branch.is_none() && path == self.repository_root()?;
                let worktree_info = WorktreeInfo {
                    path,
                    head: head_state.clone(),
                    status: if is_dirty {
                        WorkingTreeStatus::Modified
                    } else {
                        WorkingTreeStatus::Clean
                    },
                    commit_hash: commit_hash.clone(),
                    commit_subject: commit_subject.clone(),
                };

                records.push(BranchRecord::new_non_branch_worktree(
                    worktree_info,
                    is_current,
                ));
            }
        }

        Ok(records)
    }

    fn checkout_branch(&self, name: &str) -> RepositoryResult<()> {
        let output = Command::new("git")
            .arg("checkout")
            .arg(name)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()?;
        if output.status.success() {
            return Ok(());
        }
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if message.is_empty() {
            format!("git checkout {name} failed").into()
        } else {
            message.into()
        })
    }

    fn add_worktree(&self, branch: &str, path: &str) -> RepositoryResult<()> {
        let output = Command::new("git")
            .args(["worktree", "add", path, branch])
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr)
                .trim()
                .to_owned()
                .into())
        }
    }

    fn remove_worktree(&self, path: &str) -> RepositoryResult<()> {
        let output = Command::new("git")
            .args(["worktree", "remove", path])
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr)
                .trim()
                .to_owned()
                .into())
        }
    }

    fn create_branch(&self, name: &str, parent: &str) -> RepositoryResult<()> {
        let output = Command::new("git")
            .args(["branch", name, parent])
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr)
                .trim()
                .to_owned()
                .into())
        }
    }

    fn rename_branch(&self, old_name: &str, new_name: &str) -> RepositoryResult<()> {
        let output = Command::new("git")
            .args(["branch", "-m", old_name, new_name])
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr)
                .trim()
                .to_owned()
                .into())
        }
    }

    fn delete_branch(&self, name: &str) -> RepositoryResult<()> {
        let output = Command::new("git").args(["branch", "-d", name]).output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr)
                .trim()
                .to_owned()
                .into())
        }
    }

    fn diff_stat(
        &self,
        worktree_path: Option<&str>,
        branch_name: &str,
    ) -> RepositoryResult<String> {
        let mut command = Command::new("git");
        if let Some(path) = worktree_path {
            command.arg("-C").arg(path).args(["diff", "--stat"]);
        } else {
            command.args(["diff", "--stat", branch_name]);
        }
        let output = command.output()?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr)
                .trim()
                .to_owned()
                .into());
        }
        let diff = String::from_utf8(output.stdout)?;
        Ok(if diff.trim().is_empty() {
            String::from("No changes")
        } else {
            diff
        })
    }

    fn repository_root(&self) -> RepositoryResult<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(if message.is_empty() {
                String::from("git rev-parse --show-toplevel failed")
            } else {
                message
            }
            .into());
        }
        let path = String::from_utf8(output.stdout)?.trim().to_owned();
        if path.is_empty() {
            return Err("git rev-parse --show-toplevel returned an empty path".into());
        }
        Ok(path)
    }

    fn default_worktree_path(&self, branch_name: &str) -> RepositoryResult<String> {
        let root_path = self.repository_root()?;
        let root = Path::new(&root_path);
        let parent = root.parent().ok_or("repository has no parent directory")?;
        let repository_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid repository name")?;
        Ok(parent
            .join(format!(
                "{repository_name}-{}",
                branch_name.replace('/', "-")
            ))
            .display()
            .to_string())
    }
}

impl GitCliRepository {
    fn get_worktree_list(&self) -> RepositoryResult<Vec<(Option<String>, String)>> {
        let output = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .output()?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr)
                .trim()
                .to_owned()
                .into());
        }
        let output = String::from_utf8(output.stdout)?;
        parse_worktree_list(&output).map_err(Into::into)
    }

    fn get_default_branch(&self) -> RepositoryResult<Option<String>> {
        let output = Command::new("git")
            .args([
                "symbolic-ref",
                "--quiet",
                "--short",
                "refs/remotes/origin/HEAD",
            ])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        let branch = String::from_utf8(output.stdout)?.trim().to_owned();
        Ok((!branch.is_empty()).then_some(branch))
    }
}

fn parse_worktree_list(output: &str) -> Result<Vec<(Option<String>, String)>, String> {
    output
        .split("\n\n")
        .filter(|record| !record.trim().is_empty())
        .map(|record| {
            let path = record
                .lines()
                .find_map(|line| line.strip_prefix("worktree "))
                .ok_or("worktree entry is missing its path")?
                .to_owned();
            let branch = record
                .lines()
                .find_map(|line| line.strip_prefix("branch refs/heads/"))
                .map(str::to_owned);
            Ok((branch, path))
        })
        .collect()
}

fn get_branch_sync_status(
    branch: &str,
    default_branch: Option<&str>,
) -> RepositoryResult<SyncStatus> {
    let upstream_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name"])
        .arg(format!("{branch}@{{upstream}}"))
        .output()?;
    if !upstream_output.status.success() {
        return Ok(SyncStatus::NoUpstream {
            unpublished: default_branch
                .map(|base| get_unpublished_commit_count(branch, base))
                .transpose()?,
        });
    }
    let upstream = String::from_utf8(upstream_output.stdout)?.trim().to_owned();
    if upstream.is_empty() {
        return Ok(SyncStatus::NoUpstream {
            unpublished: default_branch
                .map(|base| get_unpublished_commit_count(branch, base))
                .transpose()?,
        });
    }

    let count_output = Command::new("git")
        .args(["rev-list", "--left-right", "--count"])
        .arg(format!("{branch}...{upstream}"))
        .output()?;
    if !count_output.status.success() {
        return Err(format!("git rev-list failed for {branch}").into());
    }
    let counts = String::from_utf8(count_output.stdout)?;
    let (ahead, behind) = parse_ahead_behind(&counts).ok_or("invalid git rev-list output")?;

    if ahead == 0 && behind == 0 {
        Ok(SyncStatus::UpToDate { upstream })
    } else if ahead > 0 && behind == 0 {
        Ok(SyncStatus::Ahead {
            upstream,
            count: ahead,
        })
    } else if ahead == 0 && behind > 0 {
        Ok(SyncStatus::Behind {
            upstream,
            count: behind,
        })
    } else {
        Ok(SyncStatus::Diverged {
            upstream,
            ahead,
            behind,
        })
    }
}

fn get_unpublished_commit_count(branch: &str, default_branch: &str) -> RepositoryResult<usize> {
    let output = Command::new("git")
        .args(["rev-list", "--count"])
        .arg(format!("{default_branch}..{branch}"))
        .output()?;
    if !output.status.success() {
        return Err(format!("git rev-list failed for {branch}").into());
    }
    String::from_utf8(output.stdout)?
        .trim()
        .parse()
        .map_err(|_| "invalid git rev-list output".into())
}

fn parse_ahead_behind(value: &str) -> Option<(usize, usize)> {
    let mut counts = value.split_whitespace();
    let ahead = counts.next()?.parse().ok()?;
    let behind = counts.next()?.parse().ok()?;
    counts.next().is_none().then_some((ahead, behind))
}

fn get_worktree_commit(path: &str) -> RepositoryResult<(String, String)> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["log", "-1", "--format=%h%x00%s"])
        .output()?;
    if !output.status.success() {
        return Ok((String::from("-"), String::from("No commits yet")));
    }
    let output = String::from_utf8(output.stdout)?;
    let (hash, subject) = output.trim_end().split_once('\0').unwrap_or(("-", "-"));
    Ok((hash.to_owned(), subject.to_owned()))
}

fn get_branch_commit(name: &str) -> RepositoryResult<(String, String)> {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%h%x00%s", name])
        .output()?;
    if !output.status.success() {
        return Err(format!("git log failed for {name}").into());
    }
    let output = String::from_utf8(output.stdout)?;
    let (hash, subject) = output.trim_end().split_once('\0').unwrap_or(("-", "-"));
    Ok((hash.to_owned(), subject.to_owned()))
}

fn is_worktree_dirty(path: &Path) -> RepositoryResult<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err(format!("git status failed for {}", path.display()).into());
    }
    Ok(!output.stdout.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ahead_and_behind_counts() {
        assert_eq!(parse_ahead_behind("3\t2\n"), Some((3, 2)));
        assert_eq!(parse_ahead_behind("3"), None);
        assert_eq!(parse_ahead_behind("3 2 1"), None);
    }

    #[test]
    fn parses_primary_linked_and_detached_worktrees() {
        let entries = parse_worktree_list(
            "worktree /project\nHEAD abc123\nbranch refs/heads/main\n\nworktree /project-feature\nHEAD def456\nbranch refs/heads/feature/test\n\nworktree /project-detached\nHEAD fedcba\ndetached\n",
        )
        .unwrap();
        assert_eq!(
            entries,
            vec![
                (Some(String::from("main")), String::from("/project")),
                (
                    Some(String::from("feature/test")),
                    String::from("/project-feature"),
                ),
                (None, String::from("/project-detached")),
            ]
        );
    }
}

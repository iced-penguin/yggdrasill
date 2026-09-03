use crate::app::branch_list_item::BranchListItem;
use crate::app::port::{GitRepositoryPort, RepositoryResult};

pub struct RepositoryUseCase<'a> {
    repo: &'a dyn GitRepositoryPort,
}

impl<'a> RepositoryUseCase<'a> {
    pub fn new(repo: &'a dyn GitRepositoryPort) -> Self {
        Self { repo }
    }

    pub fn refresh_branches(
        &self,
        include_sync_status: bool,
    ) -> RepositoryResult<Vec<BranchListItem>> {
        let records = self.repo.list_branches(include_sync_status)?;
        Ok(records.into_iter().map(BranchListItem::from).collect())
    }

    pub fn checkout_branch(&self, name: &str) -> RepositoryResult<()> {
        self.repo.checkout_branch(name)
    }

    pub fn add_worktree(&self, branch: &str, path: &str) -> RepositoryResult<()> {
        self.repo.add_worktree(branch, path)
    }

    pub fn remove_worktree(&self, path: &str) -> RepositoryResult<()> {
        self.repo.remove_worktree(path)
    }

    pub fn create_branch(&self, name: &str, parent: &str) -> RepositoryResult<()> {
        self.repo.create_branch(name, parent)
    }

    pub fn rename_branch(&self, old_name: &str, new_name: &str) -> RepositoryResult<()> {
        self.repo.rename_branch(old_name, new_name)
    }

    pub fn delete_branch(&self, name: &str) -> RepositoryResult<()> {
        self.repo.delete_branch(name)
    }

    pub fn get_diff_stat(
        &self,
        worktree_path: Option<&str>,
        branch_name: &str,
    ) -> RepositoryResult<String> {
        self.repo.diff_stat(worktree_path, branch_name)
    }

    pub fn get_worktree_path(
        &self,
        branch_name: &str,
        path_template: &str,
        base_directory: Option<&str>,
    ) -> RepositoryResult<String> {
        let repository_root = self.repo.repository_root()?;
        let repository_root = std::path::Path::new(&repository_root);
        let repository_name = repository_root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid repository name")?;
        let branch_slug = branch_name.replace('/', "-");
        let path = path_template
            .replace("{repo}", repository_name)
            .replace("{branch}", branch_name)
            .replace("{branch_slug}", &branch_slug);
        let path = std::path::PathBuf::from(path);
        if path.is_absolute() {
            return Err("worktree.path_template must be a relative path".into());
        }
        let base = match base_directory {
            Some(directory) => expand_home_directory(directory)?,
            None => repository_root
                .parent()
                .ok_or("repository has no parent directory")?
                .to_path_buf(),
        };
        Ok(base.join(path).display().to_string())
    }

    pub fn get_repository_root(&self) -> RepositoryResult<String> {
        self.repo.repository_root()
    }
}

fn expand_home_directory(directory: &str) -> RepositoryResult<std::path::PathBuf> {
    let suffix = if directory == "~" {
        Some("")
    } else {
        directory.strip_prefix("~/")
    };
    match suffix {
        Some(suffix) => {
            let home = std::env::var_os("HOME").ok_or("HOME environment variable is not set")?;
            Ok(std::path::PathBuf::from(home).join(suffix))
        }
        None => Ok(std::path::PathBuf::from(directory)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BranchRecord, HeadState, WorkingTreeStatus, WorktreeInfo};
    use std::cell::RefCell;

    struct FakeRepository {
        branches: RefCell<Vec<BranchRecord>>,
        should_fail: RefCell<bool>,
        committed_operations: RefCell<Vec<String>>,
    }

    impl FakeRepository {
        fn new(branches: Vec<BranchRecord>) -> Self {
            Self {
                branches: RefCell::new(branches),
                should_fail: RefCell::new(false),
                committed_operations: RefCell::new(Vec::new()),
            }
        }

        fn with_failure(self) -> Self {
            *self.should_fail.borrow_mut() = true;
            self
        }

        fn get_committed_operations(&self) -> Vec<String> {
            self.committed_operations.borrow().clone()
        }

        fn record_operation(&self, operation: String) {
            self.committed_operations.borrow_mut().push(operation);
        }
    }

    impl GitRepositoryPort for FakeRepository {
        fn list_branches(&self, _fetch: bool) -> RepositoryResult<Vec<BranchRecord>> {
            if *self.should_fail.borrow() {
                return Err("fake error: list_branches failed".into());
            }
            Ok(self.branches.borrow().clone())
        }

        fn checkout_branch(&self, name: &str) -> RepositoryResult<()> {
            if *self.should_fail.borrow() {
                return Err(format!("fake error: cannot checkout {}", name).into());
            }
            self.record_operation(format!("checkout {}", name));
            Ok(())
        }

        fn add_worktree(&self, branch: &str, path: &str) -> RepositoryResult<()> {
            if *self.should_fail.borrow() {
                return Err("fake error: add_worktree failed".into());
            }
            self.record_operation(format!("add_worktree {} at {}", branch, path));
            Ok(())
        }

        fn remove_worktree(&self, path: &str) -> RepositoryResult<()> {
            if *self.should_fail.borrow() {
                return Err("fake error: remove_worktree failed".into());
            }
            self.record_operation(format!("remove_worktree {}", path));
            Ok(())
        }

        fn create_branch(&self, name: &str, parent: &str) -> RepositoryResult<()> {
            if *self.should_fail.borrow() {
                return Err("fake error: create_branch failed".into());
            }
            self.record_operation(format!("create_branch {} from {}", name, parent));
            Ok(())
        }

        fn rename_branch(&self, old_name: &str, new_name: &str) -> RepositoryResult<()> {
            if *self.should_fail.borrow() {
                return Err("fake error: rename_branch failed".into());
            }
            self.record_operation(format!("rename_branch {} to {}", old_name, new_name));
            Ok(())
        }

        fn delete_branch(&self, name: &str) -> RepositoryResult<()> {
            if *self.should_fail.borrow() {
                return Err("fake error: delete_branch failed".into());
            }
            self.record_operation(format!("delete_branch {}", name));
            Ok(())
        }

        fn diff_stat(
            &self,
            worktree_path: Option<&str>,
            branch_name: &str,
        ) -> RepositoryResult<String> {
            if *self.should_fail.borrow() {
                return Err("fake error: diff_stat failed".into());
            }
            let location = worktree_path.unwrap_or(branch_name);
            Ok(format!(
                " 1 file changed, 5 insertions(+), 2 deletions(-) @ {}",
                location
            ))
        }

        fn repository_root(&self) -> RepositoryResult<String> {
            if *self.should_fail.borrow() {
                return Err("fake error: repository_root failed".into());
            }
            Ok(String::from("/tmp/repo"))
        }
    }

    #[test]
    fn refresh_branches_returns_list_items() {
        use crate::domain::SyncStatus;

        let branch = BranchRecord::new_branch(
            "main".into(),
            true,
            None,
            "abc123".into(),
            "Initial commit".into(),
            SyncStatus::UpToDate {
                upstream: "origin/main".into(),
            },
        );

        let fake = FakeRepository::new(vec![branch]);
        let use_case = RepositoryUseCase::new(&fake);
        let items = use_case.refresh_branches(true).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "main");
        assert!(items[0].is_current);
    }

    #[test]
    fn refresh_branches_handles_multiple_branches() {
        use crate::domain::SyncStatus;

        let branches = vec![
            BranchRecord::new_branch(
                "main".into(),
                true,
                None,
                "abc123".into(),
                "Initial commit".into(),
                SyncStatus::UpToDate {
                    upstream: "origin/main".into(),
                },
            ),
            BranchRecord::new_branch(
                "feature".into(),
                false,
                None,
                "def456".into(),
                "Add feature".into(),
                SyncStatus::Ahead {
                    upstream: "origin/feature".into(),
                    count: 2,
                },
            ),
        ];

        let fake = FakeRepository::new(branches);
        let use_case = RepositoryUseCase::new(&fake);
        let items = use_case.refresh_branches(true).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "main");
        assert_eq!(items[1].name, "feature");
        assert_eq!(items[1].ahead(), Some(2));
    }

    #[test]
    fn refresh_branches_includes_detached_head() {
        let branch = BranchRecord::new_non_branch_worktree(
            WorktreeInfo {
                path: "/tmp/detached".into(),
                head: HeadState::Detached("abc123".into()),
                status: WorkingTreeStatus::Clean,
                commit_hash: "abc123".into(),
                commit_subject: "Detached HEAD".into(),
            },
            false,
        );

        let fake = FakeRepository::new(vec![branch]);
        let use_case = RepositoryUseCase::new(&fake);
        let items = use_case.refresh_branches(true).unwrap();

        assert_eq!(items.len(), 1);
        assert!(items[0].is_detached());
        assert!(!items[0].is_unborn());
    }

    #[test]
    fn refresh_branches_includes_unborn_head() {
        let branch = BranchRecord::new_non_branch_worktree(
            WorktreeInfo {
                path: "/tmp/repo".into(),
                head: HeadState::Unborn,
                status: WorkingTreeStatus::Clean,
                commit_hash: "-".into(),
                commit_subject: "No commits yet".into(),
            },
            true,
        );

        let fake = FakeRepository::new(vec![branch]);
        let use_case = RepositoryUseCase::new(&fake);
        let items = use_case.refresh_branches(true).unwrap();

        assert_eq!(items.len(), 1);
        assert!(items[0].is_unborn());
        assert!(!items[0].is_detached());
    }

    #[test]
    fn checkout_branch_calls_port() {
        let fake = FakeRepository::new(vec![]);
        let use_case = RepositoryUseCase::new(&fake);
        use_case.checkout_branch("feature").unwrap();

        let operations = fake.get_committed_operations();
        assert_eq!(operations.len(), 1);
        assert!(operations[0].contains("checkout feature"));
    }

    #[test]
    fn checkout_branch_handles_errors() {
        let fake = FakeRepository::new(vec![]).with_failure();
        let use_case = RepositoryUseCase::new(&fake);
        let result = use_case.checkout_branch("missing-branch");

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot checkout"));
    }

    #[test]
    fn worktree_operations_are_tracked() {
        let fake = FakeRepository::new(vec![]);
        let use_case = RepositoryUseCase::new(&fake);

        use_case
            .add_worktree("feature", "/tmp/repo-feature")
            .unwrap();
        use_case.remove_worktree("/tmp/repo-feature").unwrap();

        let operations = fake.get_committed_operations();
        assert_eq!(operations.len(), 2);
        assert!(operations[0].contains("add_worktree feature"));
        assert!(operations[1].contains("remove_worktree"));
    }

    #[test]
    fn branch_operations_are_tracked() {
        let fake = FakeRepository::new(vec![]);
        let use_case = RepositoryUseCase::new(&fake);

        use_case.create_branch("feature", "main").unwrap();
        use_case.rename_branch("feature", "feature-new").unwrap();
        use_case.delete_branch("feature-new").unwrap();

        let operations = fake.get_committed_operations();
        assert_eq!(operations.len(), 3);
        assert!(operations[0].contains("create_branch feature"));
        assert!(operations[1].contains("rename_branch"));
        assert!(operations[2].contains("delete_branch"));
    }

    #[test]
    fn diff_stat_includes_location_info() {
        let fake = FakeRepository::new(vec![]);
        let use_case = RepositoryUseCase::new(&fake);

        let diff = use_case.get_diff_stat(None, "main").unwrap();
        assert!(diff.contains("@ main"));

        let diff_with_worktree = use_case
            .get_diff_stat(Some("/tmp/repo-main"), "main")
            .unwrap();
        assert!(diff_with_worktree.contains("@ /tmp/repo-main"));
    }

    #[test]
    fn get_repository_root_works() {
        let fake = FakeRepository::new(vec![]);
        let use_case = RepositoryUseCase::new(&fake);
        let root = use_case.get_repository_root().unwrap();

        assert_eq!(root, "/tmp/repo");
    }

    #[test]
    fn get_worktree_path_applies_template() {
        let fake = FakeRepository::new(vec![]);
        let use_case = RepositoryUseCase::new(&fake);
        let path = use_case
            .get_worktree_path("feature/test", "{repo}-{branch_slug}", None)
            .unwrap();

        assert_eq!(path, "/tmp/repo-feature-test");
    }

    #[test]
    fn get_worktree_path_supports_branch_name_template() {
        let fake = FakeRepository::new(vec![]);
        let use_case = RepositoryUseCase::new(&fake);
        let path = use_case
            .get_worktree_path("feature/test", "worktrees/{branch}", None)
            .unwrap();

        assert_eq!(path, "/tmp/worktrees/feature/test");
    }

    #[test]
    fn get_worktree_path_uses_configured_base_directory() {
        let fake = FakeRepository::new(vec![]);
        let use_case = RepositoryUseCase::new(&fake);
        let path = use_case
            .get_worktree_path("feature/test", "{repo}-{branch_slug}", Some("/var/wt"))
            .unwrap();

        assert_eq!(path, "/var/wt/repo-feature-test");
    }

    #[test]
    fn get_worktree_path_expands_home_in_base_directory() {
        let fake = FakeRepository::new(vec![]);
        let use_case = RepositoryUseCase::new(&fake);
        let home = std::env::var_os("HOME").expect("HOME must be set for this test");
        let expected = std::path::PathBuf::from(home).join("wt/repo-feature-test");
        let path = use_case
            .get_worktree_path("feature/test", "{repo}-{branch_slug}", Some("~/wt"))
            .unwrap();

        assert_eq!(path, expected.display().to_string());
    }

    #[test]
    fn operations_fail_when_fake_fails() {
        let fake = FakeRepository::new(vec![]).with_failure();
        let use_case = RepositoryUseCase::new(&fake);

        assert!(use_case.checkout_branch("any").is_err());
        assert!(use_case.add_worktree("any", "any").is_err());
        assert!(use_case.remove_worktree("any").is_err());
        assert!(use_case.create_branch("any", "any").is_err());
        assert!(use_case.rename_branch("any", "any").is_err());
        assert!(use_case.delete_branch("any").is_err());
        assert!(use_case.get_diff_stat(None, "any").is_err());
    }
}

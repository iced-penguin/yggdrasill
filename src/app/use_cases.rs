use crate::app::branch_list_item::BranchListItem;
use crate::app::port::{GitRepositoryPort, RepositoryResult};
use crate::domain::BranchRecord;

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

    pub fn get_default_worktree_path(&self, branch_name: &str) -> RepositoryResult<String> {
        self.repo.default_worktree_path(branch_name)
    }

    pub fn get_repository_root(&self) -> RepositoryResult<String> {
        self.repo.repository_root()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeRepository {
        branches: RefCell<Vec<BranchRecord>>,
    }

    impl FakeRepository {
        fn new(branches: Vec<BranchRecord>) -> Self {
            Self {
                branches: RefCell::new(branches),
            }
        }
    }

    impl GitRepositoryPort for FakeRepository {
        fn list_branches(&self, _fetch: bool) -> RepositoryResult<Vec<BranchRecord>> {
            Ok(self.branches.borrow().clone())
        }

        fn checkout_branch(&self, _name: &str) -> RepositoryResult<()> {
            Ok(())
        }

        fn add_worktree(&self, _branch: &str, _path: &str) -> RepositoryResult<()> {
            Ok(())
        }

        fn remove_worktree(&self, _path: &str) -> RepositoryResult<()> {
            Ok(())
        }

        fn create_branch(&self, _name: &str, _parent: &str) -> RepositoryResult<()> {
            Ok(())
        }

        fn rename_branch(&self, _old_name: &str, _new_name: &str) -> RepositoryResult<()> {
            Ok(())
        }

        fn delete_branch(&self, _name: &str) -> RepositoryResult<()> {
            Ok(())
        }

        fn diff_stat(
            &self,
            _worktree_path: Option<&str>,
            _branch_name: &str,
        ) -> RepositoryResult<String> {
            Ok(String::from("No changes"))
        }

        fn repository_root(&self) -> RepositoryResult<String> {
            Ok(String::from("/tmp/repo"))
        }

        fn default_worktree_path(&self, branch_name: &str) -> RepositoryResult<String> {
            Ok(format!("/tmp/repo-{}", branch_name))
        }
    }

    #[test]
    fn refresh_branches_returns_list_items() {
        use crate::domain::{HeadState, SyncStatus, WorkingTreeStatus};

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
    fn checkout_branch_calls_port() {
        let fake = FakeRepository::new(vec![]);
        let use_case = RepositoryUseCase::new(&fake);
        assert!(use_case.checkout_branch("feature").is_ok());
    }
}

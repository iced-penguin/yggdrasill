use crate::domain::BranchRecord;

pub type RepositoryResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub trait GitRepositoryPort {
    fn list_branches(&self, fetch: bool) -> RepositoryResult<Vec<BranchRecord>>;
    fn checkout_branch(&self, name: &str) -> RepositoryResult<()>;
    fn add_worktree(&self, branch: &str, path: &str) -> RepositoryResult<()>;
    fn remove_worktree(&self, path: &str) -> RepositoryResult<()>;
    fn create_branch(&self, name: &str, parent: &str) -> RepositoryResult<()>;
    fn rename_branch(&self, old_name: &str, new_name: &str) -> RepositoryResult<()>;
    fn delete_branch(&self, name: &str) -> RepositoryResult<()>;
    fn diff_stat(&self, worktree_path: Option<&str>, branch_name: &str)
    -> RepositoryResult<String>;
    fn repository_root(&self) -> RepositoryResult<String>;
    fn default_worktree_path(&self, branch_name: &str) -> RepositoryResult<String>;
}

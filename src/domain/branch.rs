use super::sync_status::SyncStatus;
use super::worktree::{HeadState, WorkingTreeStatus, WorktreeInfo};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchRecord {
    pub name: String,
    pub is_current: bool,
    pub worktree: Option<WorktreeInfo>,
    pub commit_hash: String,
    pub commit_subject: String,
    pub head_state: HeadState,
    pub sync_status: SyncStatus,
}

impl BranchRecord {
    pub fn new_branch(
        name: String,
        is_current: bool,
        worktree: Option<WorktreeInfo>,
        commit_hash: String,
        commit_subject: String,
        sync_status: SyncStatus,
    ) -> Self {
        Self {
            head_state: HeadState::Branch(name.clone()),
            name,
            is_current,
            worktree,
            commit_hash,
            commit_subject,
            sync_status,
        }
    }

    pub fn new_non_branch_worktree(worktree: WorktreeInfo, is_current: bool) -> Self {
        let name = worktree.head.display_name();
        let commit_hash = worktree.commit_hash.clone();
        let commit_subject = worktree.commit_subject.clone();
        let head_state = worktree.head.clone();
        Self {
            name,
            is_current,
            head_state,
            worktree: Some(worktree),
            commit_hash,
            commit_subject,
            sync_status: SyncStatus::Unknown,
        }
    }

    pub fn worktree_path(&self) -> Option<&str> {
        self.worktree.as_ref().map(|w| w.path.as_str())
    }

    pub fn working_tree_status(&self) -> WorkingTreeStatus {
        self.worktree
            .as_ref()
            .map_or(WorkingTreeStatus::Unknown, |w| w.status)
    }

    pub fn is_dirty(&self) -> Option<bool> {
        self.working_tree_status().is_dirty()
    }
}

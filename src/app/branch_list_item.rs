use crate::domain::{BranchRecord, HeadState, SyncStatus, WorkingTreeStatus};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchListItem {
    pub name: String,
    pub is_current: bool,
    pub worktree_path: Option<String>,
    pub status: WorkingTreeStatus,
    pub commit_hash: String,
    pub commit_subject: String,
    pub head_state: HeadState,
    pub sync_status: SyncStatus,
}

impl BranchListItem {
    pub fn is_dirty(&self) -> Option<bool> {
        self.status.is_dirty()
    }

    pub fn is_detached(&self) -> bool {
        self.head_state.is_detached()
    }

    pub fn is_unborn(&self) -> bool {
        self.head_state.is_unborn()
    }

    pub fn upstream(&self) -> Option<&str> {
        self.sync_status.upstream()
    }

    pub fn ahead(&self) -> Option<usize> {
        self.sync_status.ahead()
    }

    pub fn behind(&self) -> Option<usize> {
        self.sync_status.behind()
    }

    pub fn unpublished_commits(&self) -> Option<usize> {
        self.sync_status.unpublished_commits()
    }

    pub fn to_branch_entry(&self) -> super::state::BranchEntry {
        super::state::BranchEntry {
            name: self.name.clone(),
            is_current: self.is_current,
            worktree_path: self.worktree_path.clone(),
            is_dirty: self.is_dirty(),
            commit_hash: self.commit_hash.clone(),
            commit_subject: self.commit_subject.clone(),
            is_detached: self.is_detached(),
            is_unborn: self.is_unborn(),
            upstream: self.upstream().map(str::to_owned),
            ahead: self.ahead(),
            behind: self.behind(),
            unpublished_commits: self.unpublished_commits(),
        }
    }
}

impl From<BranchRecord> for BranchListItem {
    fn from(record: BranchRecord) -> Self {
        let status = record.working_tree_status();
        let worktree_path = record.worktree_path().map(str::to_owned);
        Self {
            name: record.name,
            is_current: record.is_current,
            worktree_path,
            status,
            commit_hash: record.commit_hash,
            commit_subject: record.commit_subject,
            head_state: record.head_state,
            sync_status: record.sync_status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_from_branch_record() {
        let record = BranchRecord::new_branch(
            "feature/test".into(),
            true,
            None,
            "abc1234".into(),
            "Commit msg".into(),
            SyncStatus::UpToDate {
                upstream: "origin/feature/test".into(),
            },
        );
        let item: BranchListItem = record.into();
        assert_eq!(item.name, "feature/test");
        assert!(item.is_current);
        assert_eq!(item.upstream(), Some("origin/feature/test"));
        assert_eq!(item.ahead(), Some(0));
        assert_eq!(item.behind(), Some(0));
        assert!(!item.is_detached());
        assert!(!item.is_unborn());
        assert_eq!(item.is_dirty(), None);
    }
}

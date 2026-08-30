#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkingTreeStatus {
    Clean,
    Modified,
    Unknown,
}

impl WorkingTreeStatus {
    pub fn is_dirty(&self) -> Option<bool> {
        match self {
            Self::Clean => Some(false),
            Self::Modified => Some(true),
            Self::Unknown => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadState {
    Branch(String),
    Detached(String),
    Unborn,
}

impl HeadState {
    pub fn is_detached(&self) -> bool {
        matches!(self, Self::Detached(_))
    }

    pub fn is_unborn(&self) -> bool {
        matches!(self, Self::Unborn)
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::Branch(name) => name.clone(),
            Self::Detached(commit) => format!("DETACHED ({commit})"),
            Self::Unborn => "UNBORN HEAD".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub path: String,
    pub head: HeadState,
    pub status: WorkingTreeStatus,
    pub commit_hash: String,
    pub commit_subject: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_state_helpers() {
        let branch = HeadState::Branch("main".into());
        assert!(!branch.is_detached());
        assert!(!branch.is_unborn());
        assert_eq!(branch.display_name(), "main");

        let detached = HeadState::Detached("abc1234".into());
        assert!(detached.is_detached());
        assert_eq!(detached.display_name(), "DETACHED (abc1234)");

        let unborn = HeadState::Unborn;
        assert!(unborn.is_unborn());
        assert_eq!(unborn.display_name(), "UNBORN HEAD");
    }

    #[test]
    fn working_tree_status_conversions() {
        assert_eq!(WorkingTreeStatus::Clean.is_dirty(), Some(false));
        assert_eq!(WorkingTreeStatus::Modified.is_dirty(), Some(true));
        assert_eq!(WorkingTreeStatus::Unknown.is_dirty(), None);

        assert_eq!(
            WorkingTreeStatus::from_dirty_bool(Some(true)),
            WorkingTreeStatus::Modified
        );
        assert_eq!(
            WorkingTreeStatus::from_dirty_bool(Some(false)),
            WorkingTreeStatus::Clean
        );
        assert_eq!(
            WorkingTreeStatus::from_dirty_bool(None),
            WorkingTreeStatus::Unknown
        );
    }
}

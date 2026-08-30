#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    UpToDate {
        upstream: String,
    },
    Ahead {
        upstream: String,
        count: usize,
    },
    Behind {
        upstream: String,
        count: usize,
    },
    Diverged {
        upstream: String,
        ahead: usize,
        behind: usize,
    },
    NoUpstream {
        unpublished: Option<usize>,
    },
    Unknown,
}

impl SyncStatus {
    pub fn upstream(&self) -> Option<&str> {
        match self {
            Self::UpToDate { upstream }
            | Self::Ahead { upstream, .. }
            | Self::Behind { upstream, .. }
            | Self::Diverged { upstream, .. } => Some(upstream),
            Self::NoUpstream { .. } | Self::Unknown => None,
        }
    }

    pub fn ahead(&self) -> Option<usize> {
        match self {
            Self::UpToDate { .. } | Self::Behind { .. } => Some(0),
            Self::Ahead { count, .. } => Some(*count),
            Self::Diverged { ahead, .. } => Some(*ahead),
            Self::NoUpstream { .. } | Self::Unknown => None,
        }
    }

    pub fn behind(&self) -> Option<usize> {
        match self {
            Self::UpToDate { .. } | Self::Ahead { .. } => Some(0),
            Self::Behind { count, .. } => Some(*count),
            Self::Diverged { behind, .. } => Some(*behind),
            Self::NoUpstream { .. } | Self::Unknown => None,
        }
    }

    pub fn unpublished_commits(&self) -> Option<usize> {
        match self {
            Self::NoUpstream { unpublished } => *unpublished,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_status_accessors() {
        let up = SyncStatus::UpToDate {
            upstream: "origin/main".into(),
        };
        assert_eq!(up.upstream(), Some("origin/main"));
        assert_eq!(up.ahead(), Some(0));
        assert_eq!(up.behind(), Some(0));

        let diverged = SyncStatus::Diverged {
            upstream: "origin/feat".into(),
            ahead: 3,
            behind: 2,
        };
        assert_eq!(diverged.upstream(), Some("origin/feat"));
        assert_eq!(diverged.ahead(), Some(3));
        assert_eq!(diverged.behind(), Some(2));

        let no_up = SyncStatus::NoUpstream {
            unpublished: Some(4),
        };
        assert_eq!(no_up.upstream(), None);
        assert_eq!(no_up.ahead(), None);
        assert_eq!(no_up.behind(), None);
        assert_eq!(no_up.unpublished_commits(), Some(4));
    }
}

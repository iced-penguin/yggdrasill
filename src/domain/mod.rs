//! Domain model representing Git concepts independently of UI and CLI infrastructure.
//!
//! # Concepts
//! - [`branch::BranchRecord`]: A branch or worktree entry in the repository.
//! - [`worktree::HeadState`]: The state of HEAD (`Branch`, `Detached`, or `Unborn`).
//! - [`worktree::WorkingTreeStatus`]: The dirty state of the working tree (`Clean`, `Modified`, or `Unknown`).
//! - [`sync_status::SyncStatus`]: Synchronization state with remote (`UpToDate`, `Ahead`, `Behind`, `Diverged`, etc.).

pub mod branch;
pub mod sync_status;
pub mod worktree;

pub use branch::BranchRecord;
pub use sync_status::SyncStatus;
pub use worktree::{HeadState, WorkingTreeStatus, WorktreeInfo};

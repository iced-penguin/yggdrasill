use std::time::Duration;

pub const HELP_TEXT: &str = "Navigation\n  Up/Down, j/k  Select item\n  Enter           Open worktree or checkout branch (confirms with local changes)\n  /               Search\n  d               Show diff\n\nWorktrees\n  a               Add worktree (enter path first)\n  x               Remove selected worktree (confirmation required)\n\nBranches\n  b               Create branch (no confirmation)\n  e               Rename selected branch (no confirmation)\n  D               Delete selected branch (confirmation required)\n\nOther\n  r               Refresh\n  q               Quit\n  h, Esc          Close this help";
pub const HIGHLIGHT_SYMBOL: &str = "▶ ";

pub const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub const WIDE_LAYOUT_BREAKPOINT_WIDTH: u16 = 110;
pub const FOOTER_COMPACT_BREAKPOINT_WIDTH: u16 = 80;
pub const FOOTER_MEDIUM_BREAKPOINT_WIDTH: u16 = 130;

pub const FILTER_BAR_HEIGHT: u16 = 3;
pub const FOOTER_BAR_HEIGHT: u16 = 5;
pub const MAIN_MIN_HEIGHT: u16 = 5;

pub const BRANCH_COLUMN_PERCENT: u16 = 30;
pub const STATUS_COLUMN_WIDTH: u16 = 8;
pub const WORKTREE_COLUMN_MIN_WIDTH: u16 = 20;

pub const DETAILS_PANE_TOTAL_LINES: usize = 10;

pub const LINE_SCROLL_DISTANCE: isize = 1;
pub const PAGE_SCROLL_DISTANCE: isize = 5;

pub const ELLIPSIS: &str = "...";
pub const ELLIPSIS_WIDTH: usize = 3;

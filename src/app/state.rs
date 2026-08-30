use ratatui::widgets::TableState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchEntry {
    pub name: String,
    pub is_current: bool,
    pub worktree_path: Option<String>,
    pub is_dirty: Option<bool>,
    pub commit_hash: String,
    pub commit_subject: String,
    pub is_detached: bool,
    pub is_unborn: bool,
    pub upstream: Option<String>,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
    pub unpublished_commits: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingAction {
    Checkout(BranchEntry),
    AddWorktree { branch: BranchEntry, path: String },
    RemoveWorktree(BranchEntry),
    CreateBranch { name: String, parent: String },
    RenameBranch { old_name: String, new_name: String },
    DeleteBranch(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputMode {
    AddWorktree { branch: BranchEntry, path: String },
    CreateBranch { name: String, parent: String },
    RenameBranch { old_name: String, new_name: String },
}

pub struct TuiApp {
    pub all_entries: Vec<BranchEntry>,
    pub entries: Vec<BranchEntry>,
    pub table_state: TableState,
    pub message: String,
    pub pending_action: Option<PendingAction>,
    pub input_mode: Option<InputMode>,
    pub diff_view: Option<String>,
    pub help_visible: bool,
    pub help_scroll: u16,
    pub help_max_scroll: u16,
    pub filter: Option<String>,
    pub filtering: bool,
    pub details_dialog_visible: bool,
    pub details_scroll: u16,
    pub details_max_scroll: u16,
}

impl TuiApp {
    pub fn new(mut entries: Vec<BranchEntry>) -> Self {
        entries.sort_by_key(|entry| !entry.is_current);
        let mut table_state = TableState::default();
        if !entries.is_empty() {
            table_state.select(Some(0));
        }
        Self {
            all_entries: entries.clone(),
            entries,
            table_state,
            message: String::new(),
            pending_action: None,
            input_mode: None,
            diff_view: None,
            help_visible: false,
            help_scroll: 0,
            help_max_scroll: 0,
            filter: None,
            filtering: false,
            details_dialog_visible: false,
            details_scroll: 0,
            details_max_scroll: 0,
        }
    }

    pub fn selected(&self) -> Option<&BranchEntry> {
        self.table_state
            .selected()
            .and_then(|index| self.entries.get(index))
    }

    pub fn move_selection(&mut self, amount: isize) {
        if self.entries.is_empty() {
            return;
        }
        let current = self.table_state.selected().unwrap_or_default() as isize;
        let entry_count = self.entries.len() as isize;
        self.table_state
            .select(Some((current + amount).rem_euclid(entry_count) as usize));
    }

    pub fn reload(&mut self, mut entries: Vec<BranchEntry>) {
        let selected_name = self.selected().map(|entry| entry.name.clone());
        let previous_index = self.table_state.selected().unwrap_or_default();
        entries.sort_by_key(|entry| !entry.is_current);
        self.all_entries = entries;
        self.apply_filter(selected_name, previous_index);
    }

    pub fn apply_filter(&mut self, selected_name: Option<String>, previous_index: usize) {
        let query = self.filter.as_deref().unwrap_or("").to_ascii_lowercase();
        self.entries = self
            .all_entries
            .iter()
            .filter(|entry| {
                entry.name.to_ascii_lowercase().contains(&query)
                    || entry
                        .worktree_path
                        .as_deref()
                        .is_some_and(|path| path.to_ascii_lowercase().contains(&query))
                    || entry.commit_subject.to_ascii_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        let selected_index = selected_name
            .as_ref()
            .and_then(|name| self.entries.iter().position(|entry| &entry.name == name))
            .or_else(|| {
                (!self.entries.is_empty()).then_some(previous_index.min(self.entries.len() - 1))
            });
        self.table_state.select(selected_index);
    }

    pub fn update_filter(&mut self, filter: Option<String>) {
        let selected_name = self.selected().map(|entry| entry.name.clone());
        let previous_index = self.table_state.selected().unwrap_or_default();
        self.filter = filter;
        self.apply_filter(selected_name, previous_index);
    }

    pub fn scroll_help(&mut self, amount: isize) {
        self.help_scroll = if amount.is_negative() {
            self.help_scroll
                .saturating_sub(amount.unsigned_abs() as u16)
        } else {
            self.help_scroll
                .saturating_add(amount as u16)
                .min(self.help_max_scroll)
        };
    }

    pub fn scroll_details(&mut self, amount: isize) {
        self.details_scroll = if amount.is_negative() {
            self.details_scroll
                .saturating_sub(amount.unsigned_abs() as u16)
        } else {
            self.details_scroll
                .saturating_add(amount as u16)
                .min(self.details_max_scroll)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, is_current: bool, is_dirty: Option<bool>) -> BranchEntry {
        BranchEntry {
            name: name.to_owned(),
            is_current,
            worktree_path: None,
            is_dirty,
            commit_hash: String::from("abc1234"),
            commit_subject: String::from("Test subject"),
            is_detached: false,
            is_unborn: false,
            upstream: None,
            ahead: None,
            behind: None,
            unpublished_commits: None,
        }
    }

    #[test]
    fn current_branch_is_first() {
        let app = TuiApp::new(vec![
            entry("other", false, None),
            entry("current", true, Some(false)),
        ]);
        assert_eq!(app.entries[0].name, "current");
    }

    #[test]
    fn dirty_state_is_preserved() {
        let app = TuiApp::new(vec![entry("current", true, Some(true))]);
        assert_eq!(app.selected().and_then(|entry| entry.is_dirty), Some(true));
    }

    #[test]
    fn selection_wraps_in_both_directions() {
        let mut app = TuiApp::new(vec![entry("first", true, None), entry("last", false, None)]);
        app.move_selection(-1);
        assert_eq!(
            app.selected().map(|entry| entry.name.as_str()),
            Some("last")
        );
        app.move_selection(1);
        assert_eq!(
            app.selected().map(|entry| entry.name.as_str()),
            Some("first")
        );
    }

    #[test]
    fn empty_list_ignores_selection_movement() {
        let mut app = TuiApp::new(Vec::new());
        app.move_selection(1);
        assert_eq!(app.table_state.selected(), None);
    }

    #[test]
    fn help_scrolling_stays_within_visible_bounds() {
        let mut app = TuiApp::new(Vec::new());
        app.help_max_scroll = 3;

        app.scroll_help(-1);
        assert_eq!(app.help_scroll, 0);

        app.scroll_help(5);
        assert_eq!(app.help_scroll, 3);

        app.scroll_help(-2);
        assert_eq!(app.help_scroll, 1);
    }

    #[test]
    fn details_scrolling_stays_within_bounds() {
        let mut app = TuiApp::new(Vec::new());
        app.details_max_scroll = 5;

        app.scroll_details(-1);
        assert_eq!(app.details_scroll, 0);

        app.scroll_details(10);
        assert_eq!(app.details_scroll, 5);

        app.scroll_details(-3);
        assert_eq!(app.details_scroll, 2);
    }

    #[test]
    fn filter_matches_branch_path_and_subject() {
        let mut path_entry = entry("other", false, None);
        path_entry.worktree_path = Some(String::from("/tmp/project-feature"));
        let mut subject_entry = entry("third", false, None);
        subject_entry.commit_subject = String::from("Fix checkout failure");
        let mut app = TuiApp::new(vec![
            entry("feature/login", true, None),
            path_entry,
            subject_entry,
        ]);

        app.update_filter(Some(String::from("project")));
        assert_eq!(app.entries.len(), 1);
        assert_eq!(app.entries[0].name, "other");

        app.update_filter(Some(String::from("checkout")));
        assert_eq!(app.entries.len(), 1);
        assert_eq!(app.entries[0].name, "third");
    }

    #[test]
    fn detached_and_unborn_worktrees_remain_selectable_entries() {
        let mut detached = entry("DETACHED (abc1234)", true, Some(false));
        detached.worktree_path = Some(String::from("/tmp/detached"));
        detached.is_detached = true;
        let mut unborn = entry("UNBORN HEAD", false, Some(false));
        unborn.worktree_path = Some(String::from("/tmp/unborn"));
        unborn.is_unborn = true;

        let app = TuiApp::new(vec![detached, unborn]);
        assert_eq!(app.entries.len(), 2);
        assert!(app.entries.iter().any(|entry| entry.is_detached));
        assert!(app.entries.iter().any(|entry| entry.is_unborn));
    }
}

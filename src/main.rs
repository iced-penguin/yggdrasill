use std::{
    io::stderr,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const HELP_TEXT: &str = "Navigation\n  Up/Down, j/k  Select item\n  Enter           Open worktree or checkout branch (confirms with local changes)\n  /               Search\n  d               Show diff\n\nWorktrees\n  a               Add worktree (enter path first)\n  x               Remove selected worktree (confirmation required)\n\nBranches\n  b               Create branch (no confirmation)\n  e               Rename selected branch (no confirmation)\n  D               Delete selected branch (confirmation required)\n\nOther\n  r               Refresh\n  q               Quit\n  h, Esc          Close this help";
const HIGHLIGHT_SYMBOL: &str = "▶ ";

#[derive(Clone)]
struct BranchEntry {
    name: String,
    is_current: bool,
    worktree_path: Option<String>,
    is_dirty: Option<bool>,
    commit_hash: String,
    commit_subject: String,
    is_detached: bool,
    is_unborn: bool,
    upstream: Option<String>,
    ahead: Option<usize>,
    behind: Option<usize>,
    unpublished_commits: Option<usize>,
}

struct TuiApp {
    all_entries: Vec<BranchEntry>,
    entries: Vec<BranchEntry>,
    table_state: TableState,
    message: String,
    pending_action: Option<PendingAction>,
    input_mode: Option<InputMode>,
    diff_view: Option<String>,
    help_visible: bool,
    help_scroll: u16,
    help_max_scroll: u16,
    filter: Option<String>,
    filtering: bool,
    details_dialog_visible: bool,
    details_scroll: u16,
    details_max_scroll: u16,
}

enum PendingAction {
    Checkout(BranchEntry),
    AddWorktree { branch: BranchEntry, path: String },
    RemoveWorktree(BranchEntry),
    CreateBranch { name: String, parent: String },
    RenameBranch { old_name: String, new_name: String },
    DeleteBranch(String),
}

enum InputMode {
    AddWorktree { branch: BranchEntry, path: String },
    CreateBranch { name: String, parent: String },
    RenameBranch { old_name: String, new_name: String },
}

enum TuiAction {
    Cancel,
    Select(BranchEntry),
}

struct TerminalGuard {
    raw_mode_enabled: bool,
    alternate_screen_entered: bool,
}

impl TerminalGuard {
    fn activate() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        enable_raw_mode()?;
        let mut guard = Self {
            raw_mode_enabled: true,
            alternate_screen_entered: false,
        };
        execute!(stderr(), EnterAlternateScreen)?;
        guard.alternate_screen_entered = true;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.raw_mode_enabled {
            let _ = disable_raw_mode();
        }
        if self.alternate_screen_entered {
            let _ = execute!(stderr(), LeaveAlternateScreen, Show);
        }
    }
}

impl TuiApp {
    fn new(mut entries: Vec<BranchEntry>) -> Self {
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

    fn selected(&self) -> Option<&BranchEntry> {
        self.table_state
            .selected()
            .and_then(|index| self.entries.get(index))
    }

    fn move_selection(&mut self, amount: isize) {
        if self.entries.is_empty() {
            return;
        }
        let current = self.table_state.selected().unwrap_or_default() as isize;
        let entry_count = self.entries.len() as isize;
        self.table_state
            .select(Some((current + amount).rem_euclid(entry_count) as usize));
    }

    fn reload(&mut self, mut entries: Vec<BranchEntry>) {
        let selected_name = self.selected().map(|entry| entry.name.clone());
        let previous_index = self.table_state.selected().unwrap_or_default();
        entries.sort_by_key(|entry| !entry.is_current);
        self.all_entries = entries;
        self.apply_filter(selected_name, previous_index);
    }

    fn apply_filter(&mut self, selected_name: Option<String>, previous_index: usize) {
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

    fn update_filter(&mut self, filter: Option<String>) {
        let selected_name = self.selected().map(|entry| entry.name.clone());
        let previous_index = self.table_state.selected().unwrap_or_default();
        self.filter = filter;
        self.apply_filter(selected_name, previous_index);
    }

    fn scroll_help(&mut self, amount: isize) {
        self.help_scroll = if amount.is_negative() {
            self.help_scroll
                .saturating_sub(amount.unsigned_abs() as u16)
        } else {
            self.help_scroll
                .saturating_add(amount as u16)
                .min(self.help_max_scroll)
        };
    }

    fn scroll_details(&mut self, amount: isize) {
        self.details_scroll = if amount.is_negative() {
            self.details_scroll
                .saturating_sub(amount.unsigned_abs() as u16)
        } else {
            self.details_scroll
                .saturating_add(amount as u16)
                .min(self.details_max_scroll)
        };
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let terminal_width = frame.area().width as usize;
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(5)])
            .split(frame.area());

        let (branches_area, details_area, footer_area) = if terminal_width >= 110 {
            let main_areas = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
                .split(layout[1]);
            let left_areas = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(5), Constraint::Length(5)])
                .split(main_areas[0]);
            (left_areas[0], main_areas[1], left_areas[1])
        } else {
            let main_areas = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(62),
                    Constraint::Percentage(38),
                    Constraint::Length(5),
                ])
                .split(layout[1]);
            (main_areas[0], main_areas[1], main_areas[2])
        };
            let table_columns = [
                Constraint::Percentage(30),
                Constraint::Length(8),
                Constraint::Min(20),
            ];
            let table_area = Block::default().borders(Borders::ALL).inner(branches_area);
            let columns_area = Layout::horizontal([
                Constraint::Length(HIGHLIGHT_SYMBOL.width() as u16),
                Constraint::Fill(0),
            ])
            .areas::<2>(table_area)[1];
            let column_areas = Layout::horizontal(table_columns)
                .spacing(1)
                .split(columns_area);
            let branch_width = column_areas[0].width.saturating_sub(2) as usize;
            let worktree_width = column_areas[2].width as usize;
        let filter_text = match self.filter.as_deref() {
            Some(filter) => {
                let query_width = terminal_width.saturating_sub(28).max(8);
                let cursor = if self.filtering { "_" } else { "" };
                format!(
                    "/ {}{cursor}  {} of {} matches",
                    truncate_end(filter, query_width),
                    self.entries.len(),
                    self.all_entries.len(),
                )
            }
            None => format!(
                "/  Filter branches, worktrees, and commits  {} branches",
                self.all_entries.len(),
            ),
        };
        let filter_style = if self.filtering {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let filter = Paragraph::new(filter_text)
            .style(filter_style)
            .block(Block::default().title(" Filter ").borders(Borders::ALL));
        frame.render_widget(filter, layout[0]);

        let header = Row::new([
            Cell::from("BRANCH"),
            Cell::from("STATUS"),
            Cell::from("WORKTREE"),
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);
        let rows = self.entries.iter().map(|entry| {
            let (marker, color) = if entry.is_unborn {
                ("?", Color::Yellow)
            } else if entry.is_detached {
                ("!", Color::Magenta)
            } else if entry.is_current {
                ("*", Color::Cyan)
            } else if entry.worktree_path.is_some() {
                ("+", Color::Blue)
            } else {
                (" ", Color::Green)
            };
            Row::new([
                Cell::from(format!(
                    "{marker} {}",
                    truncate_start(&entry.name, branch_width)
                )),
                Cell::from(status_markers(entry)),
                Cell::from(truncate_start(
                    entry.worktree_path.as_deref().unwrap_or("-"),
                    worktree_width,
                )),
            ])
            .style(Style::default().fg(color))
        });
        let table = Table::new(
            rows,
            table_columns,
        )
        .header(header)
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(HIGHLIGHT_SYMBOL)
        .block(Block::default().title(" Branches ").borders(Borders::ALL));
        if self.entries.is_empty() {
            let empty_state = Paragraph::new(
                "No branches or worktrees available. Create a branch, then refresh.",
            )
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().title(" Branches ").borders(Borders::ALL));
            frame.render_widget(empty_state, branches_area);
        } else {
            frame.render_stateful_widget(table, branches_area, &mut self.table_state);
        }

        let footer_prefix = if self.message.is_empty() {
            String::new()
        } else {
            format!("{}  |  ", truncate_end(&self.message, terminal_width / 2))
        };
        let footer_text = if terminal_width < 80 {
            format!("{footer_prefix}[Return] open  [/] filter  [i] details  [h] help  [q] quit")
        } else if terminal_width < 130 {
            format!(
                "{footer_prefix}[Return] open  [a/x] add/remove worktree  [b/e/D] new/rename/delete branch  [d] diff  [/] filter  [i] details  [h] help  [r] refresh  [q] quit",
            )
        } else {
            format!(
                "{footer_prefix}[Return] open/checkout  [a/x] add/remove worktree  [b/e/D] new/rename/delete branch  [d] diff  [/] filter  [i] details  [h] help  [r] refresh  [q] quit",
            )
        };
        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::Gray))
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::TOP));
        frame.render_widget(footer, footer_area);

        fn truncate_end(value: &str, max_width: usize) -> String {
            if value.chars().count() <= max_width {
                return value.to_owned();
            }
            if max_width <= 3 {
                return ".".repeat(max_width);
            }
            format!(
                "{}...",
                value.chars().take(max_width - 3).collect::<String>()
            )
        }
        
        // Render details pane
        let details = self.selected().map_or_else(
            || vec![Line::from("No branch selected")],
            |entry| {
                let label_style = Style::default().fg(Color::Cyan);
                let value_style = Style::default().fg(Color::Gray);
                let content_width = details_area.width.saturating_sub(12) as usize;
                let branch_suffix = if entry.is_current { "  CURRENT" } else { "" };
                let state = if entry.is_unborn {
                    "UNBORN"
                } else if entry.is_detached {
                    "DETACHED"
                } else {
                    "BRANCH"
                };
                let changes = match entry.is_dirty {
                    Some(true) => "MODIFIED",
                    Some(false) => "CLEAN",
                    None => "-",
                };
                let ahead = entry
                    .ahead
                    .map_or(String::from("refresh"), |value| value.to_string());
                let behind = entry
                    .behind
                    .map_or(String::from("refresh"), |value| value.to_string());
                let sync = format!("{}  up {ahead} down {behind}", sync_summary(entry));
                vec![
                    Line::from(vec![
                        Span::styled("Branch    ", label_style),
                        Span::styled(format!("{}{}", entry.name, branch_suffix), value_style),
                    ]),
                    Line::from(vec![
                        Span::styled("Commit    ", label_style),
                        Span::styled(entry.commit_hash.clone(), value_style),
                    ]),
                    Line::from(vec![
                        Span::styled("Subject   ", label_style),
                        Span::styled(
                            truncate_end(&entry.commit_subject, content_width),
                            value_style,
                        ),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Worktree  ", label_style),
                        Span::styled(entry.worktree_path.as_deref().unwrap_or("-"), value_style),
                    ]),
                    Line::from(vec![
                        Span::styled("Changes   ", label_style),
                        Span::styled(changes, value_style),
                    ]),
                    Line::from(vec![
                        Span::styled("State     ", label_style),
                        Span::styled(state, value_style),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Upstream  ", label_style),
                        Span::styled(
                            truncate_end(entry.upstream.as_deref().unwrap_or("-"), content_width),
                            value_style,
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Sync      ", label_style),
                        Span::styled(truncate_end(&sync, content_width), value_style),
                    ]),
                ]
            },
        );
        let details = Paragraph::new(details)
            .wrap(Wrap { trim: false })
            .block(Block::default().title(" Details [i] ").borders(Borders::ALL));
        frame.render_widget(details, details_area);

        if let Some(action) = &self.pending_action {
            let confirmation_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Length(5),
                    Constraint::Percentage(40),
                ])
                .split(frame.area())[1];
            let confirmation_area = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(20),
                    Constraint::Min(40),
                    Constraint::Percentage(20),
                ])
                .split(confirmation_area)[1];
            let dialog_text = match action {
                PendingAction::Checkout(_) => {
                    "Current worktree has uncommitted changes.\n\nEnter: checkout anyway    Esc: cancel"
                }
                PendingAction::AddWorktree { path, .. } => {
                    return self.render_confirmation(
                        frame,
                        confirmation_area,
                        &format!("Create worktree at {path}?\n\nEnter: create    Esc: cancel"),
                    );
                }
                PendingAction::RemoveWorktree(entry) if entry.is_dirty == Some(true) => {
                    "Worktree has uncommitted changes. Remove it anyway?\n\nEnter: remove    Esc: cancel"
                }
                PendingAction::RemoveWorktree(_) => {
                    "Remove this worktree?\n\nEnter: remove    Esc: cancel"
                }
                PendingAction::CreateBranch { name, parent } => {
                    return self.render_confirmation(
                        frame,
                        confirmation_area,
                        &format!(
                            "Create branch {name} from {parent}?\n\nEnter: create    Esc: cancel"
                        ),
                    );
                }
                PendingAction::RenameBranch { old_name, new_name } => {
                    return self.render_confirmation(
                        frame,
                        confirmation_area,
                        &format!(
                            "Rename {old_name} to {new_name}?\n\nEnter: rename    Esc: cancel"
                        ),
                    );
                }
                PendingAction::DeleteBranch(name) => {
                    return self.render_confirmation(
                        frame,
                        confirmation_area,
                        &format!("Delete branch {name}?\n\nEnter: delete    Esc: cancel"),
                    );
                }
            };
            let dialog = Paragraph::new(dialog_text)
                .style(Style::default().fg(Color::Yellow))
                .block(
                    Block::default()
                        .title(" Confirm checkout ")
                        .borders(Borders::ALL),
                );
            frame.render_widget(Clear, confirmation_area);
            frame.render_widget(dialog, confirmation_area);
        }

        if let Some(input_mode) = &self.input_mode {
            let (label, value, action) = match input_mode {
                InputMode::AddWorktree { branch, path } => {
                    (
                        format!("New worktree for {}:", branch.name),
                        path.as_str(),
                        "Enter: create    Esc: cancel",
                    )
                }
                InputMode::CreateBranch { name, parent } => {
                    (
                        format!("New branch from {parent}:"),
                        name.as_str(),
                        "Enter: continue    Esc: cancel",
                    )
                }
                InputMode::RenameBranch { old_name, new_name } => {
                    (
                        format!("Rename {old_name} to:"),
                        new_name.as_str(),
                        "Enter: continue    Esc: cancel",
                    )
                }
            };
            let input_width = frame.area().width.saturating_sub(2).max(1);
            let label_lines = wrap_input_text(&label, input_width);
            let value_lines = wrap_input_text(value, input_width);
            let input_height = (label_lines.len() as u16)
                .saturating_add(value_lines.len() as u16)
                .saturating_add(4)
                .min(frame.area().height);
            let input_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Length(input_height),
                    Constraint::Percentage(40),
                ])
                .split(frame.area())[1];
            let text = format!(
                "{}\n{}\n\n{action}",
                label_lines.join("\n"),
                value_lines.join("\n"),
            );
            let dialog = Paragraph::new(text)
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().title(" Input ").borders(Borders::ALL));
            frame.render_widget(Clear, input_area);
            frame.render_widget(dialog, input_area);

            let cursor_line = label_lines.len() as u16 + value_lines.len() as u16 - 1;
            let cursor_column = value_lines
                .last()
                .map_or(0, |line| line.width() as u16);
            frame.set_cursor_position((
                input_area.x.saturating_add(1).saturating_add(cursor_column),
                input_area.y.saturating_add(1).saturating_add(cursor_line),
            ));
        }

        if let Some(diff) = &self.diff_view {
            let diff_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(10),
                    Constraint::Min(5),
                    Constraint::Percentage(10),
                ])
                .split(frame.area())[1];
            let diff = Paragraph::new(diff.as_str())
                .style(Style::default().fg(Color::White))
                .block(
                    Block::default()
                        .title(" Diff stat (Esc to return) ")
                        .borders(Borders::ALL),
                );
            frame.render_widget(Clear, diff_area);
            frame.render_widget(diff, diff_area);
        }

        if self.help_visible {
            let help_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(20),
                    Constraint::Min(14),
                    Constraint::Percentage(20),
                ])
                .split(frame.area())[1];
            let help_area = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(15),
                    Constraint::Min(54),
                    Constraint::Percentage(15),
                ])
                .split(help_area)[1];
            let visible_lines = help_area.height.saturating_sub(2) as usize;
            self.help_max_scroll = HELP_TEXT.lines().count().saturating_sub(visible_lines) as u16;
            self.help_scroll = self.help_scroll.min(self.help_max_scroll);
            let help = Paragraph::new(HELP_TEXT)
                .style(Style::default().fg(Color::White))
                .scroll((self.help_scroll, 0))
                .block(
                    Block::default()
                        .title(" Help (j/k: line, Space/b: page, Esc: close) ")
                        .borders(Borders::ALL),
                );
            frame.render_widget(Clear, help_area);
            frame.render_widget(help, help_area);
        }

        if self.details_dialog_visible {
            let details_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(15),
                    Constraint::Min(12),
                    Constraint::Percentage(15),
                ])
                .split(frame.area())[1];
            let details_area = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(10),
                    Constraint::Min(50),
                    Constraint::Percentage(10),
                ])
                .split(details_area)[1];
            
            // Calculate scroll limits
            let visible_lines = details_area.height.saturating_sub(2) as usize;
            let total_lines = 10usize;  // Fixed number of lines in the details view
            self.details_max_scroll = total_lines.saturating_sub(visible_lines) as u16;
            self.details_scroll = self.details_scroll.min(self.details_max_scroll);
            
            let details = self.selected().map_or_else(
                || vec![Line::from("No branch selected")],
                |entry| {
                    let label_style = Style::default().fg(Color::Cyan);
                    let value_style = Style::default().fg(Color::Gray);
                    let content_width = details_area.width.saturating_sub(12) as usize;
                    let branch_suffix = if entry.is_current { "  CURRENT" } else { "" };
                    let state = if entry.is_unborn {
                        "UNBORN"
                    } else if entry.is_detached {
                        "DETACHED"
                    } else {
                        "BRANCH"
                    };
                    let changes = match entry.is_dirty {
                        Some(true) => "MODIFIED",
                        Some(false) => "CLEAN",
                        None => "-",
                    };
                    let ahead = entry
                        .ahead
                        .map_or(String::from("refresh"), |value| value.to_string());
                    let behind = entry
                        .behind
                        .map_or(String::from("refresh"), |value| value.to_string());
                    let sync = format!("{}  up {ahead} down {behind}", sync_summary(entry));
                    vec![
                        Line::from(vec![
                            Span::styled("Branch    ", label_style),
                            Span::styled(format!("{}{}", entry.name, branch_suffix), value_style),
                        ]),
                        Line::from(vec![
                            Span::styled("Commit    ", label_style),
                            Span::styled(entry.commit_hash.clone(), value_style),
                        ]),
                        Line::from(vec![
                            Span::styled("Subject   ", label_style),
                            Span::styled(
                                truncate_end(&entry.commit_subject, content_width),
                                value_style,
                            ),
                        ]),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("Worktree  ", label_style),
                            Span::styled(entry.worktree_path.as_deref().unwrap_or("-"), value_style),
                        ]),
                        Line::from(vec![
                            Span::styled("Changes   ", label_style),
                            Span::styled(changes, value_style),
                        ]),
                        Line::from(vec![
                            Span::styled("State     ", label_style),
                            Span::styled(state, value_style),
                        ]),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("Upstream  ", label_style),
                            Span::styled(
                                truncate_end(entry.upstream.as_deref().unwrap_or("-"), content_width),
                                value_style,
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("Sync      ", label_style),
                            Span::styled(truncate_end(&sync, content_width), value_style),
                        ]),
                    ]
                },
            );
            let details = Paragraph::new(details)
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(" Details (j/k: line, Space/b: page, Esc: close) ")
                        .borders(Borders::ALL),
                )
                .scroll((self.details_scroll, 0));
            frame.render_widget(Clear, details_area);
            frame.render_widget(details, details_area);
        }
    }

    fn render_confirmation(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        text: &str,
    ) {
        let dialog = Paragraph::new(text.to_owned())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().title(" Confirm ").borders(Borders::ALL));
        frame.render_widget(Clear, area);
        frame.render_widget(dialog, area);
    }
}

fn wrap_input_text(text: &str, width: u16) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut line_width: u16 = 0;

    for character in text.chars() {
        let character_width = character.width().unwrap_or_default() as u16;
        if line_width > 0 && line_width.saturating_add(character_width) > width {
            lines.push(String::new());
            line_width = 0;
        }
        lines.last_mut().unwrap().push(character);
        line_width = line_width.saturating_add(character_width);
        if line_width == width {
            lines.push(String::new());
            line_width = 0;
        }
    }

    lines
}

fn truncate_start(value: &str, max_width: usize) -> String {
    if value.width() <= max_width {
        return value.to_owned();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }
    let mut suffix = String::new();
    let mut suffix_width = 0;
    for character in value.chars().rev() {
        let character_width = character.width().unwrap_or_default();
        if suffix_width + character_width > max_width - 3 {
            break;
        }
        suffix.insert(0, character);
        suffix_width += character_width;
    }
    format!("...{suffix}")
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let repository = gix::discover(".")?;
    let selected = match run_tui(&repository)? {
        TuiAction::Cancel => return Ok(()),
        TuiAction::Select(selected) => selected,
    };

    let destination = if let Some(path) = selected.worktree_path.as_deref() {
        path.to_owned()
    } else {
        repository_root()?
    };
    println!("{destination}");
    Ok(())
}

fn repository_root() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if message.is_empty() {
            String::from("git rev-parse --show-toplevel failed")
        } else {
            message
        }
        .into());
    }
    let path = String::from_utf8(output.stdout)?.trim().to_owned();
    if path.is_empty() {
        return Err("git rev-parse --show-toplevel returned an empty path".into());
    }
    Ok(path)
}

fn load_entries(
    repository: &gix::Repository,
    include_sync_status: bool,
) -> Result<Vec<BranchEntry>, Box<dyn std::error::Error + Send + Sync>> {
    let current_branch = repository.head_name()?;
    let default_branch = if include_sync_status {
        default_branch()?
    } else {
        None
    };
    let mut worktree_entries = Vec::new();
    for (branch, path) in worktree_list()? {
        let is_dirty = is_worktree_dirty(Path::new(&path))?;
        let (commit_hash, commit_subject) = worktree_commit(&path)?;
        worktree_entries.push((branch, path, is_dirty, commit_hash, commit_subject));
    }
    let mut entries = Vec::new();
    for branch in repository.references()?.local_branches()? {
        let branch = branch?;
        let name = branch.name();
        let short_name = name.shorten().to_string();
        let worktree = worktree_entries
            .iter()
            .find(|(branch_name, _, _, _, _)| branch_name.as_deref() == Some(short_name.as_str()))
            .map(|(_, path, is_dirty, _, _)| (path.clone(), *is_dirty));
        let is_current = current_branch
            .as_ref()
            .is_some_and(|current| current.as_ref() == name);
        let (commit_hash, commit_subject) = branch_commit(&short_name)?;
        let (upstream, ahead, behind, unpublished_commits) = if include_sync_status {
            branch_sync_status(&short_name, default_branch.as_deref())?
        } else {
            (None, None, None, None)
        };
        entries.push(BranchEntry {
            name: short_name,
            is_current,
            worktree_path: worktree.as_ref().map(|(path, _)| path.clone()),
            is_dirty: worktree.map(|(_, is_dirty)| is_dirty),
            commit_hash,
            commit_subject,
            is_detached: false,
            is_unborn: false,
            upstream,
            ahead,
            behind,
            unpublished_commits,
        });
    }
    for (branch, path, is_dirty, commit_hash, commit_subject) in worktree_entries {
        if branch.is_none() {
            let is_unborn = commit_hash == "-";
            entries.push(BranchEntry {
                name: if is_unborn {
                    String::from("UNBORN HEAD")
                } else {
                    format!("DETACHED ({commit_hash})")
                },
                is_current: current_branch.is_none() && path == repository_root()?,
                worktree_path: Some(path),
                is_dirty: Some(is_dirty),
                commit_hash,
                commit_subject,
                is_detached: !is_unborn,
                is_unborn,
                upstream: None,
                ahead: None,
                behind: None,
                unpublished_commits: None,
            });
        }
    }
    Ok(entries)
}

fn worktree_list() -> Result<Vec<(Option<String>, String)>, Box<dyn std::error::Error + Send + Sync>>
{
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr)
            .trim()
            .to_owned()
            .into());
    }
    let output = String::from_utf8(output.stdout)?;
    parse_worktree_list(&output).map_err(Into::into)
}

fn parse_worktree_list(output: &str) -> Result<Vec<(Option<String>, String)>, String> {
    output
        .split("\n\n")
        .filter(|record| !record.trim().is_empty())
        .map(|record| {
            let path = record
                .lines()
                .find_map(|line| line.strip_prefix("worktree "))
                .ok_or("worktree entry is missing its path")?
                .to_owned();
            let branch = record
                .lines()
                .find_map(|line| line.strip_prefix("branch refs/heads/"))
                .map(str::to_owned);
            Ok((branch, path))
        })
        .collect()
}

fn branch_sync_status(
    branch: &str,
    default_branch: Option<&str>,
) -> Result<
    (Option<String>, Option<usize>, Option<usize>, Option<usize>),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let upstream_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name"])
        .arg(format!("{branch}@{{upstream}}"))
        .output()?;
    if !upstream_output.status.success() {
        return Ok((
            None,
            Some(0),
            Some(0),
            default_branch
                .map(|base| unpublished_commit_count(branch, base))
                .transpose()?,
        ));
    }
    let upstream = String::from_utf8(upstream_output.stdout)?.trim().to_owned();
    if upstream.is_empty() {
        return Ok((
            None,
            Some(0),
            Some(0),
            default_branch
                .map(|base| unpublished_commit_count(branch, base))
                .transpose()?,
        ));
    }
    let count_output = Command::new("git")
        .args(["rev-list", "--left-right", "--count"])
        .arg(format!("{branch}...{upstream}"))
        .output()?;
    if !count_output.status.success() {
        return Err(format!("git rev-list failed for {branch}").into());
    }
    let counts = String::from_utf8(count_output.stdout)?;
    let (ahead, behind) = parse_ahead_behind(&counts).ok_or("invalid git rev-list output")?;
    Ok((Some(upstream), Some(ahead), Some(behind), None))
}

fn default_branch() -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let output = Command::new("git")
        .args([
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let branch = String::from_utf8(output.stdout)?.trim().to_owned();
    Ok((!branch.is_empty()).then_some(branch))
}

fn unpublished_commit_count(
    branch: &str,
    default_branch: &str,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let output = Command::new("git")
        .args(["rev-list", "--count"])
        .arg(format!("{default_branch}..{branch}"))
        .output()?;
    if !output.status.success() {
        return Err(format!("git rev-list failed for {branch}").into());
    }
    String::from_utf8(output.stdout)?
        .trim()
        .parse()
        .map_err(|_| "invalid git rev-list output".into())
}

fn parse_ahead_behind(value: &str) -> Option<(usize, usize)> {
    let mut counts = value.split_whitespace();
    let ahead = counts.next()?.parse().ok()?;
    let behind = counts.next()?.parse().ok()?;
    counts.next().is_none().then_some((ahead, behind))
}

fn sync_summary(entry: &BranchEntry) -> String {
    match (entry.ahead, entry.behind) {
        (Some(0), Some(0)) if entry.upstream.is_some() => String::from("UP TO DATE"),
        (Some(ahead), Some(0)) if entry.upstream.is_some() => format!("PUSH +{ahead}"),
        (Some(0), Some(behind)) if entry.upstream.is_some() => format!("PULL -{behind}"),
        (Some(ahead), Some(behind)) if entry.upstream.is_some() => {
            format!("PUSH/PULL +{ahead}/-{behind}")
        }
        (Some(_), Some(_)) => String::from("NO UPSTREAM"),
        _ => String::from("REFRESH"),
    }
}

fn status_markers(entry: &BranchEntry) -> String {
    let mut markers = String::new();
    if entry.is_dirty == Some(true) {
        markers.push('*');
    }
    if let Some(behind) = entry.behind.filter(|behind| *behind > 0) {
        markers.push_str(&format!("\u{2193}{behind}"));
    }
    if let Some(ahead) = entry.ahead.filter(|ahead| *ahead > 0) {
        markers.push_str(&format!("\u{2191}{ahead}"));
    }
    if entry.upstream.is_none() && entry.unpublished_commits.is_some_and(|commits| commits > 0) {
        markers.push_str(&format!(
            "+{}",
            entry.unpublished_commits.unwrap_or_default()
        ));
    }
    markers
}

fn worktree_commit(
    path: &str,
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["log", "-1", "--format=%h%x00%s"])
        .output()?;
    if !output.status.success() {
        return Ok((String::from("-"), String::from("No commits yet")));
    }
    let output = String::from_utf8(output.stdout)?;
    let (hash, subject) = output.trim_end().split_once('\0').unwrap_or(("-", "-"));
    Ok((hash.to_owned(), subject.to_owned()))
}

fn branch_commit(name: &str) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%h%x00%s", name])
        .output()?;
    if !output.status.success() {
        return Err(format!("git log failed for {name}").into());
    }
    let output = String::from_utf8(output.stdout)?;
    let (hash, subject) = output.trim_end().split_once('\0').unwrap_or(("-", "-"));
    Ok((hash.to_owned(), subject.to_owned()))
}

fn is_worktree_dirty(path: &Path) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err(format!("git status failed for {}", path.display()).into());
    }
    Ok(!output.stdout.is_empty())
}

fn run_tui(
    repository: &gix::Repository,
) -> Result<TuiAction, Box<dyn std::error::Error + Send + Sync>> {
    let entries = load_entries(repository, true)?;
    let _terminal_guard = TerminalGuard::activate()?;
    let backend = CrosstermBackend::new(stderr());
    let mut terminal = Terminal::new(backend)?;
    run_event_loop(&mut terminal, repository, entries)
}

fn run_event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    repository: &gix::Repository,
    entries: Vec<BranchEntry>,
) -> Result<TuiAction, Box<dyn std::error::Error + Send + Sync>> {
    let mut app = TuiApp::new(entries);
    loop {
        terminal.draw(|frame| app.render(frame))?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = event::read()?
        else {
            continue;
        };
        if app.diff_view.is_some() {
            if code == KeyCode::Esc {
                app.diff_view = None;
                app.message = String::new();
            }
            continue;
        }
        if app.help_visible {
            match code {
                KeyCode::Esc | KeyCode::Char('h') => {
                    app.help_visible = false;
                    app.help_scroll = 0;
                    app.message = String::new();
                }
                KeyCode::Up | KeyCode::Char('k') => app.scroll_help(-1),
                KeyCode::Down | KeyCode::Char('j') => app.scroll_help(1),
                KeyCode::PageUp | KeyCode::Char('b') => app.scroll_help(-5),
                KeyCode::PageDown | KeyCode::Char(' ') => app.scroll_help(5),
                _ => {}
            }
            continue;
        }
        if app.details_dialog_visible {
            match code {
                KeyCode::Esc | KeyCode::Char('i') => {
                    app.details_dialog_visible = false;
                    app.details_scroll = 0;
                    app.message = String::new();
                }
                KeyCode::Up | KeyCode::Char('k') => app.scroll_details(-1),
                KeyCode::Down | KeyCode::Char('j') => app.scroll_details(1),
                KeyCode::PageUp | KeyCode::Char('b') => app.scroll_details(-5),
                KeyCode::PageDown | KeyCode::Char(' ') => app.scroll_details(5),
                _ => {}
            }
            continue;
        }
        if app.filtering {
            match code {
                KeyCode::Esc => {
                    app.filtering = false;
                    app.update_filter(None);
                    app.message = String::from("Filter cleared");
                }
                KeyCode::Enter => {
                    app.filtering = false;
                    app.message = format!("{} branches", app.entries.len());
                }
                KeyCode::Backspace => {
                    let mut filter = app.filter.take().unwrap_or_default();
                    filter.pop();
                    app.update_filter(Some(filter));
                }
                KeyCode::Char(character) => {
                    let mut filter = app.filter.take().unwrap_or_default();
                    filter.push(character);
                    app.update_filter(Some(filter));
                }
                _ => {}
            }
            continue;
        }
        if let Some(input_mode) = app.input_mode.take() {
            match (input_mode, code) {
                (_, KeyCode::Esc) => app.message = String::from("Operation cancelled"),
                (InputMode::AddWorktree { branch, path }, KeyCode::Enter) if !path.is_empty() => {
                    execute_action_and_reload(
                        &mut app,
                        repository,
                        PendingAction::AddWorktree { branch, path },
                    );
                }
                (InputMode::CreateBranch { name, parent }, KeyCode::Enter) if !name.is_empty() => {
                    execute_action_and_reload(
                        &mut app,
                        repository,
                        PendingAction::CreateBranch { name, parent },
                    );
                }
                (InputMode::RenameBranch { old_name, new_name }, KeyCode::Enter)
                    if !new_name.is_empty() =>
                {
                    execute_action_and_reload(
                        &mut app,
                        repository,
                        PendingAction::RenameBranch { old_name, new_name },
                    );
                }
                (InputMode::CreateBranch { mut name, parent }, KeyCode::Backspace) => {
                    name.pop();
                    app.input_mode = Some(InputMode::CreateBranch { name, parent });
                }
                (
                    InputMode::RenameBranch {
                        old_name,
                        mut new_name,
                    },
                    KeyCode::Backspace,
                ) => {
                    new_name.pop();
                    app.input_mode = Some(InputMode::RenameBranch { old_name, new_name });
                }
                (InputMode::AddWorktree { branch, mut path }, KeyCode::Backspace) => {
                    path.pop();
                    app.input_mode = Some(InputMode::AddWorktree { branch, path });
                }
                (InputMode::AddWorktree { branch, mut path }, KeyCode::Char(character)) => {
                    path.push(character);
                    app.input_mode = Some(InputMode::AddWorktree { branch, path });
                }
                (InputMode::CreateBranch { mut name, parent }, KeyCode::Char(character)) => {
                    name.push(character);
                    app.input_mode = Some(InputMode::CreateBranch { name, parent });
                }
                (
                    InputMode::RenameBranch {
                        old_name,
                        mut new_name,
                    },
                    KeyCode::Char(character),
                ) => {
                    new_name.push(character);
                    app.input_mode = Some(InputMode::RenameBranch { old_name, new_name });
                }
                (input_mode, _) => app.input_mode = Some(input_mode),
            }
            continue;
        }
        if let Some(action) = app.pending_action.take() {
            match code {
                KeyCode::Esc => app.message = String::from("Operation cancelled"),
                KeyCode::Enter => match execute_pending_action(&action) {
                    Ok(()) => match load_entries(repository, true) {
                        Ok(entries) => {
                            app.reload(entries);
                            app.message = String::from("Operation completed");
                        }
                        Err(error) => app.message = format!("Reload failed: {error}"),
                    },
                    Err(error) => app.message = format!("Operation failed: {error}"),
                },
                _ => app.pending_action = Some(action),
            }
            continue;
        }
        match (code, modifiers) {
            (KeyCode::Char('q'), KeyModifiers::NONE) => {
                return Ok(TuiAction::Cancel);
            }
            (KeyCode::Esc, KeyModifiers::NONE) => return Ok(TuiAction::Cancel),
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => app.move_selection(-1),
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => app.move_selection(1),
            (KeyCode::Enter, _) => {
                if let Some(selected) = app.selected().cloned() {
                    let should_confirm = selected.worktree_path.is_none()
                        && app
                            .entries
                            .iter()
                            .any(|entry| entry.is_current && entry.is_dirty == Some(true));
                    if should_confirm {
                        app.pending_action = Some(PendingAction::Checkout(selected));
                        app.message = String::from("Confirm checkout with uncommitted changes");
                    } else if selected.worktree_path.is_some() {
                        return Ok(TuiAction::Select(selected));
                    } else {
                        match checkout_branch(&selected.name) {
                            Ok(()) => return Ok(TuiAction::Select(selected)),
                            Err(error) => app.message = format!("Checkout failed: {error}"),
                        }
                    }
                }
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                app.message = String::from("Reloading branches...");
                match load_entries(repository, true) {
                    Ok(entries) => {
                        app.reload(entries);
                        app.message = String::from("Branches reloaded");
                    }
                    Err(error) => app.message = format!("Reload failed: {error}"),
                }
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                if let Some(selected) = app.selected() {
                    match diff_stat(selected) {
                        Ok(diff) => app.diff_view = Some(diff),
                        Err(error) => app.message = format!("Diff failed: {error}"),
                    }
                }
            }
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                app.filtering = true;
                app.update_filter(Some(String::new()));
                app.message = String::from("Filter branches");
            }
            (KeyCode::Char('a'), KeyModifiers::NONE) => {
                if let Some(selected) = app.selected().cloned() {
                    if selected.worktree_path.is_some() {
                        app.message = String::from("Branch already has a worktree");
                    } else {
                        let path = default_worktree_path(&selected.name)?;
                        app.input_mode = Some(InputMode::AddWorktree {
                            branch: selected,
                            path,
                        });
                    }
                }
            }
            (KeyCode::Char('x'), KeyModifiers::NONE) => {
                if let Some(selected) = app.selected().cloned() {
                    if selected.is_current {
                        app.message = String::from("Cannot remove the current worktree");
                    } else if selected.worktree_path.is_some() {
                        app.pending_action = Some(PendingAction::RemoveWorktree(selected));
                    } else {
                        app.message = String::from("Selected branch has no worktree");
                    }
                }
            }
            (KeyCode::Char('b'), KeyModifiers::NONE) => {
                if let Some(selected) = app.selected() {
                    if selected.is_unborn {
                        app.message = String::from("Cannot create a branch from an unborn HEAD");
                    } else {
                        app.input_mode = Some(InputMode::CreateBranch {
                            name: String::new(),
                            parent: selected.name.clone(),
                        });
                    }
                }
            }
            (KeyCode::Char('e'), KeyModifiers::NONE) => {
                if let Some(selected) = app.selected() {
                    if selected.is_detached || selected.is_unborn {
                        app.message = String::from("Cannot rename a detached or unborn HEAD");
                    } else {
                        app.input_mode = Some(InputMode::RenameBranch {
                            old_name: selected.name.clone(),
                            new_name: String::new(),
                        });
                    }
                }
            }
            (KeyCode::Char('D'), KeyModifiers::SHIFT) => {
                if let Some(selected) = app.selected() {
                    if selected.is_current || selected.worktree_path.is_some() {
                        app.message = String::from("Cannot delete a current or worktree branch");
                    } else {
                        app.pending_action =
                            Some(PendingAction::DeleteBranch(selected.name.clone()));
                    }
                }
            }
            (KeyCode::Char('h'), KeyModifiers::NONE) => {
                app.help_scroll = 0;
                app.help_visible = true;
            }
            (KeyCode::Char('i'), KeyModifiers::NONE) => {
                app.details_dialog_visible = true;
                app.details_scroll = 0;
            }
            _ => {}
        }
    }
}

fn default_worktree_path(branch: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let root_path = repository_root()?;
    let root = Path::new(&root_path);
    let parent = root.parent().ok_or("repository has no parent directory")?;
    let repository_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("invalid repository name")?;
    Ok(parent
        .join(format!("{repository_name}-{}", branch.replace('/', "-")))
        .display()
        .to_string())
}

fn execute_action_and_reload(
    app: &mut TuiApp,
    repository: &gix::Repository,
    action: PendingAction,
) {
    match execute_pending_action(&action) {
        Ok(()) => match load_entries(repository, true) {
            Ok(entries) => {
                app.reload(entries);
                app.message = String::from("Operation completed");
            }
            Err(error) => app.message = format!("Reload failed: {error}"),
        },
        Err(error) => app.message = format!("Operation failed: {error}"),
    }
}

fn execute_pending_action(action: &PendingAction) -> Result<(), String> {
    let mut command = Command::new("git");
    match action {
        PendingAction::Checkout(entry) => command.args(["checkout", &entry.name]),
        PendingAction::AddWorktree { branch, path } => {
            command.args(["worktree", "add", path, &branch.name])
        }
        PendingAction::RemoveWorktree(entry) => command.args([
            "worktree",
            "remove",
            entry
                .worktree_path
                .as_deref()
                .ok_or("missing worktree path")?,
        ]),
        PendingAction::CreateBranch { name, parent } => command.args(["branch", name, parent]),
        PendingAction::RenameBranch { old_name, new_name } => {
            command.args(["branch", "-m", old_name, new_name])
        }
        PendingAction::DeleteBranch(name) => command.args(["branch", "-d", name]),
    };
    let output = command.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

fn diff_stat(entry: &BranchEntry) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut command = Command::new("git");
    if let Some(path) = entry.worktree_path.as_deref() {
        command.arg("-C").arg(path).args(["diff", "--stat"]);
    } else {
        command.args(["diff", "--stat", &entry.name]);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr)
            .trim()
            .to_owned()
            .into());
    }
    let diff = String::from_utf8(output.stdout)?;
    Ok(if diff.trim().is_empty() {
        String::from("No changes")
    } else {
        diff
    })
}

fn checkout_branch(name: &str) -> Result<(), String> {
    let output = Command::new("git")
        .arg("checkout")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if message.is_empty() {
        format!("git checkout {name} failed")
    } else {
        message
    })
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

    #[test]
    fn parses_ahead_and_behind_counts() {
        assert_eq!(parse_ahead_behind("3\t2\n"), Some((3, 2)));
        assert_eq!(parse_ahead_behind("3"), None);
        assert_eq!(parse_ahead_behind("3 2 1"), None);
    }

    #[test]
    fn parses_primary_linked_and_detached_worktrees() {
        let entries = parse_worktree_list(
            "worktree /project\nHEAD abc123\nbranch refs/heads/main\n\nworktree /project-feature\nHEAD def456\nbranch refs/heads/feature/test\n\nworktree /project-detached\nHEAD fedcba\ndetached\n",
        )
        .unwrap();
        assert_eq!(
            entries,
            vec![
                (Some(String::from("main")), String::from("/project")),
                (
                    Some(String::from("feature/test")),
                    String::from("/project-feature"),
                ),
                (None, String::from("/project-detached")),
            ]
        );
    }

    #[test]
    fn summarizes_push_and_pull_requirements() {
        let mut branch = entry("feature", false, None);
        branch.upstream = Some(String::from("origin/feature"));
        branch.ahead = Some(2);
        branch.behind = Some(1);
        assert_eq!(sync_summary(&branch), "PUSH/PULL +2/-1");

        branch.ahead = Some(0);
        assert_eq!(sync_summary(&branch), "PULL -1");
    }

    #[test]
    fn status_markers_only_show_required_actions() {
        let mut branch = entry("feature", false, Some(true));
        assert_eq!(status_markers(&branch), "*");

        branch.is_dirty = Some(false);
        branch.upstream = Some(String::from("origin/feature"));
        branch.ahead = Some(2);
        branch.behind = Some(1);
        assert_eq!(status_markers(&branch), "\u{2193}1\u{2191}2");

        branch.ahead = Some(0);
        branch.behind = Some(0);
        assert_eq!(status_markers(&branch), "");

        branch.upstream = None;
        branch.unpublished_commits = Some(3);
        assert_eq!(status_markers(&branch), "+3");
    }

    #[test]
    fn truncates_from_start_to_preserve_suffix() {
        assert_eq!(truncate_start("short", 5), "short");
        assert_eq!(
            truncate_start("feature/very-long-ticket-123", 13),
            "...ticket-123"
        );
        assert_eq!(truncate_start("feature/長い名前", 9), "...い名前");
        assert_eq!(truncate_start("feature", 3), "...");
    }
}

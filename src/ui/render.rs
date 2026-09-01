use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::constants::{
    DETAILS_PANE_TOTAL_LINES, ELLIPSIS, ELLIPSIS_WIDTH, FOOTER_COMPACT_BREAKPOINT_WIDTH,
    FOOTER_MEDIUM_BREAKPOINT_WIDTH, HELP_TEXT, HIGHLIGHT_SYMBOL,
};
use super::layout::{
    TableLayout, calculate_confirmation_area, calculate_details_dialog_area, calculate_diff_area,
    calculate_help_area, calculate_input_area, calculate_main_layout, calculate_table_layout,
};
use crate::app::state::{BranchEntry, InputMode, PendingAction, TuiApp};

pub fn truncate_end(value: &str, max_width: usize) -> String {
    if value.chars().count() <= max_width {
        return value.to_owned();
    }
    if max_width <= ELLIPSIS_WIDTH {
        return ".".repeat(max_width);
    }
    format!(
        "{}{ELLIPSIS}",
        value
            .chars()
            .take(max_width - ELLIPSIS_WIDTH)
            .collect::<String>()
    )
}

pub fn truncate_start(value: &str, max_width: usize) -> String {
    if value.width() <= max_width {
        return value.to_owned();
    }
    if max_width <= ELLIPSIS_WIDTH {
        return ".".repeat(max_width);
    }
    let mut suffix = String::new();
    let mut suffix_width = 0;
    for character in value.chars().rev() {
        let character_width = character.width().unwrap_or_default();
        if suffix_width + character_width > max_width - ELLIPSIS_WIDTH {
            break;
        }
        suffix.insert(0, character);
        suffix_width += character_width;
    }
    format!("{ELLIPSIS}{suffix}")
}

pub fn wrap_input_text(text: &str, width: u16) -> Vec<String> {
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

pub fn sync_summary(entry: &BranchEntry) -> String {
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

pub fn status_markers(entry: &BranchEntry) -> String {
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

pub fn build_details_lines<'a>(
    entry: Option<&'a BranchEntry>,
    content_width: usize,
) -> Vec<Line<'a>> {
    entry.map_or_else(
        || vec![Line::from("No branch selected")],
        |entry| {
            let label_style = Style::default().fg(Color::Cyan);
            let value_style = Style::default().fg(Color::Gray);
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
    )
}

pub fn render_filter(
    frame: &mut Frame,
    area: Rect,
    filter: Option<&str>,
    filtering: bool,
    match_count: usize,
    total_count: usize,
    terminal_width: usize,
) {
    let filter_text = match filter {
        Some(filter) => {
            let query_width = terminal_width.saturating_sub(28).max(8);
            let cursor = if filtering { "_" } else { "" };
            format!(
                "/ {}{cursor}  {} of {} matches",
                truncate_end(filter, query_width),
                match_count,
                total_count,
            )
        }
        None => format!(
            "/  Filter branches and worktrees  {} branches",
            total_count,
        ),
    };
    let filter_style = if filtering {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let filter_widget = Paragraph::new(filter_text)
        .style(filter_style)
        .block(Block::default().title(" Filter ").borders(Borders::ALL));
    frame.render_widget(filter_widget, area);
}

pub fn render_table(
    frame: &mut Frame,
    area: Rect,
    entries: &[BranchEntry],
    table_state: &mut TableState,
    table_layout: &TableLayout,
) {
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
    let rows = entries.iter().map(|entry| {
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
                truncate_start(&entry.name, table_layout.branch_width)
            )),
            Cell::from(status_markers(entry)),
            Cell::from(truncate_start(
                entry.worktree_path.as_deref().unwrap_or("-"),
                table_layout.worktree_width,
            )),
        ])
        .style(Style::default().fg(color))
    });
    let table = Table::new(rows, table_layout.table_columns)
        .header(header)
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(HIGHLIGHT_SYMBOL)
        .block(Block::default().title(" Branches ").borders(Borders::ALL));
    if entries.is_empty() {
        let empty_state =
            Paragraph::new("No branches or worktrees available. Create a branch, then refresh.")
                .style(Style::default().fg(Color::Gray))
                .block(Block::default().title(" Branches ").borders(Borders::ALL));
        frame.render_widget(empty_state, area);
    } else {
        frame.render_stateful_widget(table, area, table_state);
    }
}

pub fn render_footer(frame: &mut Frame, area: Rect, message: &str, terminal_width: usize) {
    let footer_prefix = if message.is_empty() {
        String::new()
    } else {
        format!("{}  |  ", truncate_end(message, terminal_width / 2))
    };
    let footer_text = if frame.area().width < FOOTER_COMPACT_BREAKPOINT_WIDTH {
        format!("{footer_prefix}[Return] open  [/] filter  [i] details  [h] help  [q] quit")
    } else if frame.area().width < FOOTER_MEDIUM_BREAKPOINT_WIDTH {
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
    frame.render_widget(footer, area);
}

pub fn render_details_pane(frame: &mut Frame, area: Rect, selected: Option<&BranchEntry>) {
    let content_width = area.width.saturating_sub(12) as usize;
    let details = build_details_lines(selected, content_width);
    let details_widget = Paragraph::new(details).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(" Details [i] ")
            .borders(Borders::ALL),
    );
    frame.render_widget(details_widget, area);
}

pub fn render_confirmation(frame: &mut Frame, area: Rect, text: &str) {
    let dialog = Paragraph::new(text.to_owned())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().title(" Confirm ").borders(Borders::ALL));
    frame.render_widget(Clear, area);
    frame.render_widget(dialog, area);
}

pub fn render_pending_action(frame: &mut Frame, area: Rect, action: &PendingAction) {
    let dialog_text = match action {
        PendingAction::Checkout(_) => {
            "Current worktree has uncommitted changes.\n\nEnter: checkout anyway    Esc: cancel"
        }
        PendingAction::AddWorktree { path, .. } => {
            return render_confirmation(
                frame,
                area,
                &format!("Create worktree at {path}?\n\nEnter: create    Esc: cancel"),
            );
        }
        PendingAction::RemoveWorktree(entry) if entry.is_dirty == Some(true) => {
            "Worktree has uncommitted changes. Remove it anyway?\n\nEnter: remove    Esc: cancel"
        }
        PendingAction::RemoveWorktree(_) => "Remove this worktree?\n\nEnter: remove    Esc: cancel",
        PendingAction::CreateBranch { name, parent } => {
            return render_confirmation(
                frame,
                area,
                &format!("Create branch {name} from {parent}?\n\nEnter: create    Esc: cancel"),
            );
        }
        PendingAction::RenameBranch { old_name, new_name } => {
            return render_confirmation(
                frame,
                area,
                &format!("Rename {old_name} to {new_name}?\n\nEnter: rename    Esc: cancel"),
            );
        }
        PendingAction::DeleteBranch(name) => {
            return render_confirmation(
                frame,
                area,
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
    frame.render_widget(Clear, area);
    frame.render_widget(dialog, area);
}

pub fn render_input(
    frame: &mut Frame,
    area: Rect,
    input_mode: &InputMode,
    terminal_width: usize,
    terminal_height: u16,
) {
    let (label, value, action) = match input_mode {
        InputMode::AddWorktree { branch, path } => (
            format!("New worktree for {}:", branch.name),
            path.as_str(),
            "Enter: create    Esc: cancel",
        ),
        InputMode::CreateBranch { name, parent } => (
            format!("New branch from {parent}:"),
            name.as_str(),
            "Enter: continue    Esc: cancel",
        ),
        InputMode::RenameBranch { old_name, new_name } => (
            format!("Rename {old_name} to:"),
            new_name.as_str(),
            "Enter: continue    Esc: cancel",
        ),
    };
    let input_width = (terminal_width as u16).saturating_sub(2).max(1);
    let label_lines = wrap_input_text(&label, input_width);
    let value_lines = wrap_input_text(value, input_width);
    let input_height = (label_lines.len() as u16)
        .saturating_add(value_lines.len() as u16)
        .saturating_add(4)
        .min(terminal_height);
    let input_area = calculate_input_area(area, input_height);
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
    let cursor_column = value_lines.last().map_or(0, |line| line.width() as u16);
    frame.set_cursor_position((
        input_area.x.saturating_add(1).saturating_add(cursor_column),
        input_area.y.saturating_add(1).saturating_add(cursor_line),
    ));
}

pub fn render_diff(frame: &mut Frame, area: Rect, diff: &str) {
    let diff_widget = Paragraph::new(diff)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(" Diff stat (Esc to return) ")
                .borders(Borders::ALL),
        );
    frame.render_widget(Clear, area);
    frame.render_widget(diff_widget, area);
}

pub fn render_help(frame: &mut Frame, area: Rect, scroll: u16) {
    let help_widget = Paragraph::new(HELP_TEXT)
        .style(Style::default().fg(Color::White))
        .scroll((scroll, 0))
        .block(
            Block::default()
                .title(" Help (j/k: line, Space/b: page, Esc: close) ")
                .borders(Borders::ALL),
        );
    frame.render_widget(Clear, area);
    frame.render_widget(help_widget, area);
}

pub fn render_details_dialog(
    frame: &mut Frame,
    area: Rect,
    selected: Option<&BranchEntry>,
    scroll: u16,
) {
    let content_width = area.width.saturating_sub(12) as usize;
    let details = build_details_lines(selected, content_width);
    let details_widget = Paragraph::new(details)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title(" Details (j/k: line, Space/b: page, Esc: close) ")
                .borders(Borders::ALL),
        )
        .scroll((scroll, 0));
    frame.render_widget(Clear, area);
    frame.render_widget(details_widget, area);
}

pub fn render_app(frame: &mut Frame, app: &mut TuiApp) {
    let terminal_width = frame.area().width as usize;
    let main_layout = calculate_main_layout(frame.area());
    let table_layout = calculate_table_layout(main_layout.branches);

    render_filter(
        frame,
        main_layout.filter,
        app.filter.as_deref(),
        app.filtering,
        app.entries.len(),
        app.all_entries.len(),
        terminal_width,
    );

    render_table(
        frame,
        main_layout.branches,
        &app.entries,
        &mut app.table_state,
        &table_layout,
    );

    render_footer(frame, main_layout.footer, &app.message, terminal_width);

    render_details_pane(frame, main_layout.details, app.selected());

    if let Some(action) = &app.pending_action {
        let confirmation_area = calculate_confirmation_area(frame.area());
        render_pending_action(frame, confirmation_area, action);
    }

    if let Some(input_mode) = &app.input_mode {
        render_input(
            frame,
            frame.area(),
            input_mode,
            terminal_width,
            frame.area().height,
        );
    }

    if let Some(diff) = &app.diff_view {
        let diff_area = calculate_diff_area(frame.area());
        render_diff(frame, diff_area, diff);
    }

    if app.help_visible {
        let help_area = calculate_help_area(frame.area());
        let visible_lines = help_area.height.saturating_sub(2) as usize;
        app.help_max_scroll = HELP_TEXT.lines().count().saturating_sub(visible_lines) as u16;
        app.help_scroll = app.help_scroll.min(app.help_max_scroll);
        render_help(frame, help_area, app.help_scroll);
    }

    if app.details_dialog_visible {
        let details_area = calculate_details_dialog_area(frame.area());
        let visible_lines = details_area.height.saturating_sub(2) as usize;
        let total_lines = DETAILS_PANE_TOTAL_LINES;
        app.details_max_scroll = total_lines.saturating_sub(visible_lines) as u16;
        app.details_scroll = app.details_scroll.min(app.details_max_scroll);
        render_details_dialog(frame, details_area, app.selected(), app.details_scroll);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry() -> BranchEntry {
        BranchEntry {
            name: "feature/test".to_owned(),
            is_current: true,
            worktree_path: Some("/tmp/repo".to_owned()),
            is_dirty: Some(false),
            commit_hash: "abc1234".to_owned(),
            commit_subject: "Commit subject".to_owned(),
            is_detached: false,
            is_unborn: false,
            upstream: Some("origin/feature/test".to_owned()),
            ahead: Some(1),
            behind: Some(2),
            unpublished_commits: None,
        }
    }

    #[test]
    fn details_lines_render_all_fields() {
        let entry = test_entry();
        let lines = build_details_lines(Some(&entry), 50);
        assert_eq!(lines.len(), 10);
    }

    #[test]
    fn details_lines_render_none() {
        let lines = build_details_lines(None, 50);
        assert_eq!(lines.len(), 1);
    }
}

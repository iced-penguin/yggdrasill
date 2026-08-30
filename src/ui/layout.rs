use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders};
use unicode_width::UnicodeWidthStr;

use super::constants::{
    BRANCH_COLUMN_PERCENT, FILTER_BAR_HEIGHT, FOOTER_BAR_HEIGHT, HIGHLIGHT_SYMBOL, MAIN_MIN_HEIGHT,
    STATUS_COLUMN_WIDTH, WIDE_LAYOUT_BREAKPOINT_WIDTH, WORKTREE_COLUMN_MIN_WIDTH,
};

pub struct MainLayout {
    pub filter: Rect,
    pub branches: Rect,
    pub details: Rect,
    pub footer: Rect,
}

pub struct TableLayout {
    pub branch_width: usize,
    pub worktree_width: usize,
    pub table_columns: [Constraint; 3],
}

pub fn calculate_main_layout(area: Rect) -> MainLayout {
    let top_level = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(FILTER_BAR_HEIGHT),
            Constraint::Min(MAIN_MIN_HEIGHT),
        ])
        .split(area);

    let (branches, details, footer) = if area.width >= WIDE_LAYOUT_BREAKPOINT_WIDTH {
        let main_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
            .split(top_level[1]);
        let left_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(MAIN_MIN_HEIGHT),
                Constraint::Length(FOOTER_BAR_HEIGHT),
            ])
            .split(main_areas[0]);
        (left_areas[0], main_areas[1], left_areas[1])
    } else {
        let main_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(62),
                Constraint::Percentage(38),
                Constraint::Length(FOOTER_BAR_HEIGHT),
            ])
            .split(top_level[1]);
        (main_areas[0], main_areas[1], main_areas[2])
    };

    MainLayout {
        filter: top_level[0],
        branches,
        details,
        footer,
    }
}

pub fn calculate_table_layout(branches_area: Rect) -> TableLayout {
    let table_columns = [
        Constraint::Percentage(BRANCH_COLUMN_PERCENT),
        Constraint::Length(STATUS_COLUMN_WIDTH),
        Constraint::Min(WORKTREE_COLUMN_MIN_WIDTH),
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

    TableLayout {
        branch_width,
        worktree_width,
        table_columns,
    }
}

pub fn calculate_confirmation_area(area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(5),
            Constraint::Percentage(40),
        ])
        .split(area)[1];
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Min(40),
            Constraint::Percentage(20),
        ])
        .split(vertical)[1]
}

pub fn calculate_input_area(area: Rect, height: u16) -> Rect {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(height),
            Constraint::Percentage(40),
        ])
        .split(area)[1]
}

pub fn calculate_diff_area(area: Rect) -> Rect {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Min(5),
            Constraint::Percentage(10),
        ])
        .split(area)[1]
}

pub fn calculate_help_area(area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Min(14),
            Constraint::Percentage(20),
        ])
        .split(area)[1];
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Min(54),
            Constraint::Percentage(15),
        ])
        .split(vertical)[1]
}

pub fn calculate_details_dialog_area(area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Min(12),
            Constraint::Percentage(15),
        ])
        .split(area)[1];
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Min(50),
            Constraint::Percentage(10),
        ])
        .split(vertical)[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_layout_splits_horizontally() {
        let area = Rect::new(0, 0, 120, 40);
        let layout = calculate_main_layout(area);
        assert_eq!(layout.filter.height, 3);
        assert!(layout.branches.width > 0);
        assert!(layout.details.width > 0);
        // In wide layout, branches and details are side by side
        assert_eq!(layout.branches.y, layout.details.y);
    }

    #[test]
    fn narrow_layout_splits_vertically() {
        let area = Rect::new(0, 0, 80, 40);
        let layout = calculate_main_layout(area);
        assert_eq!(layout.filter.height, 3);
        // In narrow layout, branches is above details
        assert!(layout.branches.y < layout.details.y);
    }

    #[test]
    fn calculates_dialog_areas_within_bounds() {
        let area = Rect::new(0, 0, 100, 30);
        let conf = calculate_confirmation_area(area);
        assert!(conf.x >= area.x && conf.width <= area.width);
        assert!(conf.y >= area.y && conf.height <= area.height);

        let help = calculate_help_area(area);
        assert!(help.x >= area.x && help.width <= area.width);

        let diff = calculate_diff_area(area);
        assert!(diff.x >= area.x && diff.width <= area.width);

        let details = calculate_details_dialog_area(area);
        assert!(details.x >= area.x && details.width <= area.width);

        let input = calculate_input_area(area, 6);
        assert_eq!(input.height, 6);
    }
}

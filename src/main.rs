use std::io::stderr;

use crossterm::{
    cursor::Show,
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

mod app;
mod domain;
mod infrastructure;
mod ui;

use app::action::AppAction;
use app::port::GitRepositoryPort;
use app::state::*;
use app::use_cases::RepositoryUseCase;
use infrastructure::GitCliRepository;
use ui::constants::*;
use ui::input::map_key_event;
use ui::render::*;

pub enum TuiAction {
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

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let repository = GitCliRepository::open(".")?;
    let selected = match run_tui(&repository)? {
        TuiAction::Cancel => return Ok(()),
        TuiAction::Select(selected) => selected,
    };

    let destination = if let Some(path) = selected.worktree_path.as_deref() {
        path.to_owned()
    } else {
        let use_case = RepositoryUseCase::new(&repository);
        use_case.get_repository_root()?
    };
    println!("{destination}");
    Ok(())
}

fn run_tui(
    repository: &GitCliRepository,
) -> Result<TuiAction, Box<dyn std::error::Error + Send + Sync>> {
    let use_case = RepositoryUseCase::new(repository);
    let branch_items = use_case.refresh_branches(true)?;
    let entries: Vec<BranchEntry> = branch_items
        .iter()
        .map(|item| item.to_branch_entry())
        .collect();

    let _terminal_guard = TerminalGuard::activate()?;
    let backend = CrosstermBackend::new(stderr());
    let mut terminal = Terminal::new(backend)?;
    run_event_loop(&mut terminal, repository, entries)
}

fn run_event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    repository: &GitCliRepository,
    entries: Vec<BranchEntry>,
) -> Result<TuiAction, Box<dyn std::error::Error + Send + Sync>> {
    let use_case = RepositoryUseCase::new(repository);
    let mut app = TuiApp::new(entries);
    loop {
        terminal.draw(|frame| render_app(frame, &mut app))?;
        if !event::poll(EVENT_POLL_INTERVAL)? {
            continue;
        }
        let Event::Key(key_event) = event::read()? else {
            continue;
        };
        if key_event.kind != KeyEventKind::Press {
            continue;
        }
        let action = map_key_event(key_event, &app);
        if let Some(tui_action) = handle_action(&mut app, &use_case, action)? {
            return Ok(tui_action);
        }
    }
}

fn handle_action(
    app: &mut TuiApp,
    use_case: &RepositoryUseCase,
    action: AppAction,
) -> Result<Option<TuiAction>, Box<dyn std::error::Error + Send + Sync>> {
    match action {
        AppAction::CloseDiff => {
            app.diff_view = None;
            app.message = String::new();
        }
        AppAction::CloseHelp => {
            app.help_visible = false;
            app.help_scroll = 0;
            app.message = String::new();
        }
        AppAction::ScrollHelp(amount) => app.scroll_help(amount),
        AppAction::CloseDetails => {
            app.details_dialog_visible = false;
            app.details_scroll = 0;
            app.message = String::new();
        }
        AppAction::ScrollDetails(amount) => app.scroll_details(amount),
        AppAction::ClearFilter => {
            app.filtering = false;
            app.update_filter(None);
            app.message = String::from("Filter cleared");
        }
        AppAction::FinishFilter => {
            app.filtering = false;
            app.message = format!("{} branches", app.entries.len());
        }
        AppAction::FilterBackspace => {
            let mut filter = app.filter.take().unwrap_or_default();
            filter.pop();
            app.update_filter(Some(filter));
        }
        AppAction::FilterChar(character) => {
            let mut filter = app.filter.take().unwrap_or_default();
            filter.push(character);
            app.update_filter(Some(filter));
        }
        AppAction::CancelInput => {
            app.input_mode = None;
            app.message = String::from("Operation cancelled");
        }
        AppAction::InputBackspace => match app.input_mode.take() {
            Some(InputMode::AddWorktree { branch, mut path }) => {
                path.pop();
                app.input_mode = Some(InputMode::AddWorktree { branch, path });
            }
            Some(InputMode::CreateBranch { mut name, parent }) => {
                name.pop();
                app.input_mode = Some(InputMode::CreateBranch { name, parent });
            }
            Some(InputMode::RenameBranch {
                old_name,
                mut new_name,
            }) => {
                new_name.pop();
                app.input_mode = Some(InputMode::RenameBranch { old_name, new_name });
            }
            None => {}
        },
        AppAction::InputChar(character) => match app.input_mode.take() {
            Some(InputMode::AddWorktree { branch, mut path }) => {
                path.push(character);
                app.input_mode = Some(InputMode::AddWorktree { branch, path });
            }
            Some(InputMode::CreateBranch { mut name, parent }) => {
                name.push(character);
                app.input_mode = Some(InputMode::CreateBranch { name, parent });
            }
            Some(InputMode::RenameBranch {
                old_name,
                mut new_name,
            }) => {
                new_name.push(character);
                app.input_mode = Some(InputMode::RenameBranch { old_name, new_name });
            }
            None => {}
        },
        AppAction::SubmitInput(pending) => {
            app.input_mode = None;
            execute_action_and_reload(app, use_case, pending);
        }
        AppAction::CancelPendingAction => {
            app.pending_action = None;
            app.message = String::from("Operation cancelled");
        }
        AppAction::ConfirmPendingAction(pending) => {
            app.pending_action = None;
            execute_action_and_reload(app, use_case, pending);
        }
        AppAction::Quit => return Ok(Some(TuiAction::Cancel)),
        AppAction::MoveSelection(amount) => app.move_selection(amount),
        AppAction::OpenOrCheckout => {
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
                    return Ok(Some(TuiAction::Select(selected)));
                } else {
                    match use_case.checkout_branch(&selected.name) {
                        Ok(()) => return Ok(Some(TuiAction::Select(selected))),
                        Err(error) => app.message = format!("Checkout failed: {error}"),
                    }
                }
            }
        }
        AppAction::Refresh => {
            app.message = String::from("Reloading branches...");
            match use_case.refresh_branches(true) {
                Ok(branch_items) => {
                    let entries: Vec<BranchEntry> = branch_items
                        .iter()
                        .map(|item| item.to_branch_entry())
                        .collect();
                    app.reload(entries);
                    app.message = String::from("Branches reloaded");
                }
                Err(error) => app.message = format!("Reload failed: {error}"),
            }
        }
        AppAction::ShowDiff => {
            if let Some(selected) = app.selected() {
                match use_case.get_diff_stat(selected.worktree_path.as_deref(), &selected.name) {
                    Ok(diff) => app.diff_view = Some(diff),
                    Err(error) => app.message = format!("Diff failed: {error}"),
                }
            }
        }
        AppAction::StartFilter => {
            app.filtering = true;
            app.update_filter(Some(String::new()));
            app.message = String::from("Filter branches");
        }
        AppAction::StartAddWorktree => {
            if let Some(selected) = app.selected().cloned() {
                if selected.worktree_path.is_some() {
                    app.message = String::from("Branch already has a worktree");
                } else {
                    match use_case.get_default_worktree_path(&selected.name) {
                        Ok(path) => {
                            app.input_mode = Some(InputMode::AddWorktree {
                                branch: selected,
                                path,
                            });
                        }
                        Err(error) => app.message = format!("Failed to get path: {error}"),
                    }
                }
            }
        }
        AppAction::StartRemoveWorktree => {
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
        AppAction::StartCreateBranch => {
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
        AppAction::StartRenameBranch => {
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
        AppAction::StartDeleteBranch => {
            if let Some(selected) = app.selected() {
                if selected.is_current || selected.worktree_path.is_some() {
                    app.message = String::from("Cannot delete a current or worktree branch");
                } else {
                    app.pending_action = Some(PendingAction::DeleteBranch(selected.name.clone()));
                }
            }
        }
        AppAction::ShowHelp => {
            app.help_scroll = 0;
            app.help_visible = true;
        }
        AppAction::ShowDetails => {
            app.details_dialog_visible = true;
            app.details_scroll = 0;
        }
        AppAction::None => {}
    }
    Ok(None)
}

fn execute_action_and_reload(
    app: &mut TuiApp,
    use_case: &RepositoryUseCase,
    action: PendingAction,
) {
    match execute_pending_action(use_case, &action) {
        Ok(()) => match use_case.refresh_branches(true) {
            Ok(branch_items) => {
                let entries: Vec<BranchEntry> = branch_items
                    .iter()
                    .map(|item| item.to_branch_entry())
                    .collect();
                app.reload(entries);
                app.message = String::from("Operation completed");
            }
            Err(error) => app.message = format!("Reload failed: {error}"),
        },
        Err(error) => app.message = format!("Operation failed: {error}"),
    }
}

fn execute_pending_action(
    use_case: &RepositoryUseCase,
    action: &PendingAction,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match action {
        PendingAction::Checkout(entry) => {
            use_case.checkout_branch(&entry.name)?;
        }
        PendingAction::AddWorktree { branch, path } => {
            use_case.add_worktree(&branch.name, path)?;
        }
        PendingAction::RemoveWorktree(entry) => {
            let path = entry
                .worktree_path
                .as_deref()
                .ok_or("missing worktree path")?;
            use_case.remove_worktree(path)?;
        }
        PendingAction::CreateBranch { name, parent } => {
            use_case.create_branch(name, parent)?;
        }
        PendingAction::RenameBranch { old_name, new_name } => {
            use_case.rename_branch(old_name, new_name)?;
        }
        PendingAction::DeleteBranch(name) => {
            use_case.delete_branch(name)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {}

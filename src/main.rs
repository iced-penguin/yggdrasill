use std::{
    io::stderr,
    path::Path,
    process::{Command, Stdio},
};

use crossterm::{
    cursor::Show,
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

mod app;
mod ui;

use app::action::AppAction;
use app::state::*;
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
        if let Some(tui_action) = handle_action(&mut app, repository, action)? {
            return Ok(tui_action);
        }
    }
}

fn handle_action(
    app: &mut TuiApp,
    repository: &gix::Repository,
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
            execute_action_and_reload(app, repository, pending);
        }
        AppAction::CancelPendingAction => {
            app.pending_action = None;
            app.message = String::from("Operation cancelled");
        }
        AppAction::ConfirmPendingAction(pending) => {
            app.pending_action = None;
            execute_action_and_reload(app, repository, pending);
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
                    match checkout_branch(&selected.name) {
                        Ok(()) => return Ok(Some(TuiAction::Select(selected))),
                        Err(error) => app.message = format!("Checkout failed: {error}"),
                    }
                }
            }
        }
        AppAction::Refresh => {
            app.message = String::from("Reloading branches...");
            match load_entries(repository, true) {
                Ok(entries) => {
                    app.reload(entries);
                    app.message = String::from("Branches reloaded");
                }
                Err(error) => app.message = format!("Reload failed: {error}"),
            }
        }
        AppAction::ShowDiff => {
            if let Some(selected) = app.selected() {
                match diff_stat(selected) {
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
                    let path = default_worktree_path(&selected.name)?;
                    app.input_mode = Some(InputMode::AddWorktree {
                        branch: selected,
                        path,
                    });
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
}

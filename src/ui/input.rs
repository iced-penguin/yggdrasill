use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::constants::{LINE_SCROLL_DISTANCE, PAGE_SCROLL_DISTANCE};
use crate::app::action::AppAction;
use crate::app::state::{InputMode, PendingAction, TuiApp};

pub fn map_key_event(event: KeyEvent, app: &TuiApp) -> AppAction {
    let KeyEvent {
        code, modifiers, ..
    } = event;

    // 1. Diff view mode
    if app.diff_view.is_some() {
        if code == KeyCode::Esc {
            return AppAction::CloseDiff;
        }
        return AppAction::None;
    }

    // 2. Help dialog mode
    if app.help_visible {
        return match code {
            KeyCode::Esc | KeyCode::Char('h') => AppAction::CloseHelp,
            KeyCode::Up | KeyCode::Char('k') => AppAction::ScrollHelp(-LINE_SCROLL_DISTANCE),
            KeyCode::Down | KeyCode::Char('j') => AppAction::ScrollHelp(LINE_SCROLL_DISTANCE),
            KeyCode::PageUp | KeyCode::Char('b') => AppAction::ScrollHelp(-PAGE_SCROLL_DISTANCE),
            KeyCode::PageDown | KeyCode::Char(' ') => AppAction::ScrollHelp(PAGE_SCROLL_DISTANCE),
            _ => AppAction::None,
        };
    }

    // 3. Details dialog mode
    if app.details_dialog_visible {
        return match code {
            KeyCode::Esc | KeyCode::Char('i') => AppAction::CloseDetails,
            KeyCode::Up | KeyCode::Char('k') => AppAction::ScrollDetails(-LINE_SCROLL_DISTANCE),
            KeyCode::Down | KeyCode::Char('j') => AppAction::ScrollDetails(LINE_SCROLL_DISTANCE),
            KeyCode::PageUp | KeyCode::Char('b') => AppAction::ScrollDetails(-PAGE_SCROLL_DISTANCE),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                AppAction::ScrollDetails(PAGE_SCROLL_DISTANCE)
            }
            _ => AppAction::None,
        };
    }

    // 4. Filtering mode
    if app.filtering {
        return match code {
            KeyCode::Esc => AppAction::ClearFilter,
            KeyCode::Enter => AppAction::FinishFilter,
            KeyCode::Backspace => AppAction::FilterBackspace,
            KeyCode::Char(c) => AppAction::FilterChar(c),
            _ => AppAction::None,
        };
    }

    // 5. Input mode
    if let Some(input_mode) = &app.input_mode {
        return match (input_mode, code) {
            (_, KeyCode::Esc) => AppAction::CancelInput,
            (InputMode::AddWorktree { branch, path }, KeyCode::Enter) if !path.is_empty() => {
                AppAction::SubmitInput(PendingAction::AddWorktree {
                    branch: branch.clone(),
                    path: path.clone(),
                })
            }
            (InputMode::CreateBranch { name, parent }, KeyCode::Enter) if !name.is_empty() => {
                AppAction::SubmitInput(PendingAction::CreateBranch {
                    name: name.clone(),
                    parent: parent.clone(),
                })
            }
            (InputMode::RenameBranch { old_name, new_name }, KeyCode::Enter)
                if !new_name.is_empty() =>
            {
                AppAction::SubmitInput(PendingAction::RenameBranch {
                    old_name: old_name.clone(),
                    new_name: new_name.clone(),
                })
            }
            (_, KeyCode::Backspace) => AppAction::InputBackspace,
            (_, KeyCode::Char(c)) => AppAction::InputChar(c),
            _ => AppAction::None,
        };
    }

    // 6. Confirmation mode
    if let Some(action) = &app.pending_action {
        return match code {
            KeyCode::Esc => AppAction::CancelPendingAction,
            KeyCode::Enter => AppAction::ConfirmPendingAction(action.clone()),
            _ => AppAction::None,
        };
    }

    // 7. Normal navigation
    match (code, modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Esc, KeyModifiers::NONE) => {
            AppAction::Quit
        }
        (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => AppAction::MoveSelection(-1),
        (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
            AppAction::MoveSelection(1)
        }
        (KeyCode::Enter, _) => AppAction::OpenOrCheckout,
        (KeyCode::Char('r'), KeyModifiers::NONE) => AppAction::Refresh,
        (KeyCode::Char('d'), KeyModifiers::NONE) => AppAction::ShowDiff,
        (KeyCode::Char('/'), KeyModifiers::NONE) => AppAction::StartFilter,
        (KeyCode::Char('a'), KeyModifiers::NONE) => AppAction::StartAddWorktree,
        (KeyCode::Char('x'), KeyModifiers::NONE) => AppAction::StartRemoveWorktree,
        (KeyCode::Char('b'), KeyModifiers::NONE) => AppAction::StartCreateBranch,
        (KeyCode::Char('e'), KeyModifiers::NONE) => AppAction::StartRenameBranch,
        (KeyCode::Char('D'), KeyModifiers::SHIFT) => AppAction::StartDeleteBranch,
        (KeyCode::Char('h'), KeyModifiers::NONE) => AppAction::ShowHelp,
        (KeyCode::Char('i'), KeyModifiers::NONE) => AppAction::ShowDetails,
        _ => AppAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    fn key_shift(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    #[test]
    fn modal_precedence_is_preserved() {
        let mut app = TuiApp::new(Vec::new());

        // Normal navigation
        assert_eq!(
            map_key_event(key(KeyCode::Char('q')), &app),
            AppAction::Quit
        );
        assert_eq!(
            map_key_event(key(KeyCode::Char('j')), &app),
            AppAction::MoveSelection(1)
        );

        // Pending action mode
        app.pending_action = Some(PendingAction::DeleteBranch("test".into()));
        assert_eq!(
            map_key_event(key(KeyCode::Esc), &app),
            AppAction::CancelPendingAction
        );
        assert_eq!(
            map_key_event(key(KeyCode::Enter), &app),
            AppAction::ConfirmPendingAction(PendingAction::DeleteBranch("test".into()))
        );

        // Input mode takes precedence over pending action
        app.input_mode = Some(InputMode::CreateBranch {
            name: "new-b".into(),
            parent: "main".into(),
        });
        assert_eq!(
            map_key_event(key(KeyCode::Esc), &app),
            AppAction::CancelInput
        );
        assert_eq!(
            map_key_event(key(KeyCode::Enter), &app),
            AppAction::SubmitInput(PendingAction::CreateBranch {
                name: "new-b".into(),
                parent: "main".into(),
            })
        );

        // Filter mode takes precedence over input mode
        app.filtering = true;
        assert_eq!(
            map_key_event(key(KeyCode::Esc), &app),
            AppAction::ClearFilter
        );
        assert_eq!(
            map_key_event(key(KeyCode::Enter), &app),
            AppAction::FinishFilter
        );

        // Details dialog takes precedence over filter mode
        app.details_dialog_visible = true;
        assert_eq!(
            map_key_event(key(KeyCode::Esc), &app),
            AppAction::CloseDetails
        );

        // Help dialog takes precedence over details dialog
        app.help_visible = true;
        assert_eq!(map_key_event(key(KeyCode::Esc), &app), AppAction::CloseHelp);

        // Diff view takes highest precedence
        app.diff_view = Some("diff".into());
        assert_eq!(map_key_event(key(KeyCode::Esc), &app), AppAction::CloseDiff);
    }

    #[test]
    fn maps_delete_branch_shift_d() {
        let app = TuiApp::new(Vec::new());
        assert_eq!(
            map_key_event(key_shift(KeyCode::Char('D')), &app),
            AppAction::StartDeleteBranch
        );
    }
}

use super::state::PendingAction;

#[derive(Debug, PartialEq, Eq)]
pub enum AppAction {
    // Diff mode
    CloseDiff,

    // Help mode
    CloseHelp,
    ScrollHelp(isize),

    // Details dialog mode
    CloseDetails,
    ScrollDetails(isize),

    // Filter mode
    ClearFilter,
    FinishFilter,
    FilterBackspace,
    FilterChar(char),

    // Input mode
    CancelInput,
    InputBackspace,
    InputChar(char),
    SubmitInput(PendingAction),

    // Pending action (confirmation) mode
    CancelPendingAction,
    ConfirmPendingAction(PendingAction),

    // Normal navigation
    Quit,
    MoveSelection(isize),
    OpenOrCheckout,
    Refresh,
    ShowDiff,
    StartFilter,
    StartAddWorktree,
    StartRemoveWorktree,
    StartCreateBranch,
    StartRenameBranch,
    StartDeleteBranch,
    ShowHelp,
    ShowDetails,

    None,
}

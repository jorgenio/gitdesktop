//! Mensajes (eventos) que la UI envía a `update` — arquitectura Elm de iced.

use std::path::PathBuf;

use gitdesktop_core::{BranchInfo, CommitSummary, DiffLine, RepoStatus};
use iced::widget::text_editor;

/// Pestaña activa del panel izquierdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Changes,
    History,
}

/// Operación sobre el índice/workdir cuya finalización se reporta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Stage,
    Unstage,
    Discard,
    Commit,
    MergeCommit,
}

/// Resultado de una operación de rama (para el banner y el refresco).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchOpResult {
    Created,
    Switched,
    Deleted,
}

/// Acciones disponibles en el menú principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    OpenRepository,
    Clone,
    Refresh,
    ShowChanges,
    ShowHistory,
    NewBranch,
    MergeBranches,
    ToggleTheme,
    About,
    Quit,
}

/// Mensajes de la aplicación.
#[derive(Debug, Clone)]
pub enum Message {
    // ——— Menú y ventana (cromo personalizado) ———
    MenuToggled(Option<usize>),
    MenuSelected(MenuAction),
    TitleBarPressed,
    MinimizeWindow,
    ToggleMaximize,
    CloseWindow,
    AboutToggled(bool),
    /// Id de la ventana principal capturado al arrancar (iced 0.14 no
    /// expone un `Id::MAIN`; `window::latest()` da la ventana de boot).
    MainWindowId(Option<iced::window::Id>),

    // ——— Repositorios ———
    OpenFolderDialog,
    FolderPicked(Option<PathBuf>),
    RepoOpened(Result<(PathBuf, RepoStatus, Vec<String>), String>),
    OpenRecent(String),
    /// Refresco automático (polling), con anti-rebote.
    AutoRefresh,
    TabSelected(Tab),

    // ——— Pestaña Cambios ———
    FileSelected { path: String, staged: bool },
    ToggleStaged { path: String, staged: bool },
    RequestDiscard { path: String },
    ConfirmDiscard,
    CancelDiscard,
    SummaryChanged(String),
    DescriptionEdited(text_editor::Action),
    CommitPressed,
    OpDone(Op, Result<(), String>),
    DiffLoaded(Result<(String, Vec<DiffLine>), String>),

    // ——— Pestaña Historial ———
    LogLoaded(Result<Vec<CommitSummary>, String>),
    CommitSelected(String),
    CommitFilesLoaded(Result<(String, Vec<gitdesktop_core::FileEntry>), String>),
    CommitFileSelected(String),
    CommitDiffLoaded(Result<(String, String, Vec<DiffLine>), String>),

    // ——— Ramas (M4) ———
    BranchModalToggled(bool),
    BranchesLoaded(Result<Vec<BranchInfo>, String>),
    NewBranchNameChanged(String),
    BranchCreatePressed,
    BranchSwitch(String),
    BranchDeleteRequested(String),
    BranchDeleteConfirmed,
    BranchCancelDelete,
    BranchDone(BranchOpResult, Result<(), String>),

    // ——— Merge (M4) ———
    MergeModalToggled(bool),
    MergeTargetPicked(Option<String>),
    MergePressed,
    MergeDone(Result<gitdesktop_core::MergeOutcome, String>),
    MergeCommitPressed,
    MergeAborted,

    // ——— Clone (M4) ———
    CloneModalToggled(bool),
    CloneUrlChanged(String),
    /// Abre el diálogo nativo de carpeta de destino.
    ClonePickFolder,
    CloneDestPicked(Option<PathBuf>),
    ClonePressed,

    // ——— Red (fetch/pull/push/clone vía CLI git) ———
    /// Inicia una operación de red (botón contextual de la cabecera).
    NetStart(gitdesktop_core::NetOpKind),
    /// Progreso intermedio de la operación en curso.
    NetProgress(gitdesktop_core::Progress),
    /// Operación terminada (Ok con resumen o Err con mensaje de git).
    NetDone(gitdesktop_core::NetOpKind, Result<String, String>),

    /// Abre el archivo (en conflicto) en el editor del usuario (jorgenio).
    ResolveInEditor { path: String },
    /// Abre el editor de conflictos embebido.
    EditConflictPressed { path: String },
    /// Acción del editor embebido (teclado/rueda/botones).
    EditorMsg(crate::widgets::conflict_editor::EditAction),

    ClearError,
    ClearNotice,
}

//! Mensajes (eventos) que la UI envía a `update` — arquitectura Elm de iced.

use std::path::PathBuf;

use gitdesktop_core::{CommitSummary, DiffLine, FileEntry, RepoStatus};
use iced::widget::text_editor;

/// Pestaña activa del panel izquierdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Changes,
    History,
}

impl Tab {
    // La UI construye sus etiquetas directamente; no requiere helpers.
}

/// Operación sobre el índice/workdir cuya finalización se reporta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Stage,
    Unstage,
    Discard,
    Commit,
}

/// Acciones disponibles en el menú principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    OpenRepository,
    Refresh,
    ShowChanges,
    ShowHistory,
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
    RepoOpened(Result<(PathBuf, RepoStatus), String>),
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
    CommitFilesLoaded(Result<(String, Vec<FileEntry>), String>),
    CommitFileSelected(String),
    CommitDiffLoaded(Result<(String, String, Vec<DiffLine>), String>),

    ClearError,
}

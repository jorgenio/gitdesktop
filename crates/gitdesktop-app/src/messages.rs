//! Mensajes (eventos) que la UI envía a `update` — arquitectura Elm de iced.

use std::path::PathBuf;

use gitdesktop_core::RepoStatus;

/// Pestaña activa del panel izquierdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Changes,
    History,
}

impl Tab {
    pub const ALL: [Tab; 2] = [Tab::Changes, Tab::History];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Changes => "Cambios",
            Tab::History => "Historial",
        }
    }
}

/// Mensajes de la aplicación.
#[derive(Debug, Clone)]
pub enum Message {
    /// Abrir el diálogo nativo de selección de carpeta.
    OpenFolderDialog,
    /// Resultado del diálogo (`None` si el usuario canceló).
    FolderPicked(Option<PathBuf>),
    /// Terminó de abrir el repositorio y calcular su estado.
    RepoOpened(Result<(PathBuf, RepoStatus), String>),
    /// Reabrir un repositorio reciente por su `id`.
    OpenRecent(String),
    /// Recalcular el estado del repositorio abierto.
    Refresh,
    /// Cambiar de pestaña (Cambios / Historial).
    TabSelected(Tab),
    /// Descartar el banner de error.
    ClearError,
}

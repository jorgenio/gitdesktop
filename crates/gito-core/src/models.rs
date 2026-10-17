//! Modelos de datos que consume la UI. Nunca exponen tipos de `git2`.

use std::fmt;

/// Estado de un archivo dentro del repositorio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Typechange,
    Conflicted,
}

impl FileStatus {
    /// Etiqueta legible (i18n centralizado llegará en fase 2).
    pub fn label(self) -> &'static str {
        match self {
            FileStatus::Modified => "Modificado",
            FileStatus::Added => "Nuevo",
            FileStatus::Deleted => "Eliminado",
            FileStatus::Renamed => "Renombrado",
            FileStatus::Typechange => "Tipo cambiado",
            FileStatus::Conflicted => "Conflicto",
        }
    }
}

impl fmt::Display for FileStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Un archivo listado en la vista de cambios.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Ruta relativa a la raíz del repositorio, siempre con `/`.
    pub path: String,
    pub status: FileStatus,
    /// `true` si el cambio ya está en el índice (staged).
    pub staged: bool,
}

/// Instantánea del estado de un repositorio.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoStatus {
    /// Nombre de la rama actual (`""` si el repo no tiene commits aún).
    pub branch: String,
    /// `true` si HEAD está detached (no apunta a una rama).
    pub head_detached: bool,
    /// Commits que tiene la rama local y no tiene el upstream.
    pub ahead: u32,
    /// Commits que tiene el upstream y no tiene la rama local.
    pub behind: u32,
    pub entries: Vec<FileEntry>,
}

impl RepoStatus {
    /// Archivos ya preparados para commit.
    pub fn staged_count(&self) -> usize {
        self.entries.iter().filter(|e| e.staged).count()
    }

    /// Archivos con cambios sin preparar.
    pub fn unstaged_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.staged).count()
    }
}

/// Información de una rama (la usará el selector a partir de M4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_staged_and_unstaged() {
        let status = RepoStatus {
            entries: vec![
                FileEntry { path: "a".into(), status: FileStatus::Added, staged: true },
                FileEntry { path: "b".into(), status: FileStatus::Modified, staged: true },
                FileEntry { path: "c".into(), status: FileStatus::Modified, staged: false },
            ],
            ..RepoStatus::default()
        };

        assert_eq!(status.staged_count(), 2);
        assert_eq!(status.unstaged_count(), 1);
    }

    #[test]
    fn file_status_labels() {
        assert_eq!(FileStatus::Added.to_string(), "Nuevo");
        assert_eq!(FileStatus::Conflicted.to_string(), "Conflicto");
    }
}

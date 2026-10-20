//! Abstracción del motor Git. La UI nunca toca `git2` directamente.
//!
//! Todas las operaciones son síncronas y están pensadas para ejecutarse
//! fuera del hilo de la UI (en `gito-app`, vía `Task::perform`). A partir
//! de M3 el motor interno pasará a un hilo worker dedicado (ver PLAN.md §5).

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::models::{BranchInfo, CommitSummary, DiffLine, FileEntry, MergeOutcome, RepoStatus};
use crate::native::NativeEngine;

/// Motor de operaciones Git para un repositorio abierto.
pub trait GitEngine: Send {
    /// Ruta del repositorio.
    fn path(&self) -> &Path;

    /// Estado actual: rama, ahead/behind y archivos modificados.
    fn status(&self) -> Result<RepoStatus>;

    /// Nombres de los remotos configurados (`origin`, etc.).
    fn remotes(&self) -> Result<Vec<String>>;

    // ——— Índice (stage) ————————————————————————————————————————————

    /// Prepara un archivo (`git add`; también registra borrados).
    fn stage(&self, path: &str) -> Result<()>;

    /// Saca un archivo del índice (`git reset HEAD -- path`).
    fn unstage(&self, path: &str) -> Result<()>;

    /// Descarta los cambios sin preparar de un archivo. Para archivos sin
    /// rastrear los **elimina** (como GitHub Desktop).
    fn discard(&self, path: &str) -> Result<()>;

    /// Crea un commit con el índice actual.
    fn commit(&self, summary: &str, body: &str) -> Result<()>;

    // ——— Diffs ——————————————————————————————————————————————————————

    /// Diff unificado de un archivo. Si `staged` es `true`, HEAD ↔ índice;
    /// si no, índice ↔ directorio de trabajo.
    fn file_diff(&self, path: &str, staged: bool) -> Result<Vec<DiffLine>>;

    // ——— Historial ——————————————————————————————————————————————————

    /// Últimos `limit` commits de la rama actual.
    fn log(&self, limit: usize) -> Result<Vec<CommitSummary>>;

    /// Archivos tocados por un commit.
    fn commit_files(&self, commit_id: &str) -> Result<Vec<FileEntry>>;

    /// Diff unificado de un archivo dentro de un commit.
    fn commit_diff(&self, commit_id: &str, path: &str) -> Result<Vec<DiffLine>>;

    // ——— Ramas y fusión (M4) ———————————————————————————————————————

    /// Ramas locales, con la actual marcada.
    fn branches(&self) -> Result<Vec<BranchInfo>>;

    /// Crea una rama y cambia a ella (`git switch -c`).
    fn create_branch(&self, name: &str) -> Result<()>;

    /// Cambia a una rama existente (`git switch`).
    fn switch_branch(&self, name: &str) -> Result<()>;

    /// Elimina una rama local (`git branch -D`).
    fn delete_branch(&self, name: &str) -> Result<()>;

    /// Fusiona `name` en la rama actual (`git merge --no-edit`).
    fn merge(&self, name: &str) -> Result<MergeOutcome>;

    /// Abandona el merge en curso (`git merge --abort`).
    fn abort_merge(&self) -> Result<()>;

    /// Confirma el merge tras resolver conflictos (`git commit --no-edit`).
    fn commit_merge(&self) -> Result<()>;

    /// ¿Quedan marcadores de conflicto (`<<<<<<<`) en el archivo del
    /// directorio de trabajo? `Ok(false)` = listo para marcar resuelto.
    fn conflict_markers_remaining(&self, path: &str) -> Result<bool>;
}

/// Abre un repositorio existente con el motor nativo (libgit2).
///
/// # Errores
/// Devuelve [`GitoError::NotARepository`](crate::GitoError::NotARepository)
/// si la ruta no contiene un repositorio git.
pub fn open(path: impl Into<PathBuf>) -> Result<Box<dyn GitEngine>> {
    let engine = NativeEngine::open(path)?;
    Ok(Box::new(engine))
}

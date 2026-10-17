//! Abstracción del motor Git. La UI nunca toca `git2` directamente.
//!
//! Todas las operaciones son síncronas y están pensadas para ejecutarse
//! fuera del hilo de la UI (en `gito-app`, vía `Task::perform`). A partir
//! de M3 el motor interno pasará a un hilo worker dedicado (ver PLAN.md §5).

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::models::RepoStatus;

/// Motor de operaciones Git para un repositorio abierto.
pub trait GitEngine: Send {
    /// Ruta del repositorio.
    fn path(&self) -> &Path;

    /// Estado actual: rama, ahead/behind y archivos modificados.
    fn status(&self) -> Result<RepoStatus>;
}

/// Abre un repositorio existente con el motor nativo (libgit2).
///
/// # Errores
/// Devuelve [`GitoError::NotARepository`](crate::GitoError::NotARepository)
/// si la ruta no contiene un repositorio git.
pub fn open(path: impl Into<PathBuf>) -> Result<Box<dyn GitEngine>> {
    let engine = native::NativeEngine::open(path)?;
    Ok(Box::new(engine))
}

mod native;

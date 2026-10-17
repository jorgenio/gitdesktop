//! Implementación del motor con `git2` (libgit2 embebido).

mod status;

use std::path::{Path, PathBuf};

use crate::engine::GitEngine;
use crate::error::{GitoError, Result};
use crate::models::RepoStatus;

/// Motor basado en libgit2. Dueño exclusivo de un `git2::Repository`.
///
/// `Repository` es `Send` pero no `Sync`: este motor está pensado para
/// usarse desde un único hilo (el worker, a partir de M1/M3).
pub struct NativeEngine {
    path: PathBuf,
    repo: git2::Repository,
}

impl NativeEngine {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let repo = git2::Repository::open(&path)
            .map_err(|_| GitoError::NotARepository(path.clone()))?;

        Ok(Self { path, repo })
    }
}

impl GitEngine for NativeEngine {
    fn path(&self) -> &Path {
        &self.path
    }

    fn status(&self) -> Result<RepoStatus> {
        status::status(&self.repo)
    }
}

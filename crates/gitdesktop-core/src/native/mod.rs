//! Implementación del motor con `git2` (libgit2 embebido).

mod diff;
mod log;
mod ops;
mod status;

use std::path::{Path, PathBuf};

use crate::engine::GitEngine;
use crate::error::{GitoError, Result};
use crate::models::{CommitSummary, DiffLine, FileEntry, RepoStatus};

/// Motor basado en libgit2. Dueño exclusivo de un `git2::Repository`.
///
/// `Repository` es `Send` pero no `Sync`: este motor está pensado para
/// usarse desde un único hilo (el worker, a partir de M3).
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

    /// Ruta absoluta a un archivo del directorio de trabajo.
    pub(crate) fn workdir_path(&self, relative: &str) -> PathBuf {
        self.repo
            .workdir()
            .unwrap_or(&self.path)
            .join(relative)
    }
}

impl GitEngine for NativeEngine {
    fn path(&self) -> &Path {
        &self.path
    }

    fn status(&self) -> Result<RepoStatus> {
        status::status(&self.repo)
    }

    fn stage(&self, path: &str) -> Result<()> {
        ops::stage(&self.repo, self.workdir_path(path), path)
    }

    fn unstage(&self, path: &str) -> Result<()> {
        ops::unstage(&self.repo, path)
    }

    fn discard(&self, path: &str) -> Result<()> {
        ops::discard(&self.repo, self.workdir_path(path), path)
    }

    fn commit(&self, summary: &str, body: &str) -> Result<()> {
        ops::commit(&self.repo, summary, body)
    }

    fn file_diff(&self, path: &str, staged: bool) -> Result<Vec<DiffLine>> {
        diff::file_diff(&self.repo, path, staged)
    }

    fn log(&self, limit: usize) -> Result<Vec<CommitSummary>> {
        log::history(&self.repo, limit)
    }

    fn commit_files(&self, commit_id: &str) -> Result<Vec<FileEntry>> {
        log::commit_files(&self.repo, commit_id)
    }

    fn commit_diff(&self, commit_id: &str, path: &str) -> Result<Vec<DiffLine>> {
        log::commit_diff(&self.repo, commit_id, path)
    }
}

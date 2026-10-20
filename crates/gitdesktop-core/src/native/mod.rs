//! Implementación del motor con `git2` (libgit2 embebido).

mod diff;
mod log;
mod ops;
mod status;

use std::path::{Path, PathBuf};

use crate::cli;
use crate::engine::GitEngine;
use crate::error::{GitoError, Result};
use crate::models::{BranchInfo, CommitSummary, DiffLine, FileEntry, MergeOutcome, RepoStatus};

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

    fn remotes(&self) -> Result<Vec<String>> {
        let names = self.repo.remotes()?;
        let mut remotes = Vec::new();
        for index in 0..names.len() {
            // get() -> Result<Option<&str>, _> en git2 0.21
            if let Some(name) = names.get(index).ok().flatten() {
                remotes.push(name.to_owned());
            }
        }
        Ok(remotes)
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

    fn branches(&self) -> Result<Vec<BranchInfo>> {
        let mut branches = Vec::new();
        for entry in self.repo.branches(Some(git2::BranchType::Local))? {
            let (branch, _) = entry?;
            let Some(name) = branch.name()? else {
                continue;
            };
            branches.push(BranchInfo {
                name: name.to_owned(),
                is_current: branch.is_head(),
            });
        }
        Ok(branches)
    }

    fn create_branch(&self, name: &str) -> Result<()> {
        branch_cli(&self.path, &["switch", "-c", name])
    }

    fn switch_branch(&self, name: &str) -> Result<()> {
        branch_cli(&self.path, &["switch", name])
    }

    fn delete_branch(&self, name: &str) -> Result<()> {
        branch_cli(&self.path, &["branch", "-D", name])
    }

    fn merge(&self, name: &str) -> Result<MergeOutcome> {
        branch_cli(&self.path, &["merge", "--no-edit", name])?;

        // ¿Dejó conflictos? (el status marca CONFLICTED en el workdir)
        let conflicted = self
            .status()?
            .entries
            .iter()
            .filter(|entry| entry.status == crate::models::FileStatus::Conflicted)
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();

        if conflicted.is_empty() {
            Ok(MergeOutcome::Merged)
        } else {
            Ok(MergeOutcome::Conflicts(conflicted))
        }
    }

    fn abort_merge(&self) -> Result<()> {
        branch_cli(&self.path, &["merge", "--abort"])
    }

    fn commit_merge(&self) -> Result<()> {
        branch_cli(&self.path, &["commit", "--no-edit"])
    }

    fn rebase(&self, name: &str) -> Result<MergeOutcome> {
        let output = cli::run(&self.path, &["rebase", "--no-edit", name])
            .map_err(|e| GitoError::GitCli(format!("no se pudo ejecutar git: {e}")))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        if combined.contains("CONFLICT") {
            let conflicted = self
                .status()?
                .entries
                .iter()
                .filter(|entry| entry.status == crate::models::FileStatus::Conflicted)
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>();
            return Ok(MergeOutcome::Conflicts(conflicted));
        }

        if output.status.success() {
            if combined.contains("is up to date") {
                Ok(MergeOutcome::UpToDate)
            } else {
                Ok(MergeOutcome::Merged)
            }
        } else {
            let text = last_line(&stderr).unwrap_or_else(|| {
                last_line(&stdout).unwrap_or_else(|| "git rebase falló".into())
            });
            Err(GitoError::GitCli(text))
        }
    }

    fn continue_rebase(&self) -> Result<()> {
        branch_cli_with_env(
            &self.path,
            &["rebase", "--continue"],
            [("GIT_EDITOR", "true")],
        )
    }

    fn abort_rebase(&self) -> Result<()> {
        branch_cli(&self.path, &["rebase", "--abort"])
    }

    fn conflict_markers_remaining(&self, path: &str) -> Result<bool> {
        let full = self.workdir_path(path);
        let content = std::fs::read_to_string(&full)
            .map_err(|e| crate::error::io_err(&full, e))?;
        Ok(content
            .lines()
            .any(|line| line.starts_with("<<<<<<<") || line.starts_with(">>>>>>>")))
    }
}

/// Ejecuta git CLI y traduce los errores con el mensaje real de git.
fn branch_cli(repo: &Path, args: &[&str]) -> Result<()> {
    let output = cli::run(repo, args)
        .map_err(|e| GitoError::GitCli(format!("no se pudo ejecutar git: {e}")))?;
    finish_branch_cli(output, args)
}

/// Variante con variables de entorno extra (p. ej. GIT_EDITOR=true).
fn branch_cli_with_env(
    repo: &Path,
    args: &[&str],
    envs: [(&str, &str); 1],
) -> Result<()> {
    let output = std::process::Command::new("git")
        .current_dir(repo)
        .args(args)
        .envs(envs)
        .output()
        .map_err(|e| GitoError::GitCli(format!("no se pudo ejecutar git: {e}")))?;
    finish_branch_cli(output, args)
}

fn finish_branch_cli(output: std::process::Output, args: &[&str]) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = last_line(&stderr).unwrap_or_else(|| last_line(&stdout).unwrap_or_default());
    Err(GitoError::GitCli(if text.is_empty() {
        format!("git {} falló", args.join(" "))
    } else {
        text
    }))
}

/// Última línea no vacía de un texto (git deja el error al final).
fn last_line(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

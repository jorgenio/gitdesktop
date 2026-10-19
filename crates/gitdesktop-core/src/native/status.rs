//! Cálculo de [`RepoStatus`] con libgit2.

use crate::error::Result;
use crate::models::{FileEntry, FileStatus, RepoStatus};

/// Rama actual, ahead/behind y lista de archivos modificados.
pub(super) fn status(repo: &git2::Repository) -> Result<RepoStatus> {
    let (branch, head_detached) = current_branch(repo);
    let (ahead, behind, has_upstream) = ahead_behind(repo);
    let has_remote = has_remote(repo);

    let mut opts = git2::StatusOptions::new();
    opts.show(git2::StatusShow::IndexAndWorkdir)
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .exclude_submodules(true)
        .renames_head_to_index(true)
        .update_index(false);

    let statuses = repo.statuses(Some(&mut opts))?;

    let mut entries = Vec::with_capacity(statuses.len());
    for entry in statuses.iter() {
        let s = entry.status();
        // Como en GitHub Desktop: un archivo puede aparecer dos veces
        // (una fila "staged" en el índice y otra con cambios en el
        // directorio de trabajo).
        if let Some(st) = index_status(s) {
            entries.push(FileEntry {
                path: entry_path(&entry),
                status: st,
                staged: true,
            });
        }
        if let Some(st) = workdir_status(s) {
            entries.push(FileEntry {
                path: entry_path(&entry),
                status: st,
                staged: false,
            });
        }
    }

    Ok(RepoStatus {
        branch,
        head_detached,
        ahead,
        behind,
        has_upstream,
        has_remote,
        entries,
    })
}

fn entry_path(entry: &git2::StatusEntry<'_>) -> String {
    entry
        .path()
        .unwrap_or_default()
        .replace('\\', "/")
}

/// Cambios preparados en el índice.
fn index_status(s: git2::Status) -> Option<FileStatus> {
    if s.contains(git2::Status::INDEX_NEW) {
        Some(FileStatus::Added)
    } else if s.contains(git2::Status::INDEX_MODIFIED) {
        Some(FileStatus::Modified)
    } else if s.contains(git2::Status::INDEX_DELETED) {
        Some(FileStatus::Deleted)
    } else if s.contains(git2::Status::INDEX_RENAMED) {
        Some(FileStatus::Renamed)
    } else if s.contains(git2::Status::INDEX_TYPECHANGE) {
        Some(FileStatus::Typechange)
    } else {
        None
    }
}

/// Cambios en el directorio de trabajo (sin preparar).
fn workdir_status(s: git2::Status) -> Option<FileStatus> {
    if s.contains(git2::Status::CONFLICTED) {
        Some(FileStatus::Conflicted)
    } else if s.contains(git2::Status::WT_NEW) {
        Some(FileStatus::Added)
    } else if s.contains(git2::Status::WT_MODIFIED) {
        Some(FileStatus::Modified)
    } else if s.contains(git2::Status::WT_DELETED) {
        Some(FileStatus::Deleted)
    } else if s.contains(git2::Status::WT_RENAMED) {
        Some(FileStatus::Renamed)
    } else if s.contains(git2::Status::WT_TYPECHANGE) {
        Some(FileStatus::Typechange)
    } else {
        None
    }
}

/// `(nombre_rama, head_detached)`. En un repo recién `git init` devuelve
/// el nombre de la rama aunque aún no tenga commits.
fn current_branch(repo: &git2::Repository) -> (String, bool) {
    match repo.head() {
        Ok(head) => {
            let name = head.shorthand().unwrap_or("HEAD").to_string();
            (name, !head.is_branch())
        }
        Err(_) => match repo.find_reference("HEAD") {
            Ok(head) => {
                // symbolic_target() -> Result<Option<&str>, _>
                let name = head
                    .symbolic_target()
                    .ok()
                    .flatten()
                    .unwrap_or("HEAD")
                    .trim_start_matches("refs/heads/")
                    .to_string();
                (name, false)
            }
            Err(_) => (String::new(), false),
        },
    }
}

/// `(ahead, behind, tiene_upstream)` respecto al upstream de la rama
/// actual. Sin upstream (o sin commits) devuelve `(0, 0, false)`.
fn ahead_behind(repo: &git2::Repository) -> (u32, u32, bool) {
    let Ok(head) = repo.head() else {
        return (0, 0, false);
    };
    let Ok(local_commit) = head.peel_to_commit() else {
        return (0, 0, false);
    };

    // `shorthand()` es Result<&str, _>: en HEAD detached devuelve el
    // short-oid y `find_branch` fallará → (0, 0, false), que es lo correcto.
    let Ok(branch_name) = head.shorthand() else {
        return (0, 0, false);
    };

    let Ok(branch) = repo.find_branch(branch_name, git2::BranchType::Local) else {
        return (0, 0, false);
    };
    let Ok(upstream) = branch.upstream() else {
        return (0, 0, false);
    };
    let Ok(upstream_commit) = upstream.get().peel_to_commit() else {
        return (0, 0, true);
    };

    match repo.graph_ahead_behind(local_commit.id(), upstream_commit.id()) {
        Ok((a, b)) => (a as u32, b as u32, true),
        Err(_) => (0, 0, true),
    }
}

/// `true` si el repositorio tiene al menos un remoto configurado.
fn has_remote(repo: &git2::Repository) -> bool {
    repo.remotes()
        .map(|names| !names.is_empty())
        .unwrap_or(false)
}

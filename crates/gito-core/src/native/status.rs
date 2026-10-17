//! Cálculo de [`RepoStatus`] con libgit2.

use crate::error::Result;
use crate::models::{FileEntry, FileStatus, RepoStatus};

/// Rama actual, ahead/behind y lista de archivos modificados.
pub(super) fn status(repo: &git2::Repository) -> Result<RepoStatus> {
    let (branch, head_detached) = current_branch(repo);
    let (ahead, behind) = ahead_behind(repo);

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
                path: entry_path(entry),
                status: st,
                staged: true,
            });
        }
        if let Some(st) = workdir_status(s) {
            entries.push(FileEntry {
                path: entry_path(entry),
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
        entries,
    })
}

fn entry_path(entry: git2::StatusEntry<'_>) -> String {
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
                let target = head.symbolic_target().unwrap_or("HEAD");
                let name = target.trim_start_matches("refs/heads/").to_string();
                (name, false)
            }
            Err(_) => (String::new(), false),
        },
    }
}

/// `(ahead, behind)` respecto al upstream de la rama actual.
/// Sin upstream (o sin commits) devuelve `(0, 0)`.
fn ahead_behind(repo: &git2::Repository) -> (u32, u32) {
    let Ok(head) = repo.head() else {
        return (0, 0);
    };
    let Ok(local_commit) = head.peel_to_commit() else {
        return (0, 0);
    };

    let Some(branch_name) = head
        .as_branch()
        .and_then(|b| b.name().ok().flatten())
        .map(str::to_owned)
    else {
        return (0, 0);
    };

    let Ok(branch) = repo.find_branch(&branch_name, git2::BranchType::Local) else {
        return (0, 0);
    };
    let Ok(upstream) = branch.upstream() else {
        return (0, 0);
    };
    let Ok(upstream_commit) = upstream.get().peel_to_commit() else {
        return (0, 0);
    };

    repo.graph_ahead_behind(local_commit.id(), upstream_commit.id())
        .map(|(a, b)| (a as u32, b as u32))
        .unwrap_or((0, 0))
}

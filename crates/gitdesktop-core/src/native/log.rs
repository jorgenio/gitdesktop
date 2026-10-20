//! Historial: lista de commits, archivos tocados y diffs por commit.

use time::OffsetDateTime;

use crate::error::Result;
use crate::models::{CommitSummary, FileEntry, FileStatus};

/// Últimos `limit` commits de HEAD (orden temporal + topológico).
pub(super) fn history(repo: &git2::Repository, limit: usize) -> Result<Vec<CommitSummary>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)?;

    let mut commits = Vec::new();
    for oid in revwalk.take(limit) {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        let parents = commit
            .parent_ids()
            .map(|id| id.to_string())
            .collect::<Vec<_>>();

        commits.push(CommitSummary {
            id: oid.to_string(),
            short_id: oid.to_string()[..7].to_owned(),
            parents,
            summary: commit
                .summary()
                .ok()
                .flatten()
                .unwrap_or("(sin mensaje)")
                .to_owned(),
            author: commit.author().name().unwrap_or("?").to_owned(),
            time: OffsetDateTime::from_unix_timestamp(commit.time().seconds())
                .unwrap_or(OffsetDateTime::UNIX_EPOCH),
        });
    }

    Ok(commits)
}

/// Archivos que toca un commit (comparado con su primer padre).
pub(super) fn commit_files(
    repo: &git2::Repository,
    commit_id: &str,
) -> Result<Vec<FileEntry>> {
    let commit = repo.find_commit(git2::Oid::from_str(commit_id)?)?;
    let tree = commit.tree()?;
    let parent_tree = commit.parent(0).ok().and_then(|parent| parent.tree().ok());

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

    let mut files = Vec::new();
    diff.foreach(
        &mut |delta, _progress| {
            let status = match delta.status() {
                git2::Delta::Added => FileStatus::Added,
                git2::Delta::Deleted => FileStatus::Deleted,
                git2::Delta::Renamed => FileStatus::Renamed,
                git2::Delta::Typechange => FileStatus::Typechange,
                _ => FileStatus::Modified,
            };

            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();

            files.push(FileEntry { path, status, staged: true });
            true
        },
        None,
        None,
        None,
    )?;

    Ok(files)
}

/// Diff unificado de un archivo dentro de un commit.
pub(super) fn commit_diff(
    repo: &git2::Repository,
    commit_id: &str,
    path: &str,
) -> Result<Vec<crate::models::DiffLine>> {
    let commit = repo.find_commit(git2::Oid::from_str(commit_id)?)?;
    let tree = commit.tree()?;
    let parent_tree = commit.parent(0).ok().and_then(|parent| parent.tree().ok());

    let mut opts = git2::DiffOptions::new();
    opts.pathspec(path);

    let diff =
        repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;

    // Reutiliza el recolector de líneas del módulo diff.
    super::diff::collect(&diff)
}

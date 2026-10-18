//! Operaciones sobre el índice y el directorio de trabajo:
//! stage/unstage/discard/commit (equivalentes a `git add`, `git reset`,
//! `git checkout` y `git commit`).

use std::path::{Path, PathBuf};

use crate::error::{GitoError, Result, io_err};

/// `git add <path>` — también registra borrados (`index.remove_path`).
pub(super) fn stage(
    repo: &git2::Repository,
    full_path: PathBuf,
    relative: &str,
) -> Result<()> {
    let mut index = repo.index()?;
    let rel = Path::new(relative);

    if full_path.exists() {
        index.add_path(rel)?;
    } else {
        index.remove_path(rel)?;
    }
    index.write()?;
    Ok(())
}

/// `git reset HEAD -- <path>` — saca el archivo del índice.
pub(super) fn unstage(repo: &git2::Repository, relative: &str) -> Result<()> {
    let head = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
    // reset_default toma `Option<&Object>`: convierte el commit.
    let head_object = head.as_ref().map(|commit| commit.as_object());
    repo.reset_default(head_object, [relative])?;
    Ok(())
}

/// Descarta los cambios sin preparar de un archivo:
/// - sin rastrear → elimina el archivo (como GitHub Desktop);
/// - rastreado    → restaura el contenido del índice en el workdir.
pub(super) fn discard(
    repo: &git2::Repository,
    full_path: PathBuf,
    relative: &str,
) -> Result<()> {
    if is_untracked(repo, relative) {
        std::fs::remove_file(&full_path)
            .map_err(|e| io_err(&full_path, e))?;
        return Ok(());
    }

    let mut opts = git2::build::CheckoutBuilder::new();
    opts.force().update_index(true).path(relative);

    repo.checkout_index(None, Some(&mut opts))?;
    Ok(())
}

/// `git commit` con el índice actual.
pub(super) fn commit(repo: &git2::Repository, summary: &str, body: &str) -> Result<()> {
    let signature = identity(repo)?;

    let mut index = repo.index()?;
    index.write()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let message = if body.trim().is_empty() {
        summary.to_owned()
    } else {
        format!("{summary}\n\n{}", body.trim())
    };

    let head = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
    match head {
        Some(parent) => {
            repo.commit(Some("HEAD"), &signature, &signature, &message, &tree, &[&parent])?;
        }
        // Primer commit de un repo recién inicializado (HEAD sin nacer).
        None => {
            repo.commit(Some("HEAD"), &signature, &signature, &message, &tree, &[])?;
        }
    }
    Ok(())
}

/// Firma del commit a partir de `user.name`/`user.email` (repo o global).
fn identity(repo: &git2::Repository) -> Result<git2::Signature<'static>> {
    let config = repo.config()?;

    let read = |key: &str| -> Option<String> {
        config
            .get_string(key)
            .ok()
            .or_else(|| {
                git2::Config::open_default()
                    .and_then(|global| global.get_string(key))
                    .ok()
            })
    };

    let name = read("user.name");
    let email = read("user.email");

    match (name, email) {
        (Some(name), Some(email)) => {
            Ok(git2::Signature::now(&name, &email)?)
        }
        _ => Err(GitoError::MissingIdentity),
    }
}

fn is_untracked(repo: &git2::Repository, relative: &str) -> bool {
    let mut opts = git2::StatusOptions::new();
    opts.show(git2::StatusShow::Workdir).include_untracked(true);

    repo.statuses(Some(&mut opts))
        .map(|statuses| {
            statuses.iter().any(|entry| {
                entry.path().ok() == Some(relative)
                    && entry.status().contains(git2::Status::WT_NEW)
            })
        })
        .unwrap_or(false)
}

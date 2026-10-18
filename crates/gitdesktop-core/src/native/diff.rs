//! Cálculo de diffs unificados con libgit2.
//!
//! Límite defensivo: se muestran como máximo [`MAX_LINES`] líneas para no
//! congelar la UI con archivos enormes o binarios mal detectados.

use crate::error::Result;
use crate::models::{DiffLine, DiffLineKind};

use std::path::Path;

const MAX_LINES: usize = 4000;

/// Diff unificado de un archivo.
/// - `staged == true`:  HEAD (árbol) ↔ índice
/// - `staged == false`: índice ↔ directorio de trabajo
pub(super) fn file_diff(
    repo: &git2::Repository,
    path: &str,
    staged: bool,
) -> Result<Vec<DiffLine>> {
    if staged {
        let head_tree = repo.head().ok().and_then(|head| head.peel_to_tree().ok());

        let mut opts = git2::DiffOptions::new();
        opts.pathspec(path);

        let diff = repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?;
        return collect(&diff);
    }

    // ¿Archivo sin rastrear? Su contenido completo son líneas añadidas.
    if is_untracked(repo, path) {
        return untracked_lines(repo, path);
    }

    let mut opts = git2::DiffOptions::new();
    opts.pathspec(path).include_untracked(true);

    let diff = repo.diff_index_to_workdir(None, Some(&mut opts))?;
    collect(&diff)
}

/// Recolecta las líneas de un diff ya construido.
pub(super) fn collect(diff: &git2::Diff<'_>) -> Result<Vec<DiffLine>> {
    let mut lines: Vec<DiffLine> = Vec::new();

    diff.foreach(
        &mut |_delta, _progress| true,
        None,
        Some(&mut |_delta, _hunk| true),
        Some(&mut |_delta, _hunk, line| {
            match line.origin() {
                // Marcadores de "sin salto de línea al final": se omiten.
                '>' | '<' | '=' => return true,
                _ => {}
            }

            if lines.len() >= MAX_LINES {
                return false; // corta el recorrido
            }

            let kind = match line.origin() {
                '+' => DiffLineKind::Added,
                '-' => DiffLineKind::Deleted,
                'H' => DiffLineKind::HunkHeader,
                _ => DiffLineKind::Context,
            };

            lines.push(DiffLine {
                old_no: line.old_lineno(),
                new_no: line.new_lineno(),
                kind,
                content: String::from_utf8_lossy(line.content()).into_owned(),
            });
            true
        }),
    )?;

    if lines.len() >= MAX_LINES {
        lines.push(DiffLine {
            old_no: None,
            new_no: None,
            kind: DiffLineKind::HunkHeader,
            content: format!("… diff truncado a {MAX_LINES} líneas"),
        });
    }

    Ok(lines)
}

/// Contenido completo de un archivo sin rastrear, como líneas añadidas.
fn untracked_lines(repo: &git2::Repository, path: &str) -> Result<Vec<DiffLine>> {
    let full = repo
        .workdir()
        .unwrap_or_else(|| Path::new("."))
        .join(path);
    let bytes = std::fs::read(&full).map_err(|e| crate::error::io_err(&full, e))?;

    if bytes.len() > 2 * 1024 * 1024 {
        return Ok(vec![DiffLine {
            old_no: None,
            new_no: None,
            kind: DiffLineKind::HunkHeader,
            content: "… archivo demasiado grande para previsualizar".to_owned(),
        }]);
    }

    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<DiffLine> = text
        .lines()
        .take(MAX_LINES)
        .enumerate()
        .map(|(i, content)| DiffLine {
            old_no: None,
            new_no: Some(i as u32 + 1),
            kind: DiffLineKind::Added,
            content: content.to_owned(),
        })
        .collect();

    if text.lines().count() > MAX_LINES {
        lines.push(DiffLine {
            old_no: None,
            new_no: None,
            kind: DiffLineKind::HunkHeader,
            content: format!("… diff truncado a {MAX_LINES} líneas"),
        });
    }

    Ok(lines)
}

fn is_untracked(repo: &git2::Repository, path: &str) -> bool {
    let mut opts = git2::StatusOptions::new();
    opts.show(git2::StatusShow::Workdir).include_untracked(true);

    repo.statuses(Some(&mut opts))
        .map(|statuses| {
            statuses.iter().any(|entry| {
                entry.path().ok() == Some(path)
                    && entry.status().contains(git2::Status::WT_NEW)
            })
        })
        .unwrap_or(false)
}

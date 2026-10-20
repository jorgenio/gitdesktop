//! Puente al GitHub CLI (`gh`): PRs con checks de CI, creación y checkout.
//!
//! Misma filosofía que el puente a `git` (PLAN.md §4.2): en lugar de
//! gestionar tokens de OAuth y REST a mano, delegamos en `gh`, que ya
//! trae la autenticación del usuario (`gh auth login`), scopes correctos
//! y detección del repositorio desde el remoto de git.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// Estado consolidado de los checks de CI de un PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    None,
    Pending,
    Success,
    Failure,
}

/// Un Pull Request abierto del repositorio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    pub number: i64,
    pub title: String,
    pub author: String,
    pub head: String,
    pub draft: bool,
    pub checks: CheckState,
}

/// ¿Está `gh` disponible en PATH?
pub fn available() -> bool {
    crate::state::App::which_exists("gh")
}

/// Ejecuta `gh` en el repositorio y captura la salida.
async fn gh(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = tokio::process::Command::new("gh")
        .current_dir(repo)
        .args(args)
        .output()
        .await
        .map_err(|err| format!("No se pudo ejecutar gh (GitHub CLI): {err}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = stderr.lines().rev().find(|l| !l.trim().is_empty());
        Err(message.unwrap_or("gh falló").trim().to_owned())
    }
}

/// Severidad para consolidar el rollup (mayor = peor).
fn state_severity(state: CheckState) -> u8 {
    match state {
        CheckState::None => 0,
        CheckState::Pending => 2,
        CheckState::Success => 1,
        CheckState::Failure => 3,
    }
}

/// Extrae el estado peor de los checks (rollup) del JSON del PR.
fn rollup_state(value: &Value) -> CheckState {
    let entries = match value.get("statusCheckRollup") {
        Some(Value::Array(items)) => items.clone(),
        Some(item) => vec![item.clone()],
        None => return CheckState::None,
    };

    let mut worst = CheckState::None;
    for item in entries {
        let raw = item
            .get("conclusion")
            .or_else(|| item.get("state"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let state = match raw.to_uppercase().as_str() {
            "SUCCESS" | "NEUTRAL" => CheckState::Success,
            "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" => CheckState::Failure,
            "PENDING" | "IN_PROGRESS" | "QUEUED" | "EXPECTED" | "ACTION_REQUIRED" => {
                CheckState::Pending
            }
            _ => CheckState::None,
        };
        worst = if state_severity(state) > state_severity(worst) {
            state
        } else {
            worst
        };
    }
    worst
}

/// Lista los PRs abiertos del repositorio (hasta 30).
pub async fn pull_requests(repo: PathBuf) -> Result<Vec<PullRequest>, String> {
    let output = gh(
        &repo,
        &[
            "pr",
            "list",
            "--limit",
            "30",
            "--json",
            "number,title,author,headRefName,isDraft,statusCheckRollup",
        ],
    )
    .await?;

    let parsed: Vec<Value> =
        serde_json::from_str(&output).map_err(|err| format!("JSON inválido: {err}"))?;

    let mut prs = Vec::new();
    for item in parsed {
        let number = item.get("number").and_then(Value::as_i64).unwrap_or(0);
        let title = item
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let author = item
            .pointer("/author/login")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned();
        let head = item
            .get("headRefName")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let draft = item
            .get("isDraft")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        prs.push(PullRequest {
            number,
            title,
            author,
            head,
            draft,
            checks: rollup_state(&item),
        });
    }

    Ok(prs)
}

/// Crea un PR desde la rama actual (gh infiere la base).
pub async fn create_pr(repo: PathBuf, title: String, body: String) -> Result<String, String> {
    let url = gh(&repo, &["pr", "create", "--title", &title, "--body", &body]).await?;
    Ok(url.trim().to_owned())
}

/// Cambia a la rama del PR (crea la rama local si hace falta).
pub async fn checkout_pr(repo: PathBuf, number: i64) -> Result<(), String> {
    gh(&repo, &["pr", "checkout", &number.to_string()])
        .await
        .map(|_| ())
}

/// Abre una URL en el navegador del sistema.
pub async fn open_url(url: String) {
    let _ = open::that(url);
}

/// URL del PR en la web.
pub fn pr_url(slug: Option<(String, String)>, number: i64) -> Option<String> {
    slug.map(|(owner, name)| format!("https://github.com/{owner}/{name}/pull/{number}"))
}

/// Extrae `(owner, repo)` del remoto `origin` si apunta a GitHub.
pub fn repo_slug(path: &PathBuf) -> Option<(String, String)> {
    let output = std::process::Command::new("git")
        .current_dir(path)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let normalized = url
        .trim_start_matches("https://github.com/")
        .trim_start_matches("git@github.com:")
        .trim_end_matches(".git")
        .to_owned();
    let mut parts = normalized.split('/');
    let owner = parts.next()?.to_owned();
    let name = parts.next()?.to_owned();
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some((owner, name))
}

//! Puente al CLI `git` para operaciones de red (fetch/pull/push/clone).
//!
//! ¿Por qué el CLI y no libgit2? (PLAN.md §4.2): ejecutando el `git` del
//! sistema heredamos gratis los **credential helpers** (libsecret en
//! Ubuntu, Git Credential Manager en Windows), ssh-agent, proxies y el
//! progreso real de `--progress`. GitHub Desktop hace exactamente esto
//! con dugite.
//!
//! Las operaciones locales (status, diff, commit, stage…) siguen en
//! libgit2 dentro de [`crate::native`].

use std::path::Path;
use std::process::{Command, Stdio};

/// Tipos de operación de red disponibles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetOpKind {
    /// `git fetch --all --prune`.
    Fetch,
    /// `git pull --no-rebase` (fetch + fast-forward o merge).
    Pull,
    /// `git push`; con `publish` añade `--set-upstream` (primera vez).
    Push { publish: bool },
}

/// Progreso reportado por `git --progress` (vía stderr).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// Fase de git: "Receiving objects", "Writing objects", etc.
    pub phase: String,
    /// Porcentaje (0-100) si la fase lo reporta.
    pub percent: Option<u8>,
}

/// Construye el comando `git` para la operación pedida.
///
/// - Desactiva prompts interactivos (`GIT_TERMINAL_PROMPT=0`) para que una
///   credencial ausente falle rápido con mensaje claro en vez de colgarse.
/// - `stdout` se descarta (el progreso va por stderr); así no se llena el
///   pipe y no hay bloqueos.
pub fn build_command(kind: NetOpKind, repo: &Path, branch: &str) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    match kind {
        NetOpKind::Fetch => {
            command.args(["fetch", "--progress", "--all", "--prune"]);
        }
        NetOpKind::Pull => {
            command.args(["pull", "--progress", "--no-rebase"]);
        }
        NetOpKind::Push { publish: true } => {
            command.args(["push", "--progress", "--set-upstream", "origin", branch]);
        }
        NetOpKind::Push { publish: false } => {
            command.args(["push", "--progress", "origin", branch]);
        }
    }

    command
}

/// Extrae `Progress` de una línea de stderr de `git --progress`.
///
/// git escribe el progreso con separadores `\r`: de una lectura pueden
/// llegar varias actualizaciones; nos quedamos con la última (la más
/// reciente). Ejemplos:
/// - `"Receiving objects:  45% (100/222), 1.2 MiB"` → 45 %
/// - `"Resolving deltas: 100% (5/5), done."` → 100 %
/// - `"remote: Enumerating objects: 12, done."` → `None` (no hay %)
pub fn parse_progress(raw: &str) -> Option<Progress> {
    let fragment = raw.rsplit('\r').next()?.trim();
    let (phase, rest) = fragment.split_once(':')?;
    let percent_str = rest.trim_start().split('%').next()?;
    let percent: u8 = percent_str.trim().parse().ok()?;

    if percent > 100 {
        return None;
    }

    Some(Progress {
        phase: phase.trim().to_owned(),
        percent: Some(percent),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_percent_progress() {
        let p = parse_progress("Receiving objects:  45% (100/222), 1.2 MiB").unwrap();
        assert_eq!(p.phase, "Receiving objects");
        assert_eq!(p.percent, Some(45));
    }

    #[test]
    fn keeps_last_carriage_return_update() {
        let raw = "Resolving deltas:   0% (0/4)\rResolving deltas:  25% (1/4)";
        let p = parse_progress(raw).unwrap();
        assert_eq!(p.phase, "Resolving deltas");
        assert_eq!(p.percent, Some(25));
    }

    #[test]
    fn ignores_lines_without_percent() {
        assert!(parse_progress("remote: Enumerating objects: 12, done.").is_none());
        assert!(parse_progress("From github.com:jorgenio/gitdesktop").is_none());
    }

    #[test]
    fn rejects_out_of_range_percent() {
        assert!(parse_progress("Weird: 250% (2/1)").is_none());
    }

    #[test]
    fn fetch_uses_all_remotes() {
        let cmd = build_command(
            NetOpKind::Fetch,
            Path::new("/tmp"),
            "main",
        );
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec!["fetch", "--progress", "--all", "--prune"]);
    }

    #[test]
    fn publish_push_sets_upstream() {
        let cmd = build_command(
            NetOpKind::Push { publish: true },
            Path::new("/tmp"),
            "feature/x",
        );
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec!["push", "--progress", "--set-upstream", "origin", "feature/x"]
        );
    }
}

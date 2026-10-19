//! Suscripciones: operaciones de red con progreso en vivo.
//!
//! Cada operación se ejecuta como `git --progress` en un subproceso tokio;
//! stderr se lee línea a línea y se traduce a `Message::NetProgress`. El
//! prechequeo (remotos/upstream) usa libgit2 para dar errores claros sin
//! llegar a ejecutar git.

use std::path::PathBuf;

use gitdesktop_core::{NetOpKind, cli};
use iced::stream;
use iced::Subscription;
use iced::futures::SinkExt;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::messages::Message;

/// Suscripción de una operación de red. La identidad es
/// `(ruta, rama, tipo)`; mientras `App.running_op` exista, la suscripción
/// vive y emite progreso.
pub fn network_op(path: PathBuf, branch: String, kind: NetOpKind) -> Subscription<Message> {
    Subscription::run_with((path, branch, kind), |data: &(PathBuf, String, NetOpKind)| {
        let (path, branch, kind) = data.clone();
        stream::channel::<Message>(16, async move |mut output| {
            // 1) Prechequeo con libgit2: mensajes de error claros y rápidos.
            let engine = match gitdesktop_core::open(&path) {
                Ok(engine) => engine,
                Err(err) => {
                    let _ = output.send(Message::NetDone(kind, Err(err.to_string()))).await;
                    return;
                }
            };

            let remotes = match engine.remotes() {
                Ok(remotes) => remotes,
                Err(err) => {
                    let _ = output.send(Message::NetDone(kind, Err(err.to_string()))).await;
                    return;
                }
            };
            if remotes.is_empty() {
                let _ = output
                    .send(Message::NetDone(
                        kind,
                        Err("El repositorio no tiene remotos configurados (git remote add …).".into()),
                    ))
                    .await;
                return;
            }

            if kind == NetOpKind::Pull {
                let has_upstream = engine.status().map(|s| s.has_upstream).unwrap_or(false);
                if !has_upstream {
                    let _ = output
                        .send(Message::NetDone(
                            kind,
                            Err(format!(
                                "La rama «{branch}» no tiene upstream; usa «Publicar rama» primero."
                            )),
                        ))
                        .await;
                    return;
                }
            }
            drop(engine); // libera libgit2 antes de ejecutar el subproceso

            // 2) Ejecuta `git --progress` y transmite el progreso de stderr.
            let mut command =
                tokio::process::Command::from(cli::build_command(kind, &path, &branch));
            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(err) => {
                    let _ = output
                        .send(Message::NetDone(
                            kind,
                            Err(format!("No se pudo ejecutar git: {err}")),
                        ))
                        .await;
                    return;
                }
            };

            let Some(stderr) = child.stderr.take() else {
                let _ = output
                    .send(Message::NetDone(kind, Err("git no devolvió stderr".into())))
                    .await;
                return;
            };

            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            let mut last_meaningful = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if let Some(progress) = cli::parse_progress(&line) {
                            let _ = output.send(Message::NetProgress(progress)).await;
                        } else {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                last_meaningful = trimmed.to_owned();
                            }
                        }
                    }
                }
            }

            match child.wait().await {
                Ok(status) if status.success() => {
                    let _ = output.send(Message::NetDone(kind, Ok(String::new()))).await;
                }
                Ok(status) => {
                    let message = if last_meaningful.is_empty() {
                        format!(
                            "git terminó con código {status}. Revisa la pestaña Cambios: \
                             puede que el merge haya dejado conflictos."
                        )
                    } else {
                        last_meaningful
                    };
                    let _ = output.send(Message::NetDone(kind, Err(message))).await;
                }
                Err(err) => {
                    let _ = output.send(Message::NetDone(kind, Err(err.to_string()))).await;
                }
            }
        })
    })
}

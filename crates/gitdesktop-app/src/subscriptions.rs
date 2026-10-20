//! Suscripciones: watcher del filesystem y operaciones de red con progreso.
//!
//! El watcher usa `notify` con un canal unbounded de tokio (Send de
//! principio a fin); al detectar guardados emite `AutoRefresh`, que pasa
//! por el anti-rebote del estado. El prechequeo de red y el `git
//! --progress` se ejecutan en subprocesos tokio.

use std::path::PathBuf;

use gitdesktop_core::{NetOpKind, cli};
use iced::futures::SinkExt;
use iced::stream;
use iced::Subscription;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::messages::Message;

/// Suscripciones agregadas mientras haya un repositorio abierto:
/// watcher del filesystem + temporizador de auto-fetch.
pub fn repository_activities(path: PathBuf) -> Subscription<Message> {
    Subscription::batch([
        Subscription::run_with(path.clone(), |path: &PathBuf| {
            let path = path.clone();
            watcher_stream(path)
        }),
        iced::time::every(std::time::Duration::from_secs(60)).map(|_| Message::AutoRefresh),
    ])
}

/// Vigila el árbol del repositorio y emite `AutoRefresh` en cada guardado.
/// Ignora cambios dentro de `.git` y `target`.
fn watcher_stream(
    path: PathBuf,
) -> impl iced::futures::Stream<Item = Message> + 'static {
    stream::channel::<Message>(16, async move |mut output| {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();

        let mut watcher = match notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| {
            let relevant = match &res {
                Ok(event) => event.paths.iter().any(|p| {
                    let as_text = p.to_string_lossy();
                    !as_text.contains("/.git/")
                        && !as_text.contains("\\.git\\")
                        && !as_text.contains("/target/")
                        && !as_text.contains("\\target\\")
                }),
                Err(_) => false,
            };
                if relevant {
                    let _ = tx.send(());
                }
            },
        ) {
            Ok(watcher) => watcher,
            Err(_) => return,
        };

        if notify::Watcher::watch(&mut watcher, &path, notify::RecursiveMode::Recursive)
            .is_err()
        {
            return;
        }

        // El watcher vive mientras la suscripción exista (su drop detiene
        // el hilo interno de notify y cierra el canal).
        while rx.recv().await.is_some() {
            // Drena ráfagas de eventos: un solo AutoRefresh por ráfaga.
            while rx.try_recv().is_ok() {}
            if output.send(Message::AutoRefresh).await.is_err() {
                return;
            }
        }
    })
}

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

            if !matches!(kind, NetOpKind::Clone { .. }) {
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
                            Err(
                                "El repositorio no tiene remotos configurados (git remote add …)."
                                    .into(),
                            ),
                        ))
                        .await;
                    return;
                }

                if kind == NetOpKind::Pull {
                    let has_upstream =
                        engine.status().map(|s| s.has_upstream).unwrap_or(false);
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
            }
            drop(engine); // libera libgit2 antes de ejecutar el subproceso

            // 2) Ejecuta `git --progress` y transmite el progreso de stderr.
            let mut command = tokio::process::Command::from(cli::build_command(
                kind.clone(),
                &path,
                &branch,
            ));
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

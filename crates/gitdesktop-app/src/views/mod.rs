//! Vistas: pantalla de bienvenida, vista principal de repositorio y
//! modales (menú, Acerca de, descartar, ramas, merge, clone).

pub mod changes;
pub mod diff_view;
pub mod history;
pub mod titlebar;

use iced::widget::{button, canvas, center, column, container, mouse_area, pick_list, row, rule, scrollable, space, text, text_input, Stack};
use iced::{Alignment, Element, Fill, Length};

use crate::messages::{Message, Tab};
use crate::widgets::conflict_editor::{EditAction, EditorCanvas};
use crate::state::{App, OpenRepo};
use crate::theme;

/// Vista raíz según haya o no un repositorio abierto.
pub fn view(app: &App) -> Element<'_, Message> {
    column![
        titlebar::bar(app),
        notice_banner(app),
        error_banner(app),
        Stack::with_children(vec![
            body(app),
            titlebar::menu_overlay(app),
            modals(app),
        ])
        .width(Fill)
        .height(Fill),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

// ————————————————————————————— cuerpo ————————————————————————————————

fn body(app: &App) -> Element<'_, Message> {
    match &app.repo {
        Some(open) => repository(app, open),
        None => welcome(app),
    }
}

fn repository<'a>(app: &'a App, open: &'a OpenRepo) -> Element<'a, Message> {
    column![
        header(app, open),
        row![
            sidebar(app, open),
            rule::vertical(1),
            main_panel(app),
        ]
        .width(Fill)
        .height(Fill),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

/// Cabecera: repositorio actual · rama · sincronización contextual
/// (Fetch / Pull / Push / Publicar, como GitHub Desktop).
fn header<'a>(app: &'a App, open: &'a OpenRepo) -> Element<'a, Message> {
    container(
        row![
            column![
                text("Repositorio actual").size(10).style(theme::muted),
                text(open.entry.name.clone()).size(15),
            ]
            .spacing(2),
            button(text("Abrir…").size(12))
                .on_press(Message::OpenFolderDialog)
                .padding([6, 10])
                .style(button::text),
            space::horizontal(),
            branch_button(open),
            space::horizontal(),
            sync_button(app, open),
        ]
        .spacing(14)
        .align_y(Alignment::Center)
        .width(Fill),
    )
    .padding([10, 14])
    .style(theme::panel)
    .width(Fill)
    .into()
}

/// Botón contextual de sincronización con progreso e indicador de hora.
fn sync_button<'a>(app: &'a App, open: &'a OpenRepo) -> Element<'a, Message> {
    let status = &open.status;
    let busy = app.running_op.is_some() || app.loading;

    // Subtítulo: progreso en vivo o "última sincronización hace X".
    let subtitle = if let Some(progress) = &app.net_progress {
        match progress.percent {
            Some(percent) => format!("{} · {percent} %", progress.phase),
            None => progress.phase.clone(),
        }
    } else {
        match app.last_fetch {
            Some(at) => format!("Última sincronización: {}", ago_text(at)),
            None => "Sin sincronizar todavía".to_owned(),
        }
    };

    // Acción contextual, exactamente como GitHub Desktop:
    // sin upstream → publicar · behind → pull · ahead → push · resto → fetch.
    let (label, action): (String, Option<gitdesktop_core::NetOpKind>) =
        if app.running_op.is_some() {
            ("Sincronizando…".to_owned(), None)
        } else if !status.has_remote {
            ("Sin remoto".to_owned(), None)
        } else if status.branch.is_empty() {
            ("Sin commits".to_owned(), None)
        } else if !status.has_upstream {
            ("Publicar rama".to_owned(), Some(gitdesktop_core::NetOpKind::Push { publish: true }))
        } else if status.behind > 0 {
            (format!("Pull origin (↓{})", status.behind), Some(gitdesktop_core::NetOpKind::Pull))
        } else if status.ahead > 0 {
            (format!("Push origin (↑{})", status.ahead), Some(gitdesktop_core::NetOpKind::Push { publish: false }))
        } else {
            ("Fetch origin".to_owned(), Some(gitdesktop_core::NetOpKind::Fetch))
        };

    let actionable = action.is_some() && !busy;

    button(
        column![
            text(label).size(14),
            text(subtitle).size(10).style(theme::muted),
        ]
        .spacing(2),
    )
    .on_press_maybe(action.filter(|_| actionable).map(Message::NetStart))
    .padding([6, 14])
    .style(if actionable {
        button::primary
    } else {
        button::text
    })
    .into()
}

/// Botón de rama en la cabecera: abre el gestor de ramas.
fn branch_button(open: &OpenRepo) -> Element<'static, Message> {
    button(
        column![
            text("Rama actual").size(10).style(theme::muted),
            text(branch_label(open)).size(15),
        ]
        .spacing(2),
    )
    .on_press(Message::BranchModalToggled(true))
    .padding([6, 10])
    .style(button::text)
    .into()
}

/// Panel lateral: pestañas Cambios/Historial + contenido.
fn sidebar<'a>(app: &'a App, open: &'a OpenRepo) -> Element<'a, Message> {
    let tabs = row![
        tab_button(app.tab, Tab::Changes, format!("Cambios ({})", open.status.entries.len())),
        tab_button(app.tab, Tab::History, "Historial".to_owned()),
    ]
    .width(Fill);

    let content = match app.tab {
        Tab::Changes => changes::view(app, open),
        Tab::History => history::list(app),
    };

    container(
        column![tabs, rule::horizontal(1), content].width(Fill).height(Fill),
    )
    .style(theme::sidebar)
    .width(Length::Fixed(380.0))
    .height(Fill)
    .into()
}

fn tab_button(active: Tab, this: Tab, label: String) -> Element<'static, Message> {
    button(
        text(label)
            .size(13)
            .align_x(Alignment::Center)
            .width(Fill),
    )
    .on_press(Message::TabSelected(this))
    .padding([9, 12])
    .width(Fill)
    .style(if active == this {
        button::primary
    } else {
        button::text
    })
    .into()
}

fn main_panel(app: &App) -> Element<'_, Message> {
    match app.tab {
        Tab::Changes => diff_view::view(app.diff.as_ref(), app.selected.as_ref()),
        Tab::History => history::detail(app),
    }
}

// ————————————————————————— pantalla de bienvenida —————————————————————————

fn welcome(app: &App) -> Element<'_, Message> {
    let mut recents = column![].spacing(4).max_width(560);

    for entry in app.data.repos.iter().take(8) {
        recents = recents.push(
            button(
                row![
                    text(entry.name.clone()).width(Fill),
                    text(entry.path.display().to_string())
                        .size(11)
                        .style(theme::muted),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .width(Fill),
            )
            .on_press(Message::OpenRecent(entry.id.clone()))
            .padding([8, 12])
            .width(Fill)
            .style(button::text),
        );
    }

    let content = column![
        text("Git-Desktop Jorgenio").size(36),
        text("Tu gestor de Git, rápido y sin terminal.")
            .size(14)
            .style(theme::muted),
        button(
            text("Abrir repositorio…")
                .size(14)
                .align_x(Alignment::Center)
                .width(Fill),
        )
        .on_press(Message::OpenFolderDialog)
        .padding([10, 18])
        .width(Length::Fixed(260.0))
        .style(button::primary),
        recents,
    ]
    .spacing(16)
    .align_x(Alignment::Center);

    center(content).width(Fill).height(Fill).into()
}

// ————————————————————————————— modales ————————————————————————————————

/// Todos los modales, por prioridad.
fn modals(app: &App) -> Element<'_, Message> {
    if app.conflict_editor.is_some() {
        return editor_modal(app);
    }
    if app.about {
        return about_modal();
    }
    if let Some(path) = &app.pending_discard {
        return discard_modal(path);
    }
    if let Some(branch) = &app.pending_delete_branch {
        return delete_branch_modal(branch);
    }
    if app.branch_modal {
        return branch_modal(app);
    }
    if app.merge_modal {
        return merge_modal(app);
    }
    if app.clone_modal {
        return clone_modal(app);
    }
    column![].into()
}

fn modal_scrim(message: Message) -> Element<'static, Message> {
    mouse_area(
        container(space::Space::new().width(Fill).height(Fill))
            .width(Fill)
            .height(Fill)
            .style(theme::scrim),
    )
    .on_press(message)
    .into()
}

fn about_modal() -> Element<'static, Message> {
    let card = container(
        column![
            text("Git-Desktop Jorgenio").size(22),
            text(format!("Versión {}", env!("CARGO_PKG_VERSION")))
                .size(12)
                .style(theme::muted),
            text("Cliente Git de escritorio 100% en Rust,\ncon la filosofía de GitHub Desktop.")
                .size(12)
                .style(theme::muted),
            button(
                text("Cerrar")
                    .size(13)
                    .align_x(Alignment::Center)
                    .width(Fill),
            )
            .on_press(Message::AboutToggled(false))
            .padding([8, 0])
            .width(Fill)
            .style(button::primary),
        ]
        .spacing(12)
        .max_width(340),
    )
    .padding(24)
    .style(theme::panel);

    Stack::with_children(vec![
        modal_scrim(Message::AboutToggled(false)).into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

fn discard_modal(path: &str) -> Element<'static, Message> {
    let path_owned = path.to_owned();

    let card = container(
        column![
            text("¿Descartar los cambios?").size(18),
            text(format!(
                "Se perderán los cambios sin preparar de:\n{path_owned}"
            ))
            .size(12)
            .style(theme::muted),
            row![
                dialog_button("Cancelar", Message::CancelDiscard, button::text),
                dialog_button("Descartar", Message::ConfirmDiscard, button::danger),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(380),
    )
    .padding(24)
    .style(theme::panel);

    Stack::with_children(vec![
        modal_scrim(Message::CancelDiscard).into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

/// Confirmación de borrado de rama.
fn delete_branch_modal(branch: &str) -> Element<'static, Message> {
    let card = container(
        column![
            text("¿Eliminar la rama?").size(18),
            text(format!(
                "Se eliminará la rama local «{branch}».\nLos commits podrían perderse si no están fusionados."
            ))
            .size(12)
            .style(theme::muted),
            row![
                dialog_button("Cancelar", Message::BranchCancelDelete, button::text),
                dialog_button("Eliminar", Message::BranchDeleteConfirmed, button::danger),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(380),
    )
    .padding(24)
    .style(theme::panel);

    Stack::with_children(vec![
        modal_scrim(Message::BranchCancelDelete).into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

/// Gestor de ramas: crear, cambiar y eliminar.
fn branch_modal(app: &App) -> Element<'_, Message> {
    let can_create = !app.new_branch_name.trim().is_empty();

    let mut list = column![].spacing(2);
    for branch in &app.branches {
        let mut item = row![
            text(if branch.is_current {
                format!("✓ {}", branch.name)
            } else {
                branch.name.clone()
            })
            .size(13)
            .width(Fill),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        if !branch.is_current {
            item = item.push(
                button(text("Usar").size(11))
                    .on_press(Message::BranchSwitch(branch.name.clone()))
                    .padding([3, 8])
                    .style(theme::menu_item),
            );
            item = item.push(
                button(text("Borrar").size(11).style(theme::muted))
                    .on_press(Message::BranchDeleteRequested(branch.name.clone()))
                    .padding([3, 8])
                    .style(theme::menu_item),
            );
        }
        list = list.push(container(item).width(Fill).padding([3, 4]));
    }

    let card = container(
        column![
            text("Ramas").size(18),
            row![
                text_input("nombre-de-la-rama", &app.new_branch_name)
                    .on_input(Message::NewBranchNameChanged)
                    .on_submit(Message::BranchCreatePressed)
                    .size(13)
                    .width(Fill),
                button(
                    text("Crear")
                        .size(12)
                        .align_x(Alignment::Center)
                        .width(Fill),
                )
                .on_press_maybe(if can_create {
                    Some(Message::BranchCreatePressed)
                } else {
                    None
                })
                .padding([8, 12])
                .style(button::primary),
            ]
            .spacing(8),
            rule::horizontal(1),
            scrollable(list).height(Length::Fixed(240.0)),
            dialog_button("Cerrar", Message::BranchModalToggled(false), button::text),
        ]
        .spacing(12)
        .max_width(420),
    )
    .padding(20)
    .style(theme::panel);

    Stack::with_children(vec![
        modal_scrim(Message::BranchModalToggled(false)).into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

/// Modal de fusión: elige la rama a integrar en la actual.
fn merge_modal(app: &App) -> Element<'_, Message> {
    let current = app
        .repo
        .as_ref()
        .map(|open| open.status.branch.clone())
        .unwrap_or_default();

    let others: Vec<String> = app
        .branches
        .iter()
        .filter(|branch| !branch.is_current)
        .map(|branch| branch.name.clone())
        .collect();

    let card = container(
        column![
            text(format!("Fusionar en «{current}»")).size(18),
            text("Elige la rama cuyos cambios quieres integrar.")
                .size(12)
                .style(theme::muted),
            pick_list(
                others,
                app.merge_target.clone(),
                |name: String| Message::MergeTargetPicked(Some(name)),
            )
            .width(Fill),
            row![
                dialog_button("Cancelar", Message::MergeModalToggled(false), button::text),
                dialog_button(
                    "Fusionar",
                    Message::MergePressed,
                    button::primary,
                ),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(380),
    )
    .padding(24)
    .style(theme::panel);

    Stack::with_children(vec![
        modal_scrim(Message::MergeModalToggled(false)).into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

/// Modal de clonación: URL + carpeta de destino + progreso.
fn clone_modal(app: &App) -> Element<'_, Message> {
    let busy = app.running_op.is_some();
    let can_clone = !app.clone_url.trim().is_empty() && app.clone_folder.is_some() && !busy;

    let progress = if let Some(progress) = &app.net_progress {
        match progress.percent {
            Some(percent) => format!("{} · {percent} %", progress.phase),
            None => progress.phase.clone(),
        }
    } else if busy {
        "Clonando…".to_owned()
    } else {
        match &app.clone_folder {
            Some(folder) => format!("Destino: {}", folder.display()),
            None => String::new(),
        }
    };

    let card = container(
        column![
            text("Clonar repositorio").size(18),
            text("Pega la URL (HTTPS o SSH) y elige dónde clonarlo.")
                .size(12)
                .style(theme::muted),
            text_input("https://github.com/usuario/repo.git", &app.clone_url)
                .on_input(Message::CloneUrlChanged)
                .on_submit(Message::ClonePressed)
                .size(13),
            button(
                text("Elegir carpeta de destino…")
                    .size(12)
                    .align_x(Alignment::Center)
                    .width(Fill),
            )
            .on_press_maybe(if busy {
                None
            } else {
                Some(Message::ClonePickFolder)
            })
            .padding([8, 0])
            .width(Fill)
            .style(button::text),
            if progress.is_empty() {
                Element::<Message>::from(column![])
            } else {
                text(progress).size(11).style(theme::muted).into()
            },
            row![
                dialog_button("Cancelar", Message::CloneModalToggled(false), button::text),
                button(
                    text("Clonar")
                        .size(13)
                        .align_x(Alignment::Center)
                        .width(Fill),
                )
                .on_press_maybe(if can_clone {
                    Some(Message::ClonePressed)
                } else {
                    None
                })
                .padding([8, 0])
                .width(Fill)
                .style(button::primary),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(440),
    )
    .padding(24)
    .style(theme::panel);

    Stack::with_children(vec![
        modal_scrim(Message::CloneModalToggled(false)).into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

/// Botón estándar de diálogo (ancho completo, texto centrado).
fn dialog_button(
    label: &'static str,
    message: Message,
    style: fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style,
) -> Element<'static, Message> {
    button(
        text(label)
            .size(13)
            .align_x(Alignment::Center)
            .width(Fill),
    )
    .on_press(message)
    .padding([8, 0])
    .width(Fill)
    .style(style)
    .into()
}

/// Editor de conflictos embebido (canvas + jorgenio-core).
fn editor_modal(app: &App) -> Element<'_, Message> {
    let Some(editor) = &app.conflict_editor else {
        return column![].into();
    };

    // Sin clic-fuera para cerrar: evita perder ediciones por accidente.
    let scrim = container(space::Space::new().width(Fill).height(Fill))
        .width(Fill)
        .height(Fill)
        .style(theme::scrim);

    let caret_line = editor.buffer.char_to_line(editor.caret.min(editor.buffer.len_chars()));
    let status = format!(
        "Línea {} · columna {} · {}{}",
        caret_line + 1,
        editor.caret_col(),
        if editor.buffer.dirty() {
            "SIN GUARDAR"
        } else {
            "guardado"
        },
        editor.path,
    );

    let card = container(
        column![
            row![
                text("Resolver conflicto").size(17).width(Fill),
                text(editor.path.clone()).size(11).style(theme::muted),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            container(
                canvas::Canvas::new(EditorCanvas::new(editor))
                    .width(Fill)
                    .height(Length::Fixed(400.0)),
            )
            .width(Fill)
            .style(theme::sidebar),
            text(status).size(11).style(theme::muted),
            row![
                dialog_button(
                    "Cancelar (Esc)",
                    Message::EditorMsg(EditAction::Close),
                    button::text,
                ),
                dialog_button(
                    "Guardar",
                    Message::EditorMsg(EditAction::Save),
                    button::text,
                ),
                dialog_button(
                    "Guardar y marcar resuelto (Ctrl+S)",
                    Message::EditorMsg(EditAction::SaveAndResolve),
                    button::primary,
                ),
            ]
            .spacing(8),
        ]
        .spacing(10)
        .max_width(980),
    )
    .padding(16)
    .style(theme::panel);

    Stack::with_children(vec![
        scrim.into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

// ————————————————————————————— banners ———————————————————————————————— 

/// Banner de aviso (resultados de merge, etc.).
fn notice_banner(app: &App) -> Element<'_, Message> {
    let Some(notice) = &app.notice else {
        return column![].into();
    };

    container(
        row![
            text(notice.clone()).size(12).style(theme::notice).width(Fill),
            button(text("X").size(11))
                .on_press(Message::ClearNotice)
                .padding([2, 8])
                .style(button::text),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([6, 14])
    .width(Fill)
    .into()
}

/// Banner de error descartable (fila de altura cero si no hay error).
fn error_banner(app: &App) -> Element<'_, Message> {
    let Some(err) = &app.error else {
        return column![].into();
    };

    container(
        row![
            text(err.clone()).size(12).style(theme::error).width(Fill),
            button(text("X").size(11))
                .on_press(Message::ClearError)
                .padding([2, 8])
                .style(button::text),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([6, 14])
    .width(Fill)
    .into()
}

// ————————————————————————————— utilidades ————————————————————————————————

/// Etiqueta de la rama actual en la cabecera.
fn branch_label(open: &OpenRepo) -> String {
    if open.status.head_detached {
        format!("{} (detached)", open.status.branch)
    } else if open.status.branch.is_empty() {
        "(sin commits aún)".to_owned()
    } else {
        open.status.branch.clone()
    }
}

/// Texto humano para "última sincronización hace X".
fn ago_text(then: std::time::Instant) -> String {
    let seconds = then.elapsed().as_secs();
    if seconds < 45 {
        "hace unos segundos".to_owned()
    } else if seconds < 3600 {
        format!("hace {} min", seconds / 60)
    } else {
        format!("hace {} h", seconds / 3600)
    }
}

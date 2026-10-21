//! Vistas: pantalla de bienvenida, vista principal de repositorio y
//! modales (menú, Acerca de, descartar, ramas, merge, clone).

pub mod changes;
pub mod diff_view;
pub mod history;
pub mod prs;
pub mod titlebar;

use iced::widget::{button, canvas, center, column, container, mouse_area, pick_list, row, rule, scrollable, space, text, text_input, Stack};
use iced::{Alignment, Color, Element, Fill, Length, Padding, Theme};

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
            repo_menu_overlay(app),
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
    let section = |content: Element<'a, Message>| -> Element<'a, Message> {
        container(content)
            .padding([8, 16])
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .into()
    };

    container(
        row![
            section(repo_button(app, open)),
            vertical_separator(),
            section(branch_button(open)),
            space::horizontal(),
            container(sync_button(app, open)).padding([0, 12]),
        ]
        .height(Length::Fixed(56.0))
        .align_y(Alignment::Center)
        .width(Fill),
    )
    .width(Fill)
    .style(theme::header_bar)
    .into()
}

/// Selector de repositorio con ▾ (como «Current Repository» de GHD).
fn repo_button<'a>(app: &'a App, open: &'a OpenRepo) -> Element<'a, Message> {
    button(
        column![
            text("Repositorio actual").size(10).style(theme::muted),
            row![
                text(open.entry.name.clone()).size(15),
                text(if app.repo_menu_open { "▴" } else { "▾" })
                    .size(11)
                    .style(theme::muted),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        ]
        .spacing(2),
    )
    .on_press(Message::RepoMenuToggled(!app.repo_menu_open))
    .padding([0, 0])
    .style(theme::ghost)
    .into()
}

/// Hairline vertical entre secciones de la cabecera.
fn vertical_separator() -> Element<'static, Message> {
    container(space::Space::new().width(1).height(Length::Fill))
        .height(Length::Fill)
        .style(theme::separator)
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

    column![
        button(
            text(label)
                .size(13)
                .align_x(Alignment::Center)
                .width(Fill),
        )
        .on_press_maybe(action.filter(|_| actionable).map(Message::NetStart))
        .padding([7, 18])
        .width(Fill)
        .style(if actionable {
            theme::primary
        } else {
            theme::text_btn
        }),
        text(subtitle).size(10).style(theme::muted),
    ]
    .spacing(3)
    .width(Length::Fixed(230.0))
    .align_x(Alignment::Center)
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
    .padding([0, 0])
    .style(theme::ghost)
    .into()
}

/// Panel lateral: pestañas Cambios/Historial + contenido.
fn sidebar<'a>(app: &'a App, open: &'a OpenRepo) -> Element<'a, Message> {
    let tabs = container(
        row![
            tab_button(app.tab, Tab::Changes, format!("Cambios ({})", open.status.entries.len())),
            tab_button(app.tab, Tab::History, "Historial".to_owned()),
            tab_button(app.tab, Tab::PullRequests, "PRs".to_owned()),
        ]
        .width(Fill),
    )
    .width(Fill)
    .style(theme::header_bar);

    let content = match app.tab {
        Tab::Changes => changes::view(app, open),
        Tab::History => history::list(app),
        Tab::PullRequests => prs::list(app),
    };

    container(
        column![
            tabs,
            hairline(),
            content,
            hairline(),
            stash_section(app),
        ]
        .width(Fill)
        .height(Fill),
    )
    .style(theme::sidebar)
    .width(Length::Fixed(380.0))
    .height(Fill)
    .into()
}

/// Hairline horizontal (1px).
fn hairline() -> Element<'static, Message> {
    container(space::Space::new().width(Fill).height(1))
        .width(Fill)
        .style(theme::separator)
        .into()
}

/// Sección «Stashes» (como GitHub Desktop): crear/listar/aplicar/pop/drop.
fn stash_section<'a>(app: &'a App) -> Element<'a, Message> {
    let header = button(
        row![
            text(format!("Stashes ({})", app.stashes.len()))
                .size(12)
                .width(Fill),
            text(if app.stashes_visible { "⌄" } else { "›" }).size(12),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .width(Fill),
    )
    .on_press(Message::StashesToggled(!app.stashes_visible))
    .padding([7, 10])
    .width(Fill)
    .style(theme::text_btn);

    if !app.stashes_visible {
        return header.into();
    }

    let mut list = column![].spacing(2).padding([4, 8]);
    if app.stashes.is_empty() {
        list = list.push(
            text("Sin stashes guardados.")
                .size(11)
                .style(theme::muted)
                .width(Fill),
        );
    }
    for stash in &app.stashes {
        list = list.push(
            column![
                text(stash.message.clone())
                    .size(11)
                    .width(Fill)
                    .style(theme::muted),
                row![
                    button(text("Aplicar").size(10))
                        .on_press(Message::StashApply(stash.reference.clone()))
                        .padding([2, 6])
                        .style(theme::menu_item),
                    button(text("Pop").size(10))
                        .on_press(Message::StashPop(stash.reference.clone()))
                        .padding([2, 6])
                        .style(theme::menu_item),
                    button(text("Descartar").size(10).style(theme::muted))
                        .on_press(Message::StashDropRequested(stash.reference.clone()))
                        .padding([2, 6])
                        .style(theme::menu_item),
                ]
                .spacing(4),
            ]
            .spacing(2)
            .padding([3, 2]),
        );
    }

    column![
        header,
        scrollable(list).height(Length::Fixed(140.0)),
    ]
    .width(Fill)
    .into()
}

fn tab_button(active: Tab, this: Tab, label: String) -> Element<'static, Message> {
    let is_active = active == this;

    button(
        column![
            text(label)
                .size(13)
                .align_x(Alignment::Center)
                .width(Fill),
            // Subrayado del tab activo (como GitHub Desktop).
            container(space::Space::new().width(Fill).height(Length::Fixed(3.0)))
                .width(Fill)
                .style(move |theme: &Theme| container::Style {
                    background: Some(if is_active {
                        theme::colors(theme).accent.into()
                    } else {
                        Color::TRANSPARENT.into()
                    }),
                    ..container::Style::default()
                }),
        ]
        .spacing(6)
        .width(Fill),
    )
    .on_press(Message::TabSelected(this))
    .padding([9, 12])
    .width(Fill)
    .style(theme::menu_button(false))
    .into()
}

fn placeholder_vacio() -> Element<'static, Message> {
    column![].into()
}

fn main_panel(app: &App) -> Element<'_, Message> {
    match app.tab {
        Tab::Changes => match (&app.blame, &app.repo) {
            (Some((path, lines)), _) => diff_view::blame_view(path, lines),
            (None, Some(open)) => diff_view::view(
                &open.status,
                app.diff.as_ref(),
                app.selected.as_ref(),
            ),
            (None, None) => placeholder_vacio(),
        },
        Tab::History => history::detail(app),
        Tab::PullRequests => prs::detail(app),
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
            .style(theme::text_btn),
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
        .style(theme::primary),
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
    if let Some(reference) = &app.pending_drop_stash {
        return drop_stash_modal(reference);
    }
    if app.branch_modal {
        return branch_modal(app);
    }
    if app.merge_modal {
        return merge_modal(app);
    }
    if app.status_rebasing() {
        return rebase_paused_modal(app);
    }
    if app.clone_modal {
        return clone_modal(app);
    }
    if app.pr_modal {
        return prs::create_modal(app);
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
            .style(theme::primary),
        ]
        .spacing(12)
        .max_width(340),
    )
    .padding(24)
    .style(theme::card);

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
                dialog_button("Cancelar", Message::CancelDiscard, theme::text_btn),
                dialog_button("Descartar", Message::ConfirmDiscard, theme::danger_btn),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(380),
    )
    .padding(24)
    .style(theme::card);

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
                dialog_button("Cancelar", Message::BranchCancelDelete, theme::text_btn),
                dialog_button("Eliminar", Message::BranchDeleteConfirmed, theme::danger_btn),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(380),
    )
    .padding(24)
    .style(theme::card);

    Stack::with_children(vec![
        modal_scrim(Message::BranchCancelDelete).into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

/// Confirmación de descarte de stash.
fn drop_stash_modal(reference: &str) -> Element<'static, Message> {
    let card = container(
        column![
            text("¿Descartar el stash?").size(18),
            text(format!(
                "Se eliminará {reference} definitivamente\ny sus cambios se perderán."
            ))
            .size(12)
            .style(theme::muted),
            row![
                dialog_button("Cancelar", Message::StashCancelDrop, theme::text_btn),
                dialog_button("Descartar", Message::StashDropConfirmed, theme::danger_btn),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(380),
    )
    .padding(24)
    .style(theme::card);

    Stack::with_children(vec![
        modal_scrim(Message::StashCancelDrop).into(),
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
                    .width(Fill)
                    .style(theme::input),
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
                .style(theme::primary),
            ]
            .spacing(8),
            rule::horizontal(1),
            scrollable(list).height(Length::Fixed(240.0)),
            dialog_button("Cerrar", Message::BranchModalToggled(false), theme::text_btn),
        ]
        .spacing(12)
        .max_width(420),
    )
    .padding(20)
    .style(theme::card);

    Stack::with_children(vec![
        modal_scrim(Message::BranchModalToggled(false)).into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

/// Modal de integración: fusión o rebase según el modo.
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

    let (title, verb, on_go) = match app.merge_mode {
        crate::messages::MergeMode::Merge => (
            format!("Fusionar en «{current}»"),
            "Fusionar",
            Message::MergePressed,
        ),
        crate::messages::MergeMode::Rebase => (
            format!("Rebase de «{current}» sobre…"),
            "Rebase",
            Message::RebasePressed,
        ),
    };

    let card = container(
        column![
            text(title).size(18),
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
                dialog_button("Cancelar", Message::MergeModalToggled(false), theme::text_btn),
                dialog_button(&verb, on_go, theme::primary),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(380),
    )
    .padding(24)
    .style(theme::card);

    Stack::with_children(vec![
        modal_scrim(Message::MergeModalToggled(false)).into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

/// Modal cuando un rebase quedó pausado con conflictos.
fn rebase_paused_modal(_app: &App) -> Element<'_, Message> {
    let card = container(
        column![
            text("Rebase pausado").size(18),
            text(
                "Quedan conflictos sin resolver. Edítalos y márcalos como                  resueltos (checkbox); luego pulsa «Continuar rebase» o aborta."
            )
            .size(12)
            .style(theme::muted),
            row![
                dialog_button(
                    "Abortar rebase",
                    Message::RebaseAbortPressed,
                    theme::danger_btn,
                ),
                dialog_button(
                    "Continuar rebase",
                    Message::RebaseContinuePressed,
                    theme::primary,
                ),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(420),
    )
    .padding(24)
    .style(theme::card);

    Stack::with_children(vec![center(card).width(Fill).height(Fill).into()]).into()
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
                .size(13)
                .style(theme::input),
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
            .style(theme::text_btn),
            if progress.is_empty() {
                Element::<Message>::from(column![])
            } else {
                text(progress).size(11).style(theme::muted).into()
            },
            row![
                dialog_button("Cancelar", Message::CloneModalToggled(false), theme::text_btn),
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
                .style(theme::primary),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(440),
    )
    .padding(24)
    .style(theme::card);

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
                    theme::text_btn,
                ),
                dialog_button(
                    "Guardar",
                    Message::EditorMsg(EditAction::Save),
                    theme::text_btn,
                ),
                dialog_button(
                    "Guardar y marcar resuelto (Ctrl+S)",
                    Message::EditorMsg(EditAction::SaveAndResolve),
                    theme::primary,
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
                .style(theme::text_btn),
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
                .style(theme::text_btn),
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

/// Menú de repositorios recientes (bajo la sección de cabecera).
fn repo_menu_overlay(app: &App) -> Element<'_, Message> {
    if !app.repo_menu_open {
        return column![].into();
    }

    let scrim = mouse_area(
        container(space::Space::new().width(Fill).height(Fill))
            .width(Fill)
            .height(Fill)
            .style(theme::scrim),
    )
    .on_press(Message::RepoMenuToggled(false));

    let mut panel = column![].width(Length::Fixed(320.0)).padding(6);

    for entry in app.data.repos.iter().take(10) {
        let is_current = app
            .repo
            .as_ref()
            .map(|open| open.entry.id == entry.id)
            .unwrap_or(false);

        panel = panel.push(
            button(
                row![
                    text(if is_current {
                        format!("✓ {}", entry.name)
                    } else {
                        entry.name.clone()
                    })
                    .size(13)
                    .width(Fill),
                ],
            )
            .on_press(Message::OpenRecent(entry.id.clone()))
            .padding([7, 10])
            .width(Fill)
            .style(theme::menu_item),
        );
    }

    panel = panel.push(
        button(text("Abrir carpeta…").size(13))
            .on_press(Message::OpenFolderDialog)
            .padding([7, 10])
            .width(Fill)
            .style(theme::menu_item),
    );

    let dropdown = container(panel)
        .padding(Padding {
            top: 2.0,
            right: 0.0,
            bottom: 0.0,
            left: 6.0,
        })
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Start)
        .align_y(Alignment::Start)
        .style(theme::dropdown);

    Stack::with_children(vec![scrim.into(), dropdown.into()]).into()
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

//! Vistas: pantalla de bienvenida, vista principal de repositorio y
//! modales (menú desplegable, Acerca de, confirmación de descarte).

pub mod changes;
pub mod diff_view;
pub mod history;
pub mod titlebar;

use iced::widget::{button, center, column, container, mouse_area, row, rule, space, text, Stack};
use iced::{Alignment, Element, Fill, Length, Padding};

use crate::messages::{Message, Tab};
use crate::state::{App, OpenRepo};
use crate::theme;

/// Vista raíz según haya o no un repositorio abierto.
pub fn view(app: &App) -> Element<'_, Message> {
    column![
        titlebar::bar(app),
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
            column![
                text("Rama actual").size(10).style(theme::muted),
                text(branch_label(open)).size(15),
            ]
            .spacing(2),
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
    .on_press_maybe(
        action
            .filter(|_| actionable)
            .map(Message::NetStart),
    )
    .padding([6, 14])
    .style(if actionable {
        button::primary
    } else {
        button::text
    })
    .into()
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
    .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    })
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
        button(text("Abrir repositorio…").size(14).align_x(Alignment::Center).width(Fill))
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

/// Acerca de… y confirmación de descarte.
fn modals(app: &App) -> Element<'_, Message> {
    if app.about {
        return about_modal();
    }
    if let Some(path) = &app.pending_discard {
        return discard_modal(path);
    }
    column![].into()
}

fn about_modal() -> Element<'static, Message> {
    let scrim = mouse_area(
        container(space::Space::new().width(Fill).height(Fill))
            .width(Fill)
            .height(Fill)
            .style(theme::scrim),
    )
    .on_press(Message::AboutToggled(false));

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
        scrim.into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

fn discard_modal(path: &str) -> Element<'static, Message> {
    let _ = path; // el mensaje lleva la ruta capturada abajo
    let path_owned = path.to_owned();

    let scrim = mouse_area(
        container(space::Space::new().width(Fill).height(Fill))
            .width(Fill)
            .height(Fill)
            .style(theme::scrim),
    )
    .on_press(Message::CancelDiscard);

    let card = container(
        column![
            text("¿Descartar los cambios?").size(18),
            text(format!("Se perderán los cambios sin preparar de:\n{path_owned}"))
                .size(12)
                .style(theme::muted),
            row![
                button(
                    text("Cancelar")
                        .size(13)
                        .align_x(Alignment::Center)
                        .width(Fill),
                )
                .on_press(Message::CancelDiscard)
                .padding([8, 0])
                .width(Fill)
                .style(button::text),
                button(
                    text("Descartar")
                        .size(13)
                        .align_x(Alignment::Center)
                        .width(Fill),
                )
                .on_press(Message::ConfirmDiscard)
                .padding([8, 0])
                .width(Fill)
                .style(button::danger),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .max_width(380),
    )
    .padding(24)
    .style(theme::panel);

    Stack::with_children(vec![
        scrim.into(),
        center(card).width(Fill).height(Fill).into(),
    ])
    .into()
}

// ————————————————————————————— utilidades ————————————————————————————————

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

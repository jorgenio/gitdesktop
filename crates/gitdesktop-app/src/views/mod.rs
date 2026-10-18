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
    row![
        sidebar(app, open),
        rule::vertical(1),
        main_panel(app),
    ]
    .width(Fill)
    .height(Fill)
    .into()
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

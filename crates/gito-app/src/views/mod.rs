//! Vistas: pantalla de bienvenida y vista principal de repositorio
//! (maqueta de GitHub Desktop: cabecera, panel lateral, diff y estado).

pub mod changes;
pub mod history;

use iced::widget::{button, center, column, container, horizontal_space, rule, row, scrollable, text};
use iced::{Alignment, Element, Fill, Length};

use crate::messages::{Message, Tab};
use crate::state::{App, OpenRepo};
use crate::theme;

/// Vista raíz según haya o no un repositorio abierto.
pub fn view(app: &App) -> Element<'_, Message> {
    match &app.repo {
        Some(open) => repository(app, open),
        None => welcome(app),
    }
}

// ————————————————————————— Pantalla de bienvenida —————————————————————————

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
        text("Gito").size(42),
        text("Tu gestor de Git, rápido y sin terminal.")
            .size(14)
            .style(theme::muted),
        button(text("Abrir repositorio…").size(14))
            .on_press(Message::OpenFolderDialog)
            .padding([10, 18])
            .style(button::primary),
        recents,
    ]
    .spacing(16)
    .align_x(Alignment::Center);

    column![error_banner(app), center(content)]
        .width(Fill)
        .height(Fill)
        .into()
}

// ————————————————————————— Vista de repositorio ——————————————————————————

fn repository(app: &App, open: &OpenRepo) -> Element<'_, Message> {
    column![
        header(app, open),
        error_banner(app),
        row![sidebar(app, open), rule::vertical(1), main_panel(open)]
            .width(Fill)
            .height(Fill),
        status_bar(app, open),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

/// Cabecera: repositorio actual · rama · Pull origin (como el screenshot).
fn header(app: &App, open: &OpenRepo) -> Element<'_, Message> {
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
            horizontal_space(),
            column![
                text("Rama actual").size(10).style(theme::muted),
                text(branch_label(open)).size(15),
            ]
            .spacing(2),
            horizontal_space(),
            button(
                column![
                    text("Pull origin").size(14),
                    text(sync_label(open)).size(10).style(theme::muted),
                ]
                .spacing(2)
            )
            .padding([6, 10])
            .style(button::text),
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

/// Panel lateral: pestañas Cambios/Historial + contenido.
fn sidebar(app: &App, open: &OpenRepo) -> Element<'_, Message> {
    let tabs = row![
        tab_button(app.tab, Tab::Changes, format!("Cambios ({})", open.status.entries.len())),
        tab_button(app.tab, Tab::History, "Historial".to_owned()),
    ]
    .width(Fill);

    let body = match app.tab {
        Tab::Changes => changes::view(&open.status),
        Tab::History => history::view(open),
    };

    container(
        column![tabs, rule::horizontal(1), scrollable(body).height(Fill)]
            .width(Fill)
            .height(Fill),
    )
    .style(theme::sidebar)
    .width(Length::Fixed(380.0))
    .height(Fill)
    .into()
}

fn tab_button(active: Tab, this: Tab, label: String) -> Element<'static, Message> {
    button(text(label).size(13))
        .on_press(Message::TabSelected(this))
        .padding([8, 12])
        .width(Fill)
        .style(if active == this {
            button::primary
        } else {
            button::text
        })
        .into()
}

/// Panel principal: placeholder del visor de diff (se construye en M1).
fn main_panel(open: &OpenRepo) -> Element<'_, Message> {
    center(
        column![
            text("Vista de diff — llega en M1").size(18),
            text(format!(
                "Aquí se mostrará el diff unificado de «{}» con resaltado de sintaxis.",
                open.entry.name
            ))
            .size(12)
            .style(theme::muted),
        ]
        .spacing(8)
        .max_width(520)
        .align_x(Alignment::Center),
    )
    .width(Fill)
    .height(Fill)
    .into()
}

/// Barra de estado inferior: ruta, contadores y refresco manual.
fn status_bar(app: &App, open: &OpenRepo) -> Element<'_, Message> {
    let state_text = if app.loading {
        "Calculando estado…".to_owned()
    } else {
        format!(
            "{} preparados · {} sin preparar · ↑{} ↓{}",
            open.status.staged_count(),
            open.status.unstaged_count(),
            open.status.ahead,
            open.status.behind
        )
    };

    container(
        row![
            text(open.entry.path.display().to_string())
                .size(11)
                .style(theme::muted),
            horizontal_space(),
            text(state_text).size(11).style(theme::muted),
            button(text("Refrescar").size(11))
                .on_press(Message::Refresh)
                .padding([4, 8])
                .style(button::text),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .width(Fill),
    )
    .padding([6, 14])
    .style(theme::panel)
    .width(Fill)
    .into()
}

// —————————————————————————————— Utilidades ———————————————————————————————

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

fn branch_label(open: &OpenRepo) -> String {
    if open.status.head_detached {
        format!("{} (detached)", open.status.branch)
    } else if open.status.branch.is_empty() {
        "(sin commits aún)".to_owned()
    } else {
        open.status.branch.clone()
    }
}

fn sync_label(open: &OpenRepo) -> String {
    match (open.status.ahead, open.status.behind) {
        (0, 0) => "sincronizado con el remoto".to_owned(),
        (a, 0) => format!("{a} commit(s) sin publicar"),
        (0, b) => format!("{b} commit(s) para actualizar"),
        (a, b) => format!("{a} sin publicar · {b} para actualizar"),
    }
}

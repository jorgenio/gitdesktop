//! Vista «Cambios»: lista interactiva de archivos (stage/unstage/discard)
//! + panel de commit con resumen y descripción.

use iced::alignment;
use iced::widget::{button, center, checkbox, column, container, row, scrollable, text, text_editor, text_input};
use iced::{Alignment, Element, Fill, Length};

use gitdesktop_core::FileEntry;

/// Botón de fila de diálogo con estado habilitado opcional.
fn dialog_button_inline<'a>(
    label: &'static str,
    message: Message,
    style: fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style,
    enabled: bool,
) -> Element<'a, Message> {
    button(
        text(label)
            .size(12)
            .align_x(Alignment::Center)
            .width(Fill),
    )
    .on_press_maybe(enabled.then_some(message))
    .padding([8, 0])
    .width(Fill)
    .style(style)
    .into()
}

use crate::messages::Message;
use crate::state::{App, OpenRepo};
use crate::theme;

/// Lista de archivos (panel izquierdo, bajo las pestañas).
pub fn view<'a>(app: &'a App, open: &'a OpenRepo) -> Element<'a, Message> {
    if open.status.entries.is_empty() {
        return center(
            column![
                text("Todo limpio").size(18),
                text("No hay cambios locales en este repositorio.")
                    .size(12)
                    .style(theme::muted),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .into();
    }

    let mut list = column![].spacing(1).padding([8, 4]);

    for entry in &open.status.entries {
        list = list.push(file_row(app, entry));
    }

    column![
        scrollable(list).height(Fill),
        commit_panel(app, open),
    ]
    .height(Fill)
    .into()
}

fn file_row<'a>(app: &'a App, entry: &'a FileEntry) -> Element<'a, Message> {
    let is_selected = app
        .selected
        .as_ref()
        .map(|(path, _)| path.as_str())
        == Some(entry.path.as_str());

    // Fila como GitHub Desktop: checkbox sencillo + ruta (una línea).
    // Las acciones (descartar/editar) viven en el header del diff.
    let row = row![
        checkbox(entry.staged)
            .size(15)
            .on_toggle(move |checked| Message::ToggleStaged {
                path: entry.path.clone(),
                staged: checked,
            }),
        button(
            container(text(truncate_path(&entry.path, 28)).size(13))
                .width(Fill)
                .height(Length::Fixed(18.0))
                .clip(true),
        )
        .on_press(Message::FileSelected {
            path: entry.path.clone(),
            staged: entry.staged,
        })
        .padding([2, 4])
        .width(Fill)
        .style(theme::ghost),
        text(entry.status.label())
            .size(11)
            .style(theme::muted)
            .width(Length::Fixed(88.0))
            .align_x(alignment::Horizontal::Right),
    ]
    .spacing(10)
    .padding([6, 8])
    .align_y(Alignment::Center)
    .width(Fill);

    container_row(row, is_selected)
}

/// Trunca rutas largas al estilo GitHub Desktop: inicio + … + final.
fn truncate_path(path: &str, max: usize) -> String {
    if path.chars().count() <= max {
        return path.to_owned();
    }
    let head = 18;
    let tail = max - head - 1;
    let chars: Vec<char> = path.chars().collect();
    format!(
        "{}…{}",
        chars[..head].iter().collect::<String>(),
        chars[chars.len() - tail..].iter().collect::<String>()
    )
}

fn container_row<'a>(
    content: impl Into<iced::Element<'a, Message>>,
    selected: bool,
) -> Element<'a, Message> {
    iced::widget::container(content)
        .width(Fill)
        .height(Length::Fixed(40.0))
        .padding([2, 6])
        .style(theme::file_row(selected))
        .into()
}

/// Panel inferior: resumen + descripción + botón de commit (o de merge
/// cuando hay una fusión en curso con conflictos).
fn commit_panel<'a>(app: &'a App, open: &'a OpenRepo) -> Element<'a, Message> {
    let merging = open.status.merging;
    let rebasing = open.status.rebasing;
    let reverting = open.status.reverting;
    let cherry_picking = open.status.cherry_picking;
    let can_commit = open.status.staged_count() > 0
        && !app.summary.trim().is_empty()
        && !merging
        && !rebasing;

    let branch = if open.status.branch.is_empty() {
        "main".to_owned()
    } else {
        open.status.branch.clone()
    };

    let merge_row: Element<'a, Message> = if merging {
        row![
            dialog_button_inline("Confirmar merge", Message::MergeCommitPressed, button::primary, open.status.staged_count() > 0),
            dialog_button_inline("Abortar", Message::MergeAborted, button::danger, true),
        ]
        .spacing(8)
        .into()
    } else if rebasing {
        row![
            dialog_button_inline(
                "Continuar rebase",
                Message::RebaseContinuePressed,
                button::primary,
                open.status.staged_count() > 0,
            ),
            dialog_button_inline("Abortar rebase", Message::RebaseAbortPressed, button::danger, true),
        ]
        .spacing(8)
        .into()
    } else if reverting || cherry_picking {
        row![dialog_button_inline(
            "Continuar",
            Message::ContinueOpPressed,
            button::primary,
            open.status.staged_count() > 0,
        )]
        .spacing(8)
        .into()
    } else {
        column![].into()
    };

    let commit_button_label = if merging {
        "Commit normal deshabilitado durante el merge".to_owned()
    } else if rebasing {
        "Commit normal deshabilitado durante el rebase".to_owned()
    } else {
        format!("Hacer commit en {branch}")
    };

    column![
        text_input("Resumen (obligatorio)", &app.summary)
            .on_input(Message::SummaryChanged)
            .size(13)
            .padding([8, 10])
            .style(theme::input),
        container(
            text_editor(&app.description)
                .on_action(Message::DescriptionEdited)
                .size(12)
                .height(Length::Fixed(84.0)),
        )
        .width(Fill)
        .padding([4, 4])
        .style(theme::input_box),
        merge_row,
        row![
            button(
                text("Guardar en stash")
                    .size(11)
                    .align_x(Alignment::Center)
                    .width(Fill),
            )
            .on_press_maybe(
                if open.status.entries.is_empty() || merging || rebasing {
                    None
                } else {
                    Some(Message::StashCreatePressed)
                },
            )
            .padding([7, 0])
            .width(Fill)
            .style(button::text),
        ],
        button(
            text(commit_button_label)
                .size(13)
                .align_x(Alignment::Center)
                .width(Fill),
        )
        .on_press_maybe(if can_commit {
            Some(Message::CommitPressed)
        } else {
            None
        })
        .padding([10, 0])
        .width(Fill)
        .style(button::primary),
    ]
    .spacing(8)
    .padding([10, 10])
    .into()
}

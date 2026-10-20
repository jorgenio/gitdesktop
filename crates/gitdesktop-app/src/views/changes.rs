//! Vista «Cambios»: lista interactiva de archivos (stage/unstage/discard)
//! + panel de commit con resumen y descripción.

use iced::alignment;
use iced::widget::{button, center, checkbox, column, row, scrollable, text, text_editor, text_input};
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

    // El nombre selecciona el archivo (su diff usa staged o no según la fila).
    let name = button(text(entry.path.clone()).size(13).width(Fill))
        .on_press(Message::FileSelected {
            path: entry.path.clone(),
            staged: entry.staged,
        })
        .padding([4, 4])
        .width(Fill)
        .style(theme::ghost);

    let mut controls = row![
        checkbox(entry.staged)
            .label(entry.path.clone())
            .size(15)
            .on_toggle(move |checked| Message::ToggleStaged {
                path: entry.path.clone(),
                staged: checked,
            }),
        name,
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    // En conflicto: editor embebido (canvas + jorgenio-core)…
    if entry.status == gitdesktop_core::FileStatus::Conflicted {
        controls = controls.push(
            button(text("editar aquí").size(10).style(theme::notice))
                .on_press(Message::EditConflictPressed {
                    path: entry.path.clone(),
                })
                .padding([3, 6])
                .style(theme::menu_item),
        );
        // …o abrir en Jorgenio Text como ventana propia.
        controls = controls.push(
            button(text("jorgenio ↗").size(10).style(theme::muted))
                .on_press(Message::ResolveInEditor {
                    path: entry.path.clone(),
                })
                .padding([3, 6])
                .style(theme::menu_item),
        );
    }

    // Descartar solo tiene sentido para cambios sin preparar.
    if !entry.staged {
        controls = controls.push(
            button(text("descartar").size(10).style(theme::muted))
                .on_press(Message::RequestDiscard {
                    path: entry.path.clone(),
                })
                .padding([3, 6])
                .style(theme::menu_item),
        );
    }

    controls = controls.push(
        text(entry.status.label())
            .size(11)
            .style(theme::muted)
            .width(Length::Fixed(92.0))
            .align_x(alignment::Horizontal::Right),
    );

    container_row(controls.width(Fill), is_selected)
}

fn container_row<'a>(
    content: impl Into<iced::Element<'a, Message>>,
    selected: bool,
) -> Element<'a, Message> {
    iced::widget::container(content)
        .width(Fill)
        .padding([2, 6])
        .style(theme::file_row(selected))
        .into()
}

/// Panel inferior: resumen + descripción + botón de commit (o de merge
/// cuando hay una fusión en curso con conflictos).
fn commit_panel<'a>(app: &'a App, open: &'a OpenRepo) -> Element<'a, Message> {
    let merging = open.status.merging;
    let rebasing = open.status.rebasing;
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
            .padding([8, 10]),
        text_editor(&app.description)
            .on_action(Message::DescriptionEdited)
            .size(12)
            .height(Length::Fixed(84.0)),
        merge_row,
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

//! Vista «Historial»: lista de commits (izquierda) + detalle con archivos
//! y diff del commit seleccionado (panel derecho).

use iced::widget::{button, canvas, column, container, row, scrollable, text};
use crate::widgets::commit_graph::ROW_H;
use iced::{Alignment, Element, Fill, Length};

use time::macros::format_description;

use crate::messages::Message;
use crate::state::App;
use crate::theme;
use crate::views::diff_view;

const DATE_FORMAT: &[time::format_description::FormatItem<'static>] =
    format_description!("[day] [month repr:short] [year] [hour repr:24]:[minute]");

/// Lista de commits (panel izquierdo, bajo las pestañas).
pub fn list(app: &App) -> Element<'_, Message> {
    if app.history.is_empty() {
        return iced::widget::center(
            column![
                text("Sin commits todavía").size(18),
                text("El historial aparecerá al hacer el primer commit.")
                    .size(12)
                    .style(theme::muted),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .into();
    }

    let mut list = column![].spacing(0).padding([8, 4]);

    for commit in app.history.iter() {
        let is_selected = app.selected_commit.as_deref() == Some(commit.id.as_str());

        let date = commit
            .time
            .format(&DATE_FORMAT)
            .unwrap_or_else(|_| commit.time.to_string());

        list = list.push(
            container(
                button(
                    column![
                        text(commit.summary.clone()).size(13).width(Fill),
                        text(format!("{} · {} · {}", commit.author, date, commit.short_id))
                            .size(11)
                            .style(theme::muted),
                    ]
                    .spacing(2)
                    .width(Fill),
                )
                .on_press(Message::CommitSelected(commit.id.clone()))
                .padding([4, 8])
                .width(Fill)
                .height(Length::Fixed(ROW_H - 0.0))
                .style(theme::ghost),
            )
            .width(Fill)
            .style(theme::file_row(is_selected)),
        );
    }

    // Grafo (canvas) y lista comparten el MISMO scrollable para
    // desplazarse en sincronía.
    let graph = crate::widgets::commit_graph::GraphCanvas {
        rows: &app.graph_rows,
    };
    let graph = container(canvas::Canvas::new(graph))
        .width(Length::Fixed(44.0))
        .style(theme::sidebar);

    scrollable(
        row![graph, list].width(Fill),
    )
    .height(Fill)
    .into()
}

/// Detalle del commit seleccionado (panel derecho).
pub fn detail(app: &App) -> Element<'_, Message> {
    let Some(commit) = app
        .history
        .iter()
        .find(|c| Some(c.id.as_str()) == app.selected_commit.as_deref())
    else {
        return iced::widget::center(
            text("Selecciona un commit del historial.")
                .size(14)
                .style(theme::muted),
        )
        .into();
    };

    let date = commit
        .time
        .format(&DATE_FORMAT)
        .unwrap_or_else(|_| commit.time.to_string());

    let header = container(
        column![
            text(commit.summary.clone()).size(15),
            text(format!("{} · {} · {}", commit.author, date, commit.short_id))
                .size(11)
                .style(theme::muted),
        ]
        .spacing(3),
    )
    .padding([10, 12])
    .width(Fill)
    .style(move |t| theme::file_row(true)(t));

    // Columna de archivos del commit.
    let mut files = column![].spacing(1).width(Length::Fixed(280.0));
    for entry in &app.commit_files {
        let is_selected =
            app.selected_commit_file.as_deref() == Some(entry.path.as_str());

        files = files.push(
            container(
                button(text(entry.path.clone()).size(12).width(Fill))
                    .on_press(Message::CommitFileSelected(entry.path.clone()))
                    .padding([5, 8])
                    .width(Fill)
                    .style(theme::ghost),
            )
            .width(Fill)
            .style(theme::file_row(is_selected)),
        );
    }
    let files_panel = container(scrollable(files).height(Fill))
        .width(Length::Fixed(300.0))
        .height(Fill)
        .style(theme::sidebar);

    // Diff del archivo seleccionado del commit.
    let diff_panel: Element<'_, Message> = match (&app.selected_commit_file, &app.commit_diff) {
        (Some(file), Some(lines)) => {
            let _ = file;
            diff_view::lines_view(lines)
        }
        (Some(file), None) => iced::widget::center(
            text(format!("Calculando diff de {file}…"))
                .size(13)
                .style(theme::muted),
        )
        .into(),
        _ => iced::widget::center(
            text("Selecciona un archivo del commit.")
                .size(14)
                .style(theme::muted),
        )
        .into(),
    };

    column![
        header,
        row![
            files_panel,
            iced::widget::rule::vertical(1),
            container(diff_panel).width(Fill).height(Fill),
        ]
        .width(Fill)
        .height(Fill),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

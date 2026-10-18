//! Visor de diff unificado: números de línea, signo y color por tipo.

use iced::widget::{center, column, container, row, scrollable, text};
use iced::{alignment, Element, Fill, Font, Length};

use gitdesktop_core::{DiffLine, DiffLineKind};

use crate::messages::Message;
use crate::theme;

/// Vista principal del panel derecho para la pestaña Cambios.
pub fn view<'a>(
    diff: Option<&'a Vec<DiffLine>>,
    selected: Option<&'a (String, bool)>,
) -> Element<'a, Message> {
    let Some((path, staged)) = selected else {
        return placeholder("Selecciona un archivo de la lista para ver su diff.");
    };

    let Some(lines) = diff else {
        let hint = if *staged {
            format!("Calculando diff preparado de {path}…")
        } else {
            format!("Calculando diff de {path}…")
        };
        return placeholder(hint);
    };

    column![
        container(text(path.clone()).size(13))
            .padding([8, 12])
            .width(Fill)
            .style(move |t| theme::file_row(true)(t)),
        scrollable(lines_view(lines)).height(Fill),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

/// Columna de líneas coloreadas (reutilizada por el historial).
pub fn lines_view(lines: &[DiffLine]) -> Element<'_, Message> {
    if lines.is_empty() {
        return placeholder("Sin cambios que mostrar para este archivo.");
    }

    let mut list = column![].width(Fill);
    for line in lines {
        list = list.push(line_row(line));
    }
    container(list)
        .width(Fill)
        .padding([2, 0])
        .into()
}

fn line_row(line: &DiffLine) -> Element<'_, Message> {
    let sign = match line.kind {
        DiffLineKind::Added => "+",
        DiffLineKind::Deleted => "-",
        DiffLineKind::HunkHeader => "@",
        DiffLineKind::Context => " ",
    };

    let sign_style = match line.kind {
        DiffLineKind::Added => theme::added,
        DiffLineKind::Deleted => theme::deleted,
        _ => theme::muted,
    };

    container(
        row![
            number(line.old_no),
            number(line.new_no),
            text(sign)
                .size(12)
                .font(Font::MONOSPACE)
                .width(Length::Fixed(20.0))
                .style(sign_style),
            text(line.content.trim_end_matches(['\n', '\r']))
                .size(12)
                .font(Font::MONOSPACE)
                .width(Fill),
        ]
        .width(Fill),
    )
    .width(Fill)
    .style(theme::diff_row(line.kind))
    .into()
}

fn number(value: Option<u32>) -> Element<'static, Message> {
    text(value.map(|v| v.to_string()).unwrap_or_default())
        .size(11)
        .style(theme::muted)
        .width(Length::Fixed(44.0))
        .align_x(alignment::Horizontal::Right)
        .into()
}

fn placeholder(message: impl Into<String>) -> Element<'static, Message> {
    center(
        text(message.into())
            .size(14)
            .style(theme::muted)
            .align_x(alignment::Horizontal::Center),
    )
    .width(Fill)
    .height(Fill)
    .into()
}

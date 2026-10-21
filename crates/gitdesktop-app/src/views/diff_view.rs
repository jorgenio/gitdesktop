//! Visor de diff unificado: números de línea, signo y color por tipo.

use iced::widget::{button, center, column, container, row, scrollable, text};
use iced::{alignment, Color, Element, Fill, Font, Length, Theme};

use gitdesktop_core::{DiffLine, DiffLineKind};

use crate::messages::Message;
use crate::theme;

/// Vista principal del panel derecho para la pestaña Cambios.
pub fn view<'a>(
    status: &'a gitdesktop_core::RepoStatus,
    diff: Option<&'a Vec<DiffLine>>,
    selected: Option<&'a (String, bool)>,
) -> Element<'a, Message> {
    let Some((path, staged)) = selected else {
        return placeholder("Selecciona un archivo de la lista para ver su diff.");
    };

    let entry = status
        .entries
        .iter()
        .find(|entry| entry.path == *path && entry.staged == *staged);

    let Some(lines) = diff else {
        let hint = if *staged {
            format!("Calculando diff preparado de {path}…")
        } else {
            format!("Calculando diff de {path}…")
        };
        return placeholder(hint);
    };

    // Acciones contextuales según el estado del archivo seleccionado.
    let mut actions = row![
        button(text("blame").size(11).style(theme::muted))
            .on_press(Message::BlameRequested { path: path.clone() })
            .padding([3, 8])
            .style(theme::menu_item),
    ]
    .spacing(6);

    if let Some(entry) = entry {
        if entry.status == gitdesktop_core::FileStatus::Conflicted {
            actions = actions.push(
                button(text("editar aquí").size(11).style(theme::notice))
                    .on_press(Message::EditConflictPressed { path: path.clone() })
                    .padding([3, 8])
                    .style(theme::menu_item),
            );
            actions = actions.push(
                button(text("jorgenio ↗").size(11).style(theme::muted))
                    .on_press(Message::ResolveInEditor { path: path.clone() })
                    .padding([3, 8])
                    .style(theme::menu_item),
            );
        } else if !entry.staged {
            actions = actions.push(
                button(text("descartar").size(11).style(theme::muted))
                    .on_press(Message::RequestDiscard { path: path.clone() })
                    .padding([3, 8])
                    .style(theme::menu_item),
            );
        }
    }

    column![
        container(
            row![
                text(path.clone()).size(13).width(Fill),
                actions,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
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

    // Dos tonos como GitHub Desktop: columna de números con fondo
    // «gutter» distinto + contenido con el fondo del tipo de línea.
    container(
        row![
            container(row![number(line.old_no), number(line.new_no)].width(Fill))
                .width(Length::Fixed(92.0))
                .height(Length::Fill)
                .style(move |theme: &Theme| {
                    let c = theme::colors(theme);
                    let background = match line.kind {
                        gitdesktop_core::DiffLineKind::Added => c.add_gutter,
                        gitdesktop_core::DiffLineKind::Deleted => c.del_gutter,
                        gitdesktop_core::DiffLineKind::HunkHeader => c.hunk_bg,
                        gitdesktop_core::DiffLineKind::Context => c.panel,
                    };
                    container::Style {
                        background: Some(background.into()),
                        ..container::Style::default()
                    }
                }),
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
        .height(Length::Fixed(20.0))
        .width(Fill),
    )
    .width(Fill)
    .height(Length::Fixed(20.0))
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

/// Vista `git blame`: SHA · autor · contenido por línea.
pub fn blame_view<'a>(
    path: &'a str,
    lines: &'a [gitdesktop_core::BlameLine],
) -> Element<'a, Message> {
    let mut list = column![].width(Fill);
    for line in lines {
        let color = lane_color(&line.short_commit);
        list = list.push(
            container(
                row![
                    text(line.short_commit.clone())
                        .size(11)
                        .font(Font::MONOSPACE)
                        .width(Length::Fixed(62.0))
                        .style(move |_t| text::Style { color: Some(color) }),
                    text(line.author.clone())
                        .size(11)
                        .width(Length::Fixed(120.0))
                        .style(theme::muted),
                    text(line.content.trim_end_matches(['\n', '\r']))
                        .size(12)
                        .font(Font::MONOSPACE)
                        .width(Fill),
                ]
                .width(Fill)
                .align_y(iced::Alignment::Center),
            )
            .width(Fill)
            .style(theme::file_row(false)),
        );
    }

    column![
        container(
            row![
                text(path.to_owned()).size(13).width(Fill),
                button(text("← volver al diff").size(11).style(theme::muted))
                    .on_press(Message::BlameClosed)
                    .padding([2, 8])
                    .style(theme::menu_item),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding([8, 12])
        .width(Fill)
        .style(move |t| theme::file_row(true)(t)),
        scrollable(list).height(Fill),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

/// Color estable a partir del SHA del commit.
fn lane_color(sha: &str) -> Color {
    const PALETTE: [Color; 5] = [
        Color::from_rgb8(0x2F, 0x81, 0xF7),
        Color::from_rgb8(0x3F, 0xB9, 0x50),
        Color::from_rgb8(0xF8, 0x51, 0x49),
        Color::from_rgb8(0xA3, 0x71, 0xF7),
        Color::from_rgb8(0xD2, 0x99, 0x22),
    ];
    let byte = sha.bytes().next().unwrap_or(0);
    PALETTE[(byte % PALETTE.len() as u8) as usize]
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

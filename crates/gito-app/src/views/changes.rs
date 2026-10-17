//! Vista «Cambios»: lista de archivos modificados con checkbox de stage.
//!
//! M0: el checkbox es informativo (el toggle real de stage/unstage llega
//! en M1 junto con `git add`/`git reset` en el núcleo).

use iced::widget::{center, checkbox, column, horizontal_space, row, text};
use iced::{Alignment, Element, Fill};

use gito_core::{FileEntry, RepoStatus};

use crate::messages::Message;
use crate::theme;

pub fn view(status: &RepoStatus) -> Element<'_, Message> {
    if status.entries.is_empty() {
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

    let mut list = column![].spacing(2).padding([8, 6]);

    for entry in &status.entries {
        list = list.push(file_row(entry));
    }

    list.into()
}

fn file_row(entry: &FileEntry) -> Element<'_, Message> {
    row![
        checkbox(entry.staged).label(entry.path.clone()).size(16),
        horizontal_space(),
        text(entry.status.label()).size(11).style(theme::muted),
    ]
    .spacing(8)
    .padding([4, 4])
    .align_y(Alignment::Center)
    .into()
}

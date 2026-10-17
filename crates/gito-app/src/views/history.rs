//! Vista «Historial»: placeholder hasta M2 (lista de commits + diff).

use iced::widget::{center, column, text};
use iced::{Alignment, Element};

use crate::messages::Message;
use crate::state::OpenRepo;
use crate::theme;

pub fn view(_open: &OpenRepo) -> Element<'_, Message> {
    center(
        column![
            text("Historial — llega en M2").size(18),
            text("La lista de commits con su diff se construye en el hito M2 del plan.")
                .size(12)
                .style(theme::muted),
        ]
        .spacing(6)
        .align_x(Alignment::Center),
    )
    .into()
}

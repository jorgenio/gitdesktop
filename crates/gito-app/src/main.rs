//! Gito — cliente Git de escritorio en Rust (Ubuntu + Windows).
//!
//! Punto de entrada de la UI: iced 0.14 con arquitectura Elm.
//! El plan completo del proyecto está en `PLAN.md` (raíz del repo).

mod messages;
mod state;
mod theme;
mod views;

use messages::Message;
use state::App;

pub fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(|_state| theme::gito())
        .window_size((1280.0, 800.0))
        .centered()
        .run()
}

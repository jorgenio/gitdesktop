//! Git-Desktop Jorgenio — cliente Git de escritorio en Rust (Ubuntu + Windows).
//!
//! Punto de entrada de la UI: iced 0.14 con arquitectura Elm.
//! El plan completo del proyecto está en `PLAN.md` (raíz del repo).

mod messages;
mod state;
mod subscriptions;
mod theme;
mod views;

use state::App;

pub fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(|state: &App| theme::current(&state.data.ui.theme))
        .window(iced::window::Settings {
            icon: Some(load_icon()),
            ..Default::default()
        })
        .window_size((1280.0, 800.0))
        // Sin decoraciones nativas: barra de título personalizada, como
        // GitHub Desktop para Windows (menús + controles propios).
        .decorations(false)
        .subscription(App::subscription)
        .centered()
        .run()
}

/// Icono de la app para la ventana/barra de tareas (PNG embebido).
fn load_icon() -> iced::window::Icon {
    let bytes = include_bytes!("../../../assets/icon-128.png");
    let image = image::load_from_memory(bytes)
        .expect("el icono embebido debe ser un PNG válido")
        .to_rgba8();
    let (width, height) = image.dimensions();
    iced::window::icon::from_rgba(image.into_raw(), width, height)
        .expect("el icono embebido tiene el tamaño declarado")
}

//! Barra de título personalizada (sin decoraciones nativas), estilo
//! GitHub Desktop para Windows: menús desplegables + título arrastrable
//! + botones de ventana.

use iced::widget::{button, column, container, mouse_area, row, space, text, Stack};
use iced::{Alignment, Element, Fill, Length, Padding};

use crate::messages::{MenuAction, Message};
use crate::state::App;
use crate::theme;

/// Ancho fijo de cada botón de menú (determina la posición del desplegable).
const MENU_WIDTH: f32 = 86.0;
/// Margen izquierdo del grupo de menús.
const MENU_MARGIN: f32 = 6.0;

/// Un ítem de menú: activo (con acción) o deshabilitado (llega en hitos
/// posteriores, como indican los sufijos).
pub struct MenuItem {
    label: &'static str,
    action: Option<MenuAction>,
}

impl MenuItem {
    const fn action(label: &'static str, action: MenuAction) -> Self {
        Self { label, action: Some(action) }
    }

    const fn disabled(label: &'static str) -> Self {
        Self { label, action: None }
    }
}

/// Menús del título, como GitHub Desktop (Archivo, Editar, Ver…).
const MENUS: [(&str, &[MenuItem]); 6] = [
    (
        "Archivo",
        &[
            MenuItem::action("Abrir repositorio…", MenuAction::OpenRepository),
            MenuItem::action("Clonar repositorio…", MenuAction::Clone),
            MenuItem::action("Salir", MenuAction::Quit),
        ],
    ),
    (
        "Editar",
        &[
            MenuItem::disabled("Deshacer (próximamente)"),
            MenuItem::disabled("Rehacer (próximamente)"),
            MenuItem::disabled("Preferencias (próximamente)"),
        ],
    ),
    (
        "Ver",
        &[
            MenuItem::action("Cambios", MenuAction::ShowChanges),
            MenuItem::action("Historial", MenuAction::ShowHistory),
            MenuItem::action("Alternar tema claro/oscuro", MenuAction::ToggleTheme),
        ],
    ),
    (
        "Repositorio",
        &[
            MenuItem::action("Refrescar", MenuAction::Refresh),
            MenuItem::disabled("Abrir en el explorador (próximamente)"),
            MenuItem::disabled("Quitar de la lista (próximamente)"),
        ],
    ),
    (
        "Rama",
        &[
            MenuItem::action("Nueva rama…", MenuAction::NewBranch),
            MenuItem::action("Fusionar rama en la actual…", MenuAction::MergeBranches),
            MenuItem::action("Rebase sobre rama…", MenuAction::RebaseBranches),
            MenuItem::disabled("Renombrar rama… (fase 2)"),
        ],
    ),
    (
        "Ayuda",
        &[
            MenuItem::action("Acerca de Git-Desktop Jorgenio", MenuAction::About),
            MenuItem::disabled("Ver el plan del proyecto (próximamente)"),
        ],
    ),
];

// ————————————————————————————— barra ————————————————————————————————

/// Fila superior: menús + zona de arrastre con el título + controles.
pub fn bar(app: &App) -> Element<'_, Message> {
    row![menu_buttons(app), drag_area(app), window_controls()]
        .height(Length::Fixed(40.0))
        .align_y(Alignment::Center)
        .width(Fill)
        .into()
}

fn menu_buttons(app: &App) -> Element<'_, Message> {
    let mut menus = row![].spacing(0).padding(Padding {
        left: MENU_MARGIN,
        right: 0.0,
        top: 0.0,
        bottom: 0.0,
    });

    for (index, (label, _items)) in MENUS.iter().enumerate() {
        let is_open = app.open_menu == Some(index);
        menus = menus.push(
            button(
                text(*label)
                    .size(12)
                    .align_x(Alignment::Center)
                    .width(Fill),
            )
            .on_press(Message::MenuToggled(Some(index)))
            .padding([8, 8])
            .width(Length::Fixed(MENU_WIDTH))
            .height(Fill)
            .style(theme::menu_button(is_open)),
        );
    }

    menus.into()
}

/// Espacio flexible con el título: pulsarlo inicia el arrastre de la ventana.
fn drag_area(app: &App) -> Element<'_, Message> {
    let title = match &app.repo {
        Some(open) => open.entry.name.clone(),
        None => "Git-Desktop Jorgenio".to_owned(),
    };

    button(
        text(title)
            .size(12)
            .style(theme::muted)
            .align_x(Alignment::Center)
            .width(Fill),
    )
    .on_press(Message::TitleBarPressed)
    .padding(8)
    .width(Fill)
    .height(Fill)
    .style(theme::ghost)
    .into()
}

fn window_controls() -> Element<'static, Message> {
    let control = |glyph: &'static str, message: Message, danger: bool| {
        button(
            text(glyph)
                .size(13)
                .align_x(Alignment::Center)
                .width(Fill),
        )
        .on_press(message)
        .padding([6, 0])
        .width(Length::Fixed(42.0))
        .height(Fill)
        .style(theme::window_button(danger))
    };

    row![
        control("–", Message::MinimizeWindow, false),
        control("□", Message::ToggleMaximize, false),
        control("×", Message::CloseWindow, true),
    ]
    .height(Fill)
    .into()
}

// ————————————————————————— desplegable —————————————————————————

/// Overlay del menú abierto: velo + panel de ítems bajo el botón.
/// Debe renderizarse dentro de un `Stack` sobre el cuerpo de la app.
pub fn menu_overlay(app: &App) -> Element<'_, Message> {
    let Some(index) = app.open_menu else {
        return column![].into();
    };

    // Velo que cierra el menú al hacer clic fuera.
    let scrim = mouse_area(space::Space::new().width(Fill).height(Fill))
        .on_press(Message::MenuToggled(None));

    let (_label, items) = MENUS[index];
    let mut panel = column![].width(Length::Fixed(260.0)).padding(6);

    for item in items {
        panel = panel.push(match item.action {
            Some(action) => button(text(item.label).size(13).width(Fill))
                .on_press(Message::MenuSelected(action))
                .padding([8, 12])
                .width(Fill)
                .style(theme::menu_item),
            None => button(
                text(item.label)
                    .size(13)
                    .style(theme::muted)
                    .width(Fill),
            )
            .padding([8, 12])
            .width(Fill)
            .style(theme::menu_item_disabled),
        });
    }

    // Posición bajo el botón correspondiente (ancho fijo por botón).
    let left = MENU_MARGIN + index as f32 * MENU_WIDTH;

    let dropdown = container(panel)
        .padding(Padding {
            top: 2.0,
            right: 0.0,
            bottom: 0.0,
            left,
        })
        .align_x(Alignment::Start)
        .align_y(Alignment::Start)
        .width(Fill)
        .height(Fill)
        .style(theme::sidebar);

    Stack::with_children(vec![scrim.into(), dropdown.into()]).into()
}

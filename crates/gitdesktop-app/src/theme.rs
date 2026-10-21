//! Sistema de diseño de Git-Desktop Jorgenio — flat, sin bordes.
//!
//! Separación por fondos sutiles + hairlines; sombras solo para elementos
//! flotantes (menús, modales). Los botones reaccionan al hover (Status).

use iced::widget::{button, container, text};
use iced::{Background, Border, Color, Shadow, Theme};
use iced::theme::{Base, Palette};

use gitdesktop_core::DiffLineKind;

/// Paleta completa de la app.
#[derive(Debug, Clone, Copy)]
pub struct GitoColors {
    pub canvas: Color,     // fondo general
    pub header: Color,     // barra superior (ligeramente distinta)
    pub panel: Color,      // sidebar / listas
    pub panel_soft: Color, // hover
    pub selected: Color,   // fila seleccionada
    pub separator: Color,  // hairlines
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub success: Color,
    pub danger: Color,
    pub warning: Color,
    pub add_bg: Color,
    pub add_gutter: Color, // columna de números en líneas añadidas
    pub del_bg: Color,
    pub del_gutter: Color, // columna de números en líneas eliminadas
    pub hunk_bg: Color,
    pub add_text: Color,
    pub del_text: Color,
    pub shadow: Color,
}

/// GitHub Dark.
pub const DARK: GitoColors = GitoColors {
    canvas: hex(0x0D_11_17),
    header: hex(0x01_04_09),
    panel: hex(0x16_1B_22),
    panel_soft: hex(0x21_26_2D),
    selected: hex(0x2D_33_3B),
    separator: hex(0x21_26_2D),
    text: hex(0xE6_ED_F3),
    muted: hex(0x7D_85_90),
    accent: hex(0x2F_81_F7),
    success: hex(0x3F_B9_50),
    danger: hex(0xF8_51_49),
    warning: hex(0xD2_99_22),
    add_bg: hex(0x12_2B_1E),
    add_gutter: hex(0x0C_20_16),
    del_bg: hex(0x3C_1D_20),
    del_gutter: hex(0x2E_16_19),
    hunk_bg: hex(0x1B_2B_4A),
    add_text: hex(0x7E_E7_87),
    del_text: hex(0xFF_7B_72),
    shadow: Color { a: 0.5, ..Color::BLACK },
};

/// GitHub Light.
pub const LIGHT: GitoColors = GitoColors {
    canvas: hex(0xFF_FF_FF),
    header: hex(0xF6_F8_FA),
    panel: hex(0xF6_F8_FA),
    panel_soft: hex(0xEA_ED_F0),
    selected: hex(0xDD_E4_EA),
    separator: hex(0xE7_EC_F0),
    text: hex(0x1F_23_28),
    muted: hex(0x59_63_6E),
    accent: hex(0x09_69_DA),
    success: hex(0x1A_7F_37),
    danger: hex(0xCF_22_2E),
    warning: hex(0x9A_67_00),
    add_bg: hex(0xE6_FF_EC),
    add_gutter: hex(0xCC_FF_D8),
    del_bg: hex(0xFF_EB_E9),
    del_gutter: hex(0xFF_D7_D5),
    hunk_bg: hex(0xDD_F4_FF),
    add_text: hex(0x1A_7F_37),
    del_text: hex(0xCF_22_2E),
    shadow: Color { a: 0.12, ..Color::BLACK },
};

const fn hex(value: u32) -> Color {
    Color {
        r: ((value >> 16) & 0xff) as f32 / 255.0,
        g: ((value >> 8) & 0xff) as f32 / 255.0,
        b: (value & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

/// Colores activos según el tema de iced en curso.
pub fn colors(theme: &Theme) -> &'static GitoColors {
    if theme.name() == "gitdesktop-light" {
        &LIGHT
    } else {
        &DARK
    }
}

/// Tema base de la app a partir del valor persistido.
pub fn current(theme_name: &str) -> Theme {
    let palette = match theme_name {
        "light" => Palette {
            background: LIGHT.canvas,
            text: LIGHT.text,
            primary: LIGHT.accent,
            success: LIGHT.success,
            danger: LIGHT.danger,
            warning: LIGHT.warning,
        },
        _ => Palette {
            background: DARK.canvas,
            text: DARK.text,
            primary: DARK.accent,
            success: DARK.success,
            danger: DARK.danger,
            warning: DARK.warning,
        },
    };

    let name = match theme_name {
        "light" => "gitdesktop-light",
        _ => "gitdesktop-dark",
    };

    Theme::custom(name, palette)
}

/// Campo de texto con borde redondeado (única excepción con borde, como
/// el box «Description» de GitHub Desktop). Focused → borde del acento.
pub fn input(theme: &Theme, status: iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    let c = colors(theme);
    let border_color = match status {
        iced::widget::text_input::Status::Focused { .. } => c.accent,
        _ => c.separator,
    };
    iced::widget::text_input::Style {
        background: Background::Color(c.canvas),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 3.0.into(),
        },
        icon: c.muted,
        value: c.text,
        placeholder: c.muted,
        selection: c.accent,
    }
}

/// Caja que envuelve editores multilínea (mismo look que input).
pub fn input_box(theme: &Theme) -> container::Style {
    let c = colors(theme);
    container::Style {
        background: Some(c.canvas.into()),
        border: Border {
            color: c.separator,
            width: 1.0,
            radius: 3.0.into(),
        },
        ..container::Style::default()
    }
}

// ——————————————————————————— Contenedores ————————————————————————————

/// Barra de cabecera: fondo propio, sin bordes.
pub fn header_bar(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(colors(theme).header.into()),
        ..container::Style::default()
    }
}

/// Panel genérico (sidebar, listas): fondo plano, sin bordes.
pub fn panel(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(colors(theme).panel.into()),
        ..container::Style::default()
    }
}

/// Alias de panel (columna lateral).
pub fn sidebar(theme: &Theme) -> container::Style {
    panel(theme)
}

/// Hairline vertical u horizontal (separadores de 1px).
pub fn separator(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(colors(theme).separator.into()),
        ..container::Style::default()
    }
}

/// Fila de archivo: base; la selección la resalta el hover del botón.
pub fn file_row(selected: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme: &Theme| {
        let c = colors(theme);
        container::Style {
            background: Some(if selected { c.selected } else { c.panel }.into()),
            ..container::Style::default()
        }
    }
}

/// Fondo de una línea de diff según su tipo.
pub fn diff_row(kind: DiffLineKind) -> impl Fn(&Theme) -> container::Style {
    move |theme: &Theme| {
        let c = colors(theme);
        let background = match kind {
            DiffLineKind::Added => c.add_bg,
            DiffLineKind::Deleted => c.del_bg,
            DiffLineKind::HunkHeader => c.hunk_bg,
            DiffLineKind::Context => c.panel,
        };
        container::Style {
            background: Some(background.into()),
            ..container::Style::default()
        }
    }
}

/// Velo detrás de menús y modales (se adapta al tema).
pub fn scrim(theme: &Theme) -> container::Style {
    let c = colors(theme);
    container::Style {
        background: Some(Background::Color(Color {
            a: if c.text.r > 0.5 { 0.25 } else { 0.45 },
            ..Color::BLACK
        })),
        ..container::Style::default()
    }
}

/// Tarjeta flotante (modales): sombra suave, SIN borde.
pub fn card(theme: &Theme) -> container::Style {
    let c = colors(theme);
    container::Style {
        background: Some(c.panel.into()),
        border: Border::default().rounded(6),
        shadow: Shadow {
            color: c.shadow,
            offset: iced::Vector::new(0.0, 6.0),
            blur_radius: 18.0,
        },
        ..container::Style::default()
    }
}

/// Panel del menú desplegable (flotante, sombra).
pub fn dropdown(theme: &Theme) -> container::Style {
    let c = colors(theme);
    container::Style {
        background: Some(c.panel.into()),
        border: Border::default().rounded(6),
        shadow: Shadow {
            color: c.shadow,
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 20.0,
        },
        ..container::Style::default()
    }
}

// —————————————————————————————— Textos ————————————————————————————————

pub fn muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors(theme).muted),
    }
}

pub fn error(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors(theme).danger),
    }
}

pub fn notice(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors(theme).success),
    }
}

pub fn added(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors(theme).add_text),
    }
}

pub fn deleted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors(theme).del_text),
    }
}

// —————————————————————————————— Botones ————————————————————————————————

/// Aclara/oscura un color (para hovers de botones sólidos).
fn shade(color: Color, factor: f32) -> Color {
    Color {
        r: (color.r * factor).min(1.0),
        g: (color.g * factor).min(1.0),
        b: (color.b * factor).min(1.0),
        a: color.a,
    }
}

/// Botón primario (commit, fetch, crear…): plano, radio 3, hover oscuro.
pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    let c = colors(theme);
    let background = match status {
        button::Status::Hovered => shade(c.accent, 0.82),
        button::Status::Pressed => shade(c.accent, 0.7),
        button::Status::Disabled => Color { a: 0.35, ..c.accent },
        _ => c.accent,
    };
    button::Style {
        background: Some(background.into()),
        text_color: Color::WHITE,
        border: Border::default().rounded(3),
        ..button::Style::default()
    }
}

/// Botón de acción destructiva (descartar, eliminar, abortar).
pub fn danger_btn(theme: &Theme, status: button::Status) -> button::Style {
    let c = colors(theme);
    let background = match status {
        button::Status::Hovered => shade(c.danger, 0.82),
        button::Status::Pressed => shade(c.danger, 0.7),
        button::Status::Disabled => Color { a: 0.35, ..c.danger },
        _ => c.danger,
    };
    button::Style {
        background: Some(background.into()),
        text_color: Color::WHITE,
        border: Border::default().rounded(3),
        ..button::Style::default()
    }
}

/// Botón de texto (Cancelar, Cerrar…): transparente con hover sutil.
pub fn text_btn(theme: &Theme, status: button::Status) -> button::Style {
    let c = colors(theme);
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => c.panel_soft,
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(background.into()),
        text_color: c.muted,
        border: Border::default().rounded(3),
        ..button::Style::default()
    }
}

/// Botón de la barra de menús (Archivo, Editar…): hover sutil.
pub fn menu_button(open: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme: &Theme, status: button::Status| {
        let c = colors(theme);
        let background = match status {
            button::Status::Active if open => c.panel_soft,
            button::Status::Hovered | button::Status::Pressed => c.panel_soft,
            _ => c.header,
        };
        button::Style {
            background: Some(background.into()),
            text_color: c.text,
            border: Border::default().rounded(3),
            ..button::Style::default()
        }
    }
}

/// Botón fantasma (filas, título arrastrable): hover sutil.
pub fn ghost(theme: &Theme, status: button::Status) -> button::Style {
    let c = colors(theme);
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => c.panel_soft,
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(background.into()),
        text_color: c.muted,
        border: Border::default().rounded(3),
        ..button::Style::default()
    }
}

/// Botones de control de ventana (min/max/cerrar): hover con color.
pub fn window_button(danger: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme: &Theme, status: button::Status| {
        let c = colors(theme);
        let background = match status {
            button::Status::Hovered | button::Status::Pressed => {
                if danger {
                    c.danger
                } else {
                    c.panel_soft
                }
            }
            _ => Color::TRANSPARENT,
        };
        let text_color = if danger && status == button::Status::Hovered {
            Color::WHITE
        } else if danger {
            c.danger
        } else {
            c.muted
        };
        button::Style {
            background: Some(background.into()),
            text_color,
            border: Border::default().rounded(0),
            ..button::Style::default()
        }
    }
}

/// Ítem activo de menú desplegable: hover sutil.
pub fn menu_item(theme: &Theme, status: button::Status) -> button::Style {
    let c = colors(theme);
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => c.panel_soft,
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(background.into()),
        text_color: c.text,
        border: Border::default().rounded(3),
        ..button::Style::default()
    }
}

/// Ítem deshabilitado de menú desplegable.
pub fn menu_item_disabled(theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: colors(theme).muted,
        border: Border::default().rounded(3),
        ..button::Style::default()
    }
}

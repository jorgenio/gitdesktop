//! Tema oscuro estilo GitHub Desktop + estilos reutilizables (PLAN.md §6).

use iced::widget::{container, text};
use iced::{Border, Color, Theme};
use iced::theme::Palette;

// Paleta "GitHub Dark".
pub const CANVAS: Color = hex(0x0D_11_17); // fondo general
pub const PANEL: Color = hex(0x16_1B_22); // paneles y tarjetas
pub const PANEL_SOFT: Color = hex(0x1C_21_28); // filas / hover
pub const BORDER: Color = hex(0x30_36_3D);
pub const TEXT: Color = hex(0xE6_ED_F3);
pub const TEXT_MUTED: Color = hex(0x8B_94_9E);
pub const ACCENT: Color = hex(0x2F_81_F7); // azul GitHub
pub const SUCCESS: Color = hex(0x23_86_36);
pub const DANGER: Color = hex(0xDA_36_33);

const fn hex(value: u32) -> Color {
    Color {
        r: ((value >> 16) & 0xff) as f32 / 255.0,
        g: ((value >> 8) & 0xff) as f32 / 255.0,
        b: (value & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

/// Tema base de Gito (oscuro).
pub fn gito() -> Theme {
    Theme::custom(
        "gito-dark",
        Palette {
            background: CANVAS,
            text: TEXT,
            primary: ACCENT,
            success: SUCCESS,
            danger: DANGER,
        },
    )
}

/// Panel con borde redondeado (cabecera, tarjetas).
pub fn panel(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(PANEL.into()),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    }
}

/// Columna lateral sin borde.
pub fn sidebar(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(PANEL.into()),
        ..container::Style::default()
    }
}

/// Texto atenuado (secundario).
pub fn muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(TEXT_MUTED),
    }
}

/// Texto de error.
pub fn error(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(DANGER),
    }
}

//! Tema oscuro estilo GitHub Desktop + estilos reutilizables (PLAN.md §6).

use iced::widget::{button, container, text};
use iced::{Background, Border, Color, Theme};
use iced::theme::Palette;

use gitdesktop_core::DiffLineKind;

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
pub const WARNING: Color = hex(0xD2_99_22); // amarillo GitHub

// Colores de diff (estilo GitHub Dark).
pub const DIFF_ADD_BG: Color = hex(0x0E_44_29);
pub const DIFF_DEL_BG: Color = hex(0x43_1E_22);
pub const DIFF_HUNK_BG: Color = hex(0x1B_2B_4A);

const fn hex(value: u32) -> Color {
    Color {
        r: ((value >> 16) & 0xff) as f32 / 255.0,
        g: ((value >> 8) & 0xff) as f32 / 255.0,
        b: (value & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

/// Tema base de Git-Desktop Jorgenio (oscuro).
pub fn gitdesktop() -> Theme {
    Theme::custom(
        "gitdesktop-dark",
        Palette {
            background: CANVAS,
            text: TEXT,
            primary: ACCENT,
            success: SUCCESS,
            danger: DANGER,
            warning: WARNING,
        },
    )
}

// ——————————————————————————— Contenedores ————————————————————————————

/// Panel con borde redondeado (cabecera, tarjetas).
pub fn panel(_theme: &Theme) -> container::Style {
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
pub fn sidebar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(PANEL.into()),
        ..container::Style::default()
    }
}

/// Fila de la lista de archivos; resalta la seleccionada.
pub fn file_row(selected: bool) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(if selected { PANEL_SOFT } else { PANEL }.into()),
        ..container::Style::default()
    }
}

/// Fondo de una línea de diff según su tipo.
pub fn diff_row(kind: DiffLineKind) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| {
        let background = match kind {
            DiffLineKind::Added => DIFF_ADD_BG,
            DiffLineKind::Deleted => DIFF_DEL_BG,
            DiffLineKind::HunkHeader => DIFF_HUNK_BG,
            DiffLineKind::Context => PANEL,
        };
        container::Style {
            background: Some(background.into()),
            ..container::Style::default()
        }
    }
}

/// Velo oscuro detrás de menús y modales.
pub fn scrim(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color {
            a: 0.55,
            ..Color::BLACK
        })),
        ..container::Style::default()
    }
}

// —————————————————————————————— Textos ————————————————————————————————

/// Texto atenuado (secundario).
pub fn muted(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(TEXT_MUTED),
    }
}

/// Texto de error.
pub fn error(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(DANGER),
    }
}

/// Texto de aviso exitoso (banners de resultado).
pub fn notice(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(hex(0x3F_B9_50)),
    }
}

/// Texto de líneas añadidas.
pub fn added(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(hex(0x7E_E7_87)),
    }
}

/// Texto de líneas eliminadas.
pub fn deleted(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(hex(0xFF_7B_72)),
    }
}

// —————————————————————————————— Botones ————————————————————————————————

/// Botón de la barra de menús (Archivo, Editar…).
pub fn menu_button(open: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, _status: button::Status| button::Style {
        background: Some(if open { PANEL_SOFT.into() } else { Color::TRANSPARENT.into() }),
        text_color: TEXT,
        border: Border::default().rounded(4),
        ..button::Style::default()
    }
}

/// Botón fantasma (área de arrastre del título): sin nada visible.
pub fn ghost(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: TEXT_MUTED,
        border: Border::default().rounded(0),
        ..button::Style::default()
    }
}

/// Botones de control de ventana (minimizar, maximizar, cerrar).
pub fn window_button(danger: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, _status: button::Status| button::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: if danger { DANGER } else { TEXT_MUTED },
        border: Border::default().rounded(4),
        ..button::Style::default()
    }
}

/// Ítem activo de un menú desplegable.
pub fn menu_item(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: TEXT,
        border: Border::default().rounded(4),
        ..button::Style::default()
    }
}

/// Ítem deshabilitado de un menú desplegable.
pub fn menu_item_disabled(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: TEXT_MUTED,
        border: Border::default().rounded(4),
        ..button::Style::default()
    }
}

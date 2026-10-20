//! Temas claro/oscuro estilo GitHub + estilos reutilizables (PLAN.md §6).
//!
//! Todas las funciones de estilo reciben `&Theme` y toman sus colores de
//! [`colors`], que despacha entre las paletas dark/light según el nombre
//! del tema. El tema activo se persiste en `UiState.theme`.

use iced::widget::{button, container, text};
use iced::{Background, Border, Color, Theme};
use iced::theme::{Base, Palette};

use gitdesktop_core::DiffLineKind;

/// Paleta completa de la app (más rica que `iced::theme::Palette`).
#[derive(Debug, Clone, Copy)]
pub struct GitoColors {
    pub canvas: Color,     // fondo general
    pub panel: Color,      // paneles y tarjetas
    pub panel_soft: Color, // filas / hover / selección
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color, // azul GitHub
    pub success: Color,
    pub danger: Color,
    pub warning: Color,
    pub add_bg: Color,    // fondo de líneas añadidas
    pub del_bg: Color,    // fondo de líneas eliminadas
    pub hunk_bg: Color,   // fondo de cabeceras de hunk
    pub add_text: Color,  // texto de líneas añadidas
    pub del_text: Color,  // texto de líneas eliminadas
}

/// GitHub Dark.
pub const DARK: GitoColors = GitoColors {
    canvas: hex(0x0D_11_17),
    panel: hex(0x16_1B_22),
    panel_soft: hex(0x1C_21_28),
    border: hex(0x30_36_3D),
    text: hex(0xE6_ED_F3),
    muted: hex(0x8B_94_9E),
    accent: hex(0x2F_81_F7),
    success: hex(0x23_86_36),
    danger: hex(0xDA_36_33),
    warning: hex(0xD2_99_22),
    add_bg: hex(0x0E_44_29),
    del_bg: hex(0x43_1E_22),
    hunk_bg: hex(0x1B_2B_4A),
    add_text: hex(0x7E_E7_87),
    del_text: hex(0xFF_7B_72),
};

/// GitHub Light.
pub const LIGHT: GitoColors = GitoColors {
    canvas: hex(0xFF_FF_FF),
    panel: hex(0xF6_F8_FA),
    panel_soft: hex(0xEA_EF_F2),
    border: hex(0xD0_D7_DE),
    text: hex(0x1F_23_28),
    muted: hex(0x65_6D_76),
    accent: hex(0x09_69_DA),
    success: hex(0x1A_7F_37),
    danger: hex(0xCF_22_2E),
    warning: hex(0x9A_67_00),
    add_bg: hex(0xDA_FB_E1),
    del_bg: hex(0xFF_EB_E9),
    hunk_bg: hex(0xDD_F4_FF),
    add_text: hex(0x1A_7F_37),
    del_text: hex(0xCF_22_2E),
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

/// Tema base de Git-Desktop Jorgenio a partir del valor persistido
/// (`UiState.theme`: "dark" | "light").
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

// ——————————————————————————— Contenedores ————————————————————————————

/// Panel con borde redondeado (cabecera, tarjetas).
pub fn panel(theme: &Theme) -> container::Style {
    let c = colors(theme);
    container::Style {
        background: Some(c.panel.into()),
        border: Border {
            color: c.border,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    }
}

/// Columna lateral sin borde.
pub fn sidebar(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(colors(theme).panel.into()),
        ..container::Style::default()
    }
}

/// Fila de la lista de archivos; resalta la seleccionada.
pub fn file_row(selected: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme: &Theme| {
        let c = colors(theme);
        container::Style {
            background: Some(if selected { c.panel_soft } else { c.panel }.into()),
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

/// Velo oscuro detrás de menús y modales (se adapta al tema).
pub fn scrim(theme: &Theme) -> container::Style {
    let c = colors(theme);
    container::Style {
        background: Some(Background::Color(Color {
            a: if c.text.r > 0.5 { 0.35 } else { 0.55 },
            ..Color::BLACK
        })),
        ..container::Style::default()
    }
}

// —————————————————————————————— Textos ————————————————————————————————

/// Texto atenuado (secundario).
pub fn muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors(theme).muted),
    }
}

/// Texto de error.
pub fn error(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors(theme).danger),
    }
}

/// Texto de aviso exitoso (banners de resultado).
pub fn notice(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors(theme).success),
    }
}

/// Texto de líneas añadidas.
pub fn added(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors(theme).add_text),
    }
}

/// Texto de líneas eliminadas.
pub fn deleted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors(theme).del_text),
    }
}

// —————————————————————————————— Botones ————————————————————————————————

/// Botón de la barra de menús (Archivo, Editar…).
pub fn menu_button(open: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme: &Theme, _status: button::Status| {
        let c = colors(theme);
        button::Style {
            background: Some(if open { c.panel_soft.into() } else { Color::TRANSPARENT.into() }),
            text_color: c.text,
            border: Border::default().rounded(4),
            ..button::Style::default()
        }
    }
}

/// Botón fantasma (área de arrastre del título): sin nada visible.
pub fn ghost(theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: colors(theme).muted,
        border: Border::default().rounded(0),
        ..button::Style::default()
    }
}

/// Botones de control de ventana (minimizar, maximizar, cerrar).
pub fn window_button(danger: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme: &Theme, _status: button::Status| {
        let c = colors(theme);
        button::Style {
            background: Some(Color::TRANSPARENT.into()),
            text_color: if danger { c.danger } else { c.muted },
            border: Border::default().rounded(4),
            ..button::Style::default()
        }
    }
}

/// Ítem activo de un menú desplegable.
pub fn menu_item(theme: &Theme, _status: button::Status) -> button::Style {
    let c = colors(theme);
    button::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: c.text,
        border: Border::default().rounded(4),
        ..button::Style::default()
    }
}

/// Ítem deshabilitado de un menú desplegable.
pub fn menu_item_disabled(theme: &Theme, _status: button::Status) -> button::Style {
    let c = colors(theme);
    button::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: c.muted,
        border: Border::default().rounded(4),
        ..button::Style::default()
    }
}

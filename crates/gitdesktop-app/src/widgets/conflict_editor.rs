//! Editor de conflictos embebido: canvas iced + `jorgenio-core` como motor.
//!
//! El modelo de texto (buffer ropey, transacciones, undo/redo) vive en
//! `jorgenio-core` (submódulo `vendor/text-editor`): las mejoras de ese
//! núcleo benefician a Jorgenio Text y a Git-Desktop por igual. El render
//! usa primitivas de iced (fill_text monoespaciado + rectángulos), con
//! resaltado de los marcadores de conflicto.

use std::path::PathBuf;

use iced::widget::canvas;
use iced::{Font, Point, Rectangle, Size, Theme};
use jorgenio_core::buffer::{Buffer, Edit};
use jorgenio_core::history::{History, Transaction};
use jorgenio_core::movement;

use crate::messages::Message;
use crate::theme;

const FONT_SIZE: f32 = 13.0;
const LINE_H: f32 = 20.0;
const CHAR_W: f32 = 7.8; // ancho aproximado del monoespaciado
const GUTTER: f32 = 56.0;
const PAD: f32 = 8.0;
const TAB_WIDTH: usize = 4;

/// Acciones editables desde teclado/botones.
#[derive(Debug, Clone)]
pub enum EditAction {
    Char(String),
    Newline,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Undo,
    Redo,
    PageUp,
    PageDown,
    /// Rueda del ratón: líneas a desplazar (+ abajo).
    Scroll(i32),
    /// Clic: coloca el caret en un offset absoluto.
    Caret(usize),
    Save,
    SaveAndResolve,
    Close,
}

/// Estado del editor embebido (poseído por `App`).
pub struct ConflictEditor {
    /// Ruta relativa al repo (para stage/refresh).
    pub path: String,
    /// Ruta absoluta en disco (para guardar).
    pub abs: PathBuf,
    pub buffer: Buffer,
    pub history: History,
    pub caret: usize,
    pub scroll: usize,
}

impl ConflictEditor {
    /// Abre el archivo del directorio de trabajo.
    pub fn open(rel: &str, abs: PathBuf) -> Result<Self, String> {
        let buffer = Buffer::load(&abs).map_err(|err| err.to_string())?;
        Ok(Self {
            path: rel.to_owned(),
            abs,
            buffer,
            history: History::new(),
            caret: 0,
            scroll: 0,
        })
    }

    /// Guarda el buffer en disco.
    pub fn save(&mut self) -> Result<(), String> {
        self.buffer.save(&self.abs).map_err(|err| err.to_string())
    }

    fn caret_line(&self) -> usize {
        self.buffer
            .char_to_line(self.caret.min(self.buffer.len_chars()))
    }

    fn line_len(&self, line: usize) -> usize {
        if line >= self.buffer.line_count() {
            0
        } else {
            let text = self.buffer.line_text(line);
            text.trim_end_matches(['\n', '\r']).chars().count()
        }
    }

    pub fn caret_col(&self) -> usize {
        let line = self.caret_line();
        let line_start = self.buffer.line_to_char(line);
        self.caret.saturating_sub(line_start)
    }

    fn set_caret(&mut self, caret: usize, visible_lines: usize) {
        self.caret = caret.min(self.buffer.len_chars());
        self.clamp_scroll(visible_lines);
    }

    fn clamp_scroll(&mut self, visible_lines: usize) {
        let line = self.caret_line();
        if line < self.scroll {
            self.scroll = line;
        } else if visible_lines > 0 && line >= self.scroll + visible_lines {
            self.scroll = line + 1 - visible_lines;
        }
    }

    /// Registra la edición del cuerpo como UNA transacción de undo.
    fn with_transaction(&mut self, before_caret: usize, body: impl FnOnce(&mut Buffer)) {
        let before = vec![(before_caret, before_caret)];
        self.buffer.begin();
        body(self.buffer_alias());
        let edits = self.buffer.take_edits();
        let after = vec![(self.caret, self.caret)];
        self.history.commit(Transaction::new(edits, before, after));
    }

    // Truco mínimo para mutar el buffer dentro del closure sin pelear con
    // los prestamos de self (el buffer es el único campo tocado ahí).
    fn buffer_alias(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    /// Aplica una acción de edición. Devuelve `true` si el buffer cambió.
    pub fn apply(&mut self, action: &EditAction, visible_lines: usize) -> bool {
        let before = self.caret;
        match action {
            EditAction::Char(text) => {
                self.with_transaction(before, |buffer| {
                    buffer.insert(before, text);
                });
                self.set_caret(before + text.chars().count(), visible_lines);
            }
            EditAction::Newline => {
                let ending = self.buffer.line_ending().as_str().to_owned();
                self.with_transaction(before, |buffer| {
                    buffer.insert(before, &ending);
                });
                self.set_caret(before + ending.chars().count(), visible_lines);
            }
            EditAction::Backspace => {
                if before > 0 {
                    let prev = movement::prev_grapheme(self.buffer_alias(), before);
                    let text = self.buffer.text_range(prev..before);
                    self.with_transaction(before, |buffer| {
                        Edit::Delete { at: prev, text: text.clone() }.apply(buffer);
                    });
                    self.set_caret(prev, visible_lines);
                }
            }
            EditAction::Delete => {
                let next = movement::next_grapheme(self.buffer_alias(), before);
                if next > before {
                    let text = self.buffer.text_range(before..next);
                    self.with_transaction(before, |buffer| {
                        Edit::Delete { at: before, text: text.clone() }.apply(buffer);
                    });
                }
            }
            EditAction::Left => {
                let prev = movement::prev_grapheme(self.buffer_alias(), before);
                self.set_caret(prev, visible_lines);
            }
            EditAction::Right => {
                let next = movement::next_grapheme(self.buffer_alias(), before);
                self.set_caret(next, visible_lines);
            }
            EditAction::Up => {
                let moved = movement::vertical_move(self.buffer_alias(), before, -1, TAB_WIDTH);
                self.set_caret(moved, visible_lines);
            }
            EditAction::Down => {
                let moved = movement::vertical_move(self.buffer_alias(), before, 1, TAB_WIDTH);
                self.set_caret(moved, visible_lines);
            }
            EditAction::Home => {
                let moved = movement::line_start(self.buffer_alias(), before);
                self.set_caret(moved, visible_lines);
            }
            EditAction::End => {
                let moved = movement::line_end(self.buffer_alias(), before);
                self.set_caret(moved, visible_lines);
            }
            EditAction::PageUp => {
                self.scroll = self.scroll.saturating_sub(visible_lines.max(1));
                let target = self
                    .buffer
                    .line_to_char(self.scroll.min(self.buffer.line_count().saturating_sub(1)));
                self.caret = target;
            }
            EditAction::PageDown => {
                self.scroll += visible_lines.max(1);
                let max_line = self.buffer.line_count().saturating_sub(1);
                if self.scroll > max_line {
                    self.scroll = max_line;
                }
                let target = self.buffer.line_to_char(self.scroll);
                self.caret = target;
            }
            EditAction::Undo => {
                if let Some(pairs) = self.history.undo(&mut self.buffer) {
                    if let Some(&(caret, _)) = pairs.first() {
                        self.set_caret(caret, visible_lines);
                    }
                }
            }
            EditAction::Redo => {
                if let Some(pairs) = self.history.redo(&mut self.buffer) {
                    if let Some(&(caret, _)) = pairs.first() {
                        self.set_caret(caret, visible_lines);
                    }
                }
            }
            EditAction::Scroll(lines) => {
                let max_line = self.buffer.line_count().saturating_sub(1);
                let target = self.scroll as i32 + lines;
                self.scroll = target.clamp(0, max_line as i32) as usize;
            }
            EditAction::Caret(caret) => {
                self.set_caret(*caret, visible_lines);
            }
            // Guardar/cerrar los gestiona `App::update` (necesitan el motor).
            EditAction::Save | EditAction::SaveAndResolve | EditAction::Close => {}
        }
        self.buffer.dirty()
    }
}

// ————————————————————————————— render ————————————————————————————————

/// Widget canvas que dibuja el buffer y captura clic/scroll.
pub struct EditorCanvas<'a> {
    pub state: &'a ConflictEditor,
}

impl<'a> EditorCanvas<'a> {
    pub fn new(state: &'a ConflictEditor) -> Self {
        Self { state }
    }

    fn marker_kind(line_text: &str) -> Option<Marker> {
        let trimmed = line_text.trim_start();
        if trimmed.starts_with("<<<<<<<") {
            Some(Marker::Start)
        } else if trimmed.starts_with("=======") {
            Some(Marker::Divider)
        } else if trimmed.starts_with(">>>>>>>") {
            Some(Marker::End)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy)]
enum Marker {
    Start,
    Divider,
    End,
}

impl canvas::Program<Message> for EditorCanvas<'_> {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            canvas::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                let lines = match delta {
                    iced::mouse::ScrollDelta::Lines { y, .. } => (-y * 3.0) as i32,
                    iced::mouse::ScrollDelta::Pixels { y, .. } => (-y / LINE_H) as i32,
                };
                Some(canvas::Action::publish(Message::EditorMsg(
                    EditAction::Scroll(lines),
                )))
            }
            canvas::Event::Mouse(iced::mouse::Event::ButtonPressed(
                iced::mouse::Button::Left,
            )) => {
                let pos = cursor.position_in(bounds)?;
                let line = (self.state.scroll as f32 + (pos.y - PAD) / LINE_H)
                    .max(0.0) as usize;
                let line = line.min(self.state.buffer.line_count().saturating_sub(1));
                let col = ((pos.x - GUTTER) / CHAR_W).max(0.0) as usize;
                let line_start = self.state.buffer.line_to_char(line);
                let caret = line_start + col.min(self.state.line_len(line));
                Some(canvas::Action::publish(Message::EditorMsg(
                    EditAction::Caret(caret),
                )))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let c = theme::colors(theme);
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Fondo del área de edición.
        frame.fill_rectangle(
            Point::ORIGIN,
            bounds.size(),
            c.panel,
        );

        let visible = ((bounds.height - PAD) / LINE_H).ceil() as usize;
        let total_lines = self.state.buffer.line_count();
        let first = self.state.scroll;
        let last = (first + visible).min(total_lines);

        // Resaltado de marcadores y regiones del conflicto.
        let mut region: Option<Marker> = None;
        for line_idx in first..last {
            let text = self.state.buffer.line_text(line_idx);
            let y = PAD + (line_idx - first) as f32 * LINE_H;

            let bg = match Self::marker_kind(&text) {
                Some(Marker::Start) => {
                    region = Some(Marker::Start);
                    c.del_bg
                }
                Some(Marker::Divider) => {
                    region = Some(Marker::Divider);
                    c.hunk_bg
                }
                Some(Marker::End) => {
                    region = None;
                    c.del_bg
                }
                None => match region {
                    Some(Marker::Start) => c.add_bg,
                    Some(Marker::Divider) => c.del_bg,
                    _ => c.panel,
                },
            };

            if bg != c.panel {
                frame.fill_rectangle(
                    Point::new(0.0, y),
                    Size::new(bounds.width, LINE_H),
                    bg,
                );
            }
        }

        // Gutter + texto visible.
        for line_idx in first..last {
            let text = self.state.buffer.line_text(line_idx);
            let clean = text.trim_end_matches(['\n', '\r']);
            let y = PAD + (line_idx - first) as f32 * LINE_H;

            frame.fill_text(canvas::Text {
                content: (line_idx + 1).to_string(),
                position: Point::new(4.0, y),
                color: c.muted,
                size: (FONT_SIZE - 1.0).into(),
                font: Font::MONOSPACE,
                ..Default::default()
            });

            frame.fill_text(canvas::Text {
                content: clean.to_owned(),
                position: Point::new(GUTTER, y),
                color: c.text,
                size: FONT_SIZE.into(),
                font: Font::MONOSPACE,
                ..Default::default()
            });
        }

        // Caret (línea vertical en la columna actual).
        let caret_line = self.state.caret_line();
        if caret_line >= first && caret_line < last {
            let col = self.state.caret_col();
            let x = GUTTER + col as f32 * CHAR_W;
            let y = PAD + (caret_line - first) as f32 * LINE_H;
            frame.fill_rectangle(
                Point::new(x, y),
                Size::new(2.0, LINE_H),
                c.accent,
            );
        }

        // Borde.
        frame.fill_rectangle(
            Point::new(0.0, 0.0),
            Size::new(bounds.width, 1.0),
            c.border,
        );

        vec![frame.into_geometry()]
    }
}

// `Geometry` lo devuelve el frame; alias para la firma del Program.
use canvas::Geometry;


/// Líneas visibles del editor (alto fijo del modal: 400 px).
pub const EDITOR_VISIBLE_LINES: usize = 20;

//! Grafo de commits (canvas): un carril por línea de historia, con
//! aristas hacia los padres. Los carriles los calcula [`compute_lanes`].

use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Size, Theme};
use gitdesktop_core::CommitSummary;

use crate::messages::Message;
use crate::theme;

/// Alto fijo de cada fila del historial (mismo valor que la lista).
pub const ROW_H: f32 = 44.0;
const GRAPH_W: f32 = 44.0;
const LANE_W: f32 = 12.0;

/// Una fila del grafo: carril del commit y aristas hacia sus padres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRow {
    pub lane: usize,
    /// `(carril_origen, carril_destino)` — segmentos hacia la fila siguiente.
    pub edges: Vec<(usize, usize)>,
}

/// Asigna carriles por commit (algoritmo de carriles simple):
/// el primer padre hereda el carril del commit; los demás abren uno nuevo.
pub fn compute_lanes(commits: &[CommitSummary]) -> Vec<GraphRow> {
    let mut lanes: Vec<String> = Vec::new();
    let mut rows = Vec::new();

    for commit in commits {
        let lane = match lanes.iter().position(|oid| oid == &commit.id) {
            Some(pos) => {
                lanes.remove(pos);
                pos
            }
            None => {
                lanes.push(commit.id.clone());
                lanes.len() - 1
            }
        };

        let mut edges = Vec::new();
        for (index, parent) in commit.parents.iter().enumerate() {
            let target = if index == 0 {
                // Primer padre: hereda el carril del commit (o ya existía).
                match lanes.iter().position(|oid| oid == parent) {
                    Some(existing) => existing,
                    None => {
                        let pos = lane.min(lanes.len());
                        lanes.insert(pos, parent.clone());
                        pos
                    }
                }
            } else {
                match lanes.iter().position(|oid| oid == parent) {
                    Some(existing) => existing,
                    None => {
                        lanes.push(parent.clone());
                        lanes.len() - 1
                    }
                }
            };
            edges.push((lane, target));
        }

        rows.push(GraphRow { lane, edges });
    }

    rows
}

/// Colores por carril (ciclo de 5).
fn lane_color(index: usize) -> Color {
    const PALETTE: [Color; 5] = [
        Color::from_rgb8(0x2F, 0x81, 0xF7), // azul
        Color::from_rgb8(0x3F, 0xB9, 0x50), // verde
        Color::from_rgb8(0xF8, 0x51, 0x49), // rojo
        Color::from_rgb8(0xA3, 0x71, 0xF7), // violeta
        Color::from_rgb8(0xD2, 0x99, 0x22), // ámbar
    ];
    PALETTE[index % PALETTE.len()]
}

/// Canvas que dibuja todo el grafo (una sola geometría, alto = filas × ROW_H).
pub struct GraphCanvas<'a> {
    pub rows: &'a [GraphRow],
}

impl canvas::Program<Message> for GraphCanvas<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let c = theme::colors(theme);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), c.panel);

        for (index, row) in self.rows.iter().enumerate() {
            let y = index as f32 * ROW_H + ROW_H / 2.0;
            let x = |lane: usize| GRAPH_W / 2.0 + lane as f32 * LANE_W;

            // Aristas hacia la fila siguiente (padres).
            for (from, to) in &row.edges {
                let color = lane_color(*to);
                if from == to {
                    // Línea recta vertical.
                    frame.fill_rectangle(
                        Point::new(x(*from) - 1.5, y),
                        Size::new(3.0, ROW_H),
                        color,
                    );
                } else {
                    // Diagonal hacia el carril destino.
                    frame.fill_rectangle(
                        Point::new(x(*to).min(x(*from)), y + ROW_H / 2.0 - 1.5),
                        Size::new((x(*to) as i32 - x(*from) as i32).abs() as f32 + 3.0, 3.0),
                        color,
                    );
                    frame.fill_rectangle(
                        Point::new(x(*from) - 1.5, y),
                        Size::new(3.0, ROW_H / 2.0),
                        lane_color(*from),
                    );
                    frame.fill_rectangle(
                        Point::new(x(*to) - 1.5, y + ROW_H / 2.0),
                        Size::new(3.0, ROW_H / 2.0),
                        color,
                    );
                }
            }

            // Nodo del commit.
            let center = Point::new(x(row.lane), y);
            frame.fill(&canvas::Path::circle(center, 5.0), lane_color(row.lane));
            frame.fill(&canvas::Path::circle(center, 2.0), c.panel);
        }

        vec![frame.into_geometry()]
    }
}

//! `catena-demo` (plan §19 M3): the community fixture and the 120-node generated family in the
//! `Graph` widget, every binding of the default key table live, with the mouse selecting,
//! hovering, dragging and zooming. Tab switches graphs; `q` or Ctrl+C quits. With `NO_COLOR`
//! set and not empty it draws plain ASCII without colors.
//!
//! `cargo run -p catena-ratatui --features demo --bin catena-demo`
//!
//! The demo is a host, so it owns what the library never does: the terminal, the clock (it
//! passes the time between frames to `tick`) and the environment.

use std::collections::BTreeMap;
use std::io;
use std::time::{Duration, Instant}; // lint-allow: time — a host's loop owns the clock, and this bin is a host (plan §10.4)

use catena::geometry::cell::CellBox;
use catena::interact::Dir;
use catena::raster::PaletteColor;
use catena::{Blitter, GraphView, Outcome, Redraw};
use catena_ratatui::input::from_crossterm;
use catena_ratatui::{Graph, Legend, Minimap};
use catena_testkit::fixtures::{FixtureGraph, community, generated};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier, Style};
use ratatui_core::terminal::{Frame, Terminal};
use ratatui_crossterm::CrosstermBackend;

/// The status line's reminder of the keys.
const KEYS: &str = "hjkl pan  +/- zoom  n/N cycle  Shift+arrows nav  Enter pin  Esc clear  \
                    b bundling  Tab graph  q quit";

/// How long the loop waits for input while something animates, and while nothing does.
const FRAME: Duration = Duration::from_millis(16);
const IDLE: Duration = Duration::from_millis(250);

fn main() -> io::Result<()> {
    let plain = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let mut app = App::new(plain);
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        default_hook(info);
    }));
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let ran = Terminal::new(CrosstermBackend::new(stdout))
        .and_then(|mut terminal| run(&mut terminal, &mut app));
    let restored = restore();
    ran.and(restored)
}

/// Puts the terminal back as the demo found it.
fn restore() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> io::Result<()> {
    let mut last = Instant::now(); // lint-allow: time — the host's clock, read once a frame
    loop {
        let dt = last.elapsed();
        last += dt;
        let redraw = app.tick(dt);
        if redraw == Redraw::Needed || app.dirty {
            terminal.draw(|frame| app.draw(frame))?;
            app.dirty = false;
        }
        let wait = if redraw == Redraw::Needed {
            FRAME
        } else {
            IDLE
        };
        if event::poll(wait)? && app.event(&event::read()?) == Flow::Quit {
            return Ok(());
        }
    }
}

/// Whether the demo goes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Continue,
    Quit,
}

/// One graph the demo shows, with its nodes' labels by key for the status line.
struct Shown {
    name: &'static str,
    view: GraphView<String>,
    labels: BTreeMap<String, String>,
}

impl Shown {
    fn new(name: &'static str, fixture: &FixtureGraph, plain: bool) -> Self {
        let builder = GraphView::builder();
        let builder = if plain {
            builder.blitter(Blitter::Ascii).ascii_glyphs()
        } else {
            builder.blitter(Blitter::Braille)
        };
        Shown {
            name,
            view: fixture.view(builder).expect("fixtures are consistent"),
            labels: fixture
                .nodes
                .iter()
                .map(|n| (n.key.clone(), n.label.clone()))
                .collect(),
        }
    }

    fn label<'a>(&'a self, key: &'a str) -> &'a str {
        self.labels.get(key).map_or(key, String::as_str)
    }
}

/// The demo's state, apart from the terminal.
struct App {
    graphs: Vec<Shown>,
    current: usize,
    /// `NO_COLOR`: ASCII glyphs and no colors.
    plain: bool,
    /// What last happened, for the status line.
    status: String,
    /// Whether the status line changed since the last frame.
    dirty: bool,
}

impl App {
    fn new(plain: bool) -> Self {
        App {
            graphs: vec![
                Shown::new("community", &community(), plain),
                Shown::new("generated 7 x 120", &generated(7, 120), plain),
            ],
            current: 0,
            plain,
            status: "n selects the first node; the mouse works too".to_string(),
            dirty: true,
        }
    }

    fn shown(&mut self) -> &mut Shown {
        &mut self.graphs[self.current]
    }

    fn tick(&mut self, dt: Duration) -> Redraw {
        self.shown().view.tick(dt)
    }

    /// Acts on one terminal event.
    fn event(&mut self, event: &Event) -> Flow {
        match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Char('q') => return Flow::Quit,
                    KeyCode::Char('c') if ctrl => return Flow::Quit,
                    KeyCode::Tab => {
                        self.current = (self.current + 1) % self.graphs.len();
                        self.status = format!("showing {}", self.graphs[self.current].name);
                        self.dirty = true;
                        return Flow::Continue;
                    }
                    _ => {}
                }
            }
            // The frame's own size lays the graph out, so a resize only needs drawing.
            Event::Resize(..) => {
                self.dirty = true;
                return Flow::Continue;
            }
            _ => {}
        }
        if let Some(input) = from_crossterm(event) {
            let bundling = self.shown().view.bundling();
            for outcome in self.shown().view.handle(input) {
                if let Some(line) = self.describe(outcome) {
                    self.status = line;
                    self.dirty = true;
                }
            }
            if self.shown().view.bundling() != bundling {
                self.dirty = true;
            }
        }
        Flow::Continue
    }

    /// The status line's words for `outcome`, if it has any.
    fn describe(&self, outcome: Outcome) -> Option<String> {
        let shown = &self.graphs[self.current];
        let name = |ix| shown.view.key(ix).map_or("?", |key| shown.label(key));
        Some(match outcome {
            Outcome::Selected(Some(ix)) => format!("selected {}", name(ix)),
            Outcome::Selected(None) => "nothing selected".to_string(),
            Outcome::Hovered(Some(ix)) => format!("over {}", name(ix)),
            Outcome::EdgeSelected(Some(id)) => {
                let edge = shown.view.edge(id)?;
                format!(
                    "selected {} -> {}",
                    shown.label(edge.from()),
                    shown.label(edge.to())
                )
            }
            Outcome::Pinned(ix, true) => format!("pinned {}", name(ix)),
            Outcome::Pinned(ix, false) => format!("unpinned {}", name(ix)),
            Outcome::DragCompleted(ix, _) => format!("dropped {}", name(ix)),
            Outcome::Boundary(dir) => {
                let way = match dir {
                    Dir::Up => "up",
                    Dir::Down => "down",
                    Dir::Left => "left",
                    Dir::Right => "right",
                };
                format!("nothing further {way}")
            }
            _ => return None,
        })
    }

    /// Draws the graph over every row but the last, and the status line on it.
    fn draw(&mut self, frame: &mut Frame<'_>) {
        let area = frame.area();
        let rows = area.height.saturating_sub(1);
        let graph = Rect::new(area.x, area.y, area.width, rows);
        let extras = overlays(graph);
        let occluded: Vec<CellBox> = extras
            .iter()
            .flatten()
            .map(|r| {
                let at = |v: u16, from: u16| i32::from(v - from);
                CellBox::new(
                    at(r.x, graph.x),
                    at(r.y, graph.y),
                    u32::from(r.width),
                    u32::from(r.height),
                )
            })
            .collect();
        self.shown().view.set_occlusions(&occluded);
        frame.render_stateful_widget(Graph::new(), graph, &mut self.shown().view);
        let plain = self.plain;
        let shown = &self.graphs[self.current];
        if let Some([legend, minimap]) = extras {
            let swatch = if plain { '#' } else { '■' };
            frame.render_widget(Legend::new(&LEGEND).swatch(swatch), legend);
            frame.render_widget(Minimap::new(&shown.view), minimap);
        }
        let bundling = if shown.view.bundling() {
            ""
        } else {
            " | bundling off"
        };
        let line = format!(" {} | {}{bundling} | {KEYS}", shown.name, self.status);
        let buf = frame.buffer_mut();
        if plain {
            strip_colors(buf, graph);
        }
        if area.height > rows {
            let style = Style::default().add_modifier(Modifier::REVERSED);
            buf.set_stringn(area.x, area.y + rows, line, usize::from(area.width), style);
        }
    }
}

/// The legend of the drawing's placeholder styles (until plan §12's themes name their own).
const LEGEND: [(&str, PaletteColor); 4] = [
    ("node", PaletteColor::Ansi(7)),
    ("edge", PaletteColor::Ansi(8)),
    ("hover-lifted edge", PaletteColor::Ansi(15)),
    ("minimap: in view", PaletteColor::Ansi(14)),
];

/// Where the legend (top right) and the minimap (bottom right) float over a graph drawn in
/// `graph`, when it has room for them beside a graph worth seeing.
fn overlays(graph: Rect) -> Option<[Rect; 2]> {
    let (width, legend_rows, minimap_rows) = (24, 4, 9);
    if graph.width < 3 * width || graph.height < 2 * (legend_rows + minimap_rows) {
        return None;
    }
    let x = graph.right() - width - 1;
    Some([
        Rect::new(x, graph.y + 1, width, legend_rows),
        Rect::new(x, graph.bottom() - minimap_rows - 1, width, minimap_rows),
    ])
}

/// `NO_COLOR`: every cell of `area` in the terminal's own colors, its attributes kept.
fn strip_colors(buf: &mut Buffer, area: Rect) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            cell.fg = Color::Reset;
            cell.bg = Color::Reset;
        }
    }
}

#[cfg(test)]
mod tests;

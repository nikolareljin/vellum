use std::io;
use std::path::Path;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};
use ratatui::Terminal;

use crate::parser;
use crate::renderer::render_elements;

struct App {
    lines: Vec<Line<'static>>,
    scroll: usize,
    viewport_height: usize,
}

impl App {
    fn new(lines: Vec<Line<'static>>) -> Self {
        App { lines, scroll: 0, viewport_height: 0 }
    }

    fn scroll_down(&mut self, n: usize) {
        let max = self.lines.len().saturating_sub(self.viewport_height);
        self.scroll = (self.scroll + n).min(max);
    }

    fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }

    fn page_down(&mut self) {
        self.scroll_down(self.viewport_height.saturating_sub(2));
    }

    fn page_up(&mut self) {
        self.scroll_up(self.viewport_height.saturating_sub(2));
    }

    fn goto_top(&mut self) {
        self.scroll = 0;
    }

    fn goto_bottom(&mut self) {
        self.scroll = self.lines.len().saturating_sub(self.viewport_height);
    }
}

pub fn run(file: &Path) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(file)?;
    let elements = parser::parse(&source);
    let lines = render_elements(&elements);
    let fname = file.file_name().unwrap_or_default().to_string_lossy().to_string();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(lines);

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(area);

            app.viewport_height = chunks[0].height as usize;

            let visible: Vec<Line<'static>> = app
                .lines
                .iter()
                .skip(app.scroll)
                .take(app.viewport_height)
                .cloned()
                .collect();

            let content = Paragraph::new(Text::from(visible))
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: false });
            f.render_widget(content, chunks[0]);

            // Status bar
            let total = app.lines.len();
            let pct = if total == 0 { 100 } else { ((app.scroll + app.viewport_height).min(total) * 100) / total };
            let status = Line::from(vec![
                Span::styled(
                    format!(" {} ", fname),
                    Style::default().fg(Color::Black).bg(Color::Cyan),
                ),
                Span::raw(format!(
                    "  line {}/{} ({}%)  │  j/k scroll  g/G top/bot  e code-view  q quit",
                    app.scroll + 1, total, pct,
                )),
            ]);
            f.render_widget(Paragraph::new(status), chunks[1]);

            // Scrollbar
            let mut scrollbar_state = ScrollbarState::new(app.lines.len()).position(app.scroll);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                chunks[0],
                &mut scrollbar_state,
            );
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(KeyEvent { code, modifiers, .. }) => match (code, modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                    (KeyCode::Char('j'), _) | (KeyCode::Down, _) => app.scroll_down(1),
                    (KeyCode::Char('k'), _) | (KeyCode::Up, _) => app.scroll_up(1),
                    (KeyCode::PageDown, _) | (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                        app.page_down()
                    }
                    (KeyCode::PageUp, _) | (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                        app.page_up()
                    }
                    (KeyCode::Char('g'), _) | (KeyCode::Home, _) => app.goto_top(),
                    (KeyCode::Char('G'), _) | (KeyCode::End, _) => app.goto_bottom(),
                    (KeyCode::Char('e'), _) => {
                        // Temporarily leave TUI, open code view, re-enter
                        disable_raw_mode()?;
                        execute!(
                            terminal.backend_mut(),
                            LeaveAlternateScreen,
                            crossterm::event::DisableMouseCapture
                        )?;
                        let _ = open_code_view(file);
                        enable_raw_mode()?;
                        execute!(
                            terminal.backend_mut(),
                            EnterAlternateScreen,
                            crossterm::event::EnableMouseCapture
                        )?;
                        terminal.clear()?;
                    }
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => app.scroll_down(3),
                    MouseEventKind::ScrollUp => app.scroll_up(3),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    Ok(())
}

pub fn open_code_view(file: &Path) -> anyhow::Result<()> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if which_exists("bat") {
                "bat".into()
            } else if which_exists("less") {
                "less".into()
            } else {
                "cat".into()
            }
        });

    let status = std::process::Command::new(&editor).arg(file).status()?;
    if !status.success() {
        anyhow::bail!("editor '{}' exited with {}", editor, status);
    }
    Ok(())
}

fn which_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

use std::io;
use std::path::Path;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};
use ratatui::Terminal;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

use crate::image::ImageCache;
use crate::links::{build_anchor_map, collect_links, open_url};
use crate::parser::{self, Element};
use crate::renderer::render_elements;
use crate::search::{search_lines, SearchResult};
use crate::video::extract_thumbnail;

/// A rendered line is either a text line or an inline image slot.
#[derive(Clone)]
pub enum DisplayLine {
    Text(Line<'static>),
    /// Image: the src path and desired height in terminal rows.
    Image { src: String, height: u16 },
}

struct App {
    lines: Vec<DisplayLine>,
    scroll: usize,
    viewport_height: usize,
    /// ratatui-image state per src (loaded on first render)
    image_states: std::collections::HashMap<String, StatefulProtocol>,
    picker: Picker,
    image_cache: ImageCache,
    /// (href, rendered-line-offset)
    doc_links: Vec<(String, usize)>,
    link_cursor: Option<usize>,
    anchor_map: std::collections::HashMap<String, usize>,
    /// Keep temp files alive for the session (drop = delete)
    _thumb_files: Vec<tempfile::NamedTempFile>,
    /// Search state
    search_mode: bool,
    search_query: String,
    search_results: Vec<SearchResult>,
    search_cursor: usize,
}

impl App {
    fn new(
        lines: Vec<DisplayLine>,
        picker: Picker,
        doc_links: Vec<(String, usize)>,
        anchor_map: std::collections::HashMap<String, usize>,
        thumb_files: Vec<tempfile::NamedTempFile>,
    ) -> Self {
        App {
            lines,
            scroll: 0,
            viewport_height: 0,
            image_states: Default::default(),
            picker,
            image_cache: Default::default(),
            doc_links,
            link_cursor: None,
            anchor_map,
            _thumb_files: thumb_files,
            search_mode: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_cursor: 0,
        }
    }

    fn scroll_down(&mut self, n: usize) {
        let max = self.lines.len().saturating_sub(self.viewport_height);
        self.scroll = (self.scroll + n).min(max);
    }

    fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }

    fn page_down(&mut self) { self.scroll_down(self.viewport_height.saturating_sub(2)); }
    fn page_up(&mut self) { self.scroll_up(self.viewport_height.saturating_sub(2)); }
    fn goto_top(&mut self) { self.scroll = 0; }
    fn goto_bottom(&mut self) {
        self.scroll = self.lines.len().saturating_sub(self.viewport_height);
    }
}

/// Build DisplayLine list. Returns (lines, thumb_files).
/// `base_dir` is the directory that contains the Markdown file; image paths
/// that are relative are resolved against it so that `vellum /any/dir/doc.md`
/// always finds sibling images regardless of the shell's working directory.
/// thumb_files must be kept alive by the caller (drop = delete temp files).
fn build_display_lines(
    elements: &[Element],
    base_dir: &Path,
) -> (Vec<DisplayLine>, Vec<tempfile::NamedTempFile>) {
    let mut out = Vec::new();
    let mut thumb_files = Vec::new();

    for el in elements {
        match el {
            Element::Image { src, .. } => {
                let resolved = resolve_path(src, base_dir);
                // Only create an Image slot when the file is local and exists.
                // Remote URLs and missing files fall back to the styled text
                // placeholder so they don't leave an invisible blank gap.
                if is_local_file_readable(&resolved) {
                    out.push(DisplayLine::Image { src: resolved, height: 10 });
                } else {
                    let text_lines = render_elements(std::slice::from_ref(el));
                    for l in text_lines { out.push(DisplayLine::Text(l)); }
                }
                out.push(DisplayLine::Text(Line::from("")));
            }
            Element::Video { src } => {
                // Parser already classified this as video; extract thumbnail.
                let resolved = resolve_path(src, base_dir);
                match extract_thumbnail(&resolved) {
                    Ok(tmp) => {
                        let path = tmp.path().to_string_lossy().to_string();
                        out.push(DisplayLine::Image { src: path, height: 10 });
                        out.push(DisplayLine::Text(Line::from("")));
                        thumb_files.push(tmp);
                    }
                    Err(_) => {
                        // ffmpeg missing or file unreadable — text placeholder
                        let text_lines = render_elements(std::slice::from_ref(el));
                        for l in text_lines { out.push(DisplayLine::Text(l)); }
                    }
                }
            }
            _ => {
                let text_lines = render_elements(std::slice::from_ref(el));
                for l in text_lines { out.push(DisplayLine::Text(l)); }
            }
        }
    }
    (out, thumb_files)
}

/// Returns `true` when `src` is a local path that exists and is a regular file.
/// Remote URLs are always `false`; they cannot be rendered inline.
fn is_local_file_readable(src: &str) -> bool {
    if src.starts_with("http://") || src.starts_with("https://") {
        return false;
    }
    std::path::Path::new(src).is_file()
}

/// Returns `true` when `href` looks like a relative link to a Markdown file
/// (not an anchor, not an HTTP URL, ends with `.md` or `.markdown`).
fn is_local_md_link(href: &str) -> bool {
    if href.starts_with('#')
        || href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("mailto:")
    {
        return false;
    }
    let lower = href.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

/// Resolve an image/video `src` relative to the document's `base_dir`.
/// Absolute paths and `http(s)://` URLs are returned unchanged.
fn resolve_path(src: &str, base_dir: &Path) -> String {
    if src.starts_with("http://") || src.starts_with("https://") {
        return src.to_owned();
    }
    let p = Path::new(src);
    if p.is_absolute() {
        src.to_owned()
    } else {
        base_dir.join(p).to_string_lossy().into_owned()
    }
}

pub fn run(file: &Path) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(file)?;
    let elements = parser::parse(&source);
    let base_dir = file.parent().unwrap_or(Path::new("."));
    let (display_lines, thumb_files) = build_display_lines(&elements, base_dir);
    let doc_links = collect_links(&elements);
    let anchor_map = build_anchor_map(&elements);
    let fname = file.file_name().unwrap_or_default().to_string_lossy().to_string();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Auto-detect best image protocol (Kitty → Sixel → iTerm2 → halfblock)
    let picker = Picker::from_query_stdio()
        .unwrap_or_else(|_| Picker::from_fontsize((8, 12)));

    let mut app = App::new(display_lines, picker, doc_links, anchor_map, thumb_files);

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(area);

            app.viewport_height = chunks[0].height as usize;
            let content_area = chunks[0];

            // Render visible lines
            let visible: Vec<&DisplayLine> = app.lines.iter().skip(app.scroll).take(app.viewport_height).collect();

            let mut y_offset = 0u16;
            let mut text_batch: Vec<Line<'static>> = Vec::new();

            for dl in visible {
                match dl {
                    DisplayLine::Text(line) => {
                        text_batch.push(line.clone());
                        y_offset += 1;
                    }
                    DisplayLine::Image { src, height } => {
                        // Flush accumulated text lines before rendering image
                        if !text_batch.is_empty() {
                            let batch_h = text_batch.len() as u16;
                            let text_y = y_offset.saturating_sub(batch_h);
                            let rect = Rect {
                                x: content_area.x,
                                y: content_area.y + text_y,
                                width: content_area.width,
                                height: batch_h.min(content_area.height.saturating_sub(text_y)),
                            };
                            if rect.height > 0 {
                                f.render_widget(
                                    Paragraph::new(Text::from(text_batch.clone()))
                                        .block(Block::default().borders(Borders::NONE))
                                        .wrap(Wrap { trim: false }),
                                    rect,
                                );
                            }
                            text_batch.clear();
                        }
                        // Load image state on first encounter
                        if !app.image_states.contains_key(src) {
                            if let Ok(dyn_img) = app.image_cache.get_or_load(src) {
                                let state = app.picker.new_resize_protocol(dyn_img.clone());
                                app.image_states.insert(src.clone(), state);
                            }
                        }

                        if app.image_states.contains_key(src) {
                            // Image loaded — render it at full height
                            let img_rect = Rect {
                                x: content_area.x,
                                y: content_area.y + y_offset,
                                width: content_area.width,
                                height: (*height).min(content_area.height.saturating_sub(y_offset)),
                            };
                            if img_rect.height > 0 {
                                if let Some(state) = app.image_states.get_mut(src) {
                                    let widget = StatefulImage::new().resize(Resize::Fit(None));
                                    f.render_stateful_widget(widget, img_rect, state);
                                }
                            }
                            y_offset += height;
                        } else {
                            // Load failed — consume only 1 row so there is no
                            // large blank hole where the image would have been
                            y_offset += 1;
                        }
                    }
                }
            }
            // flush any trailing text
            if !text_batch.is_empty() {
                let start_y = y_offset.saturating_sub(text_batch.len() as u16);
                let rect = Rect {
                    x: content_area.x,
                    y: content_area.y + start_y,
                    width: content_area.width,
                    height: (text_batch.len() as u16).min(content_area.height.saturating_sub(start_y)),
                };
                if rect.height > 0 {
                    f.render_widget(
                        Paragraph::new(Text::from(text_batch.clone()))
                            .block(Block::default().borders(Borders::NONE))
                            .wrap(Wrap { trim: false }),
                        rect,
                    );
                }
            }

            // Status bar — shows search prompt when in search mode
            let total = app.lines.len();
            let pct = if total == 0 { 100 } else { ((app.scroll + app.viewport_height).min(total) * 100) / total };
            let status = if app.search_mode {
                Line::from(vec![
                    Span::styled(" / ", Style::default().fg(Color::Black).bg(Color::Yellow)),
                    Span::raw(format!("{}_", app.search_query)),
                ])
            } else if !app.search_results.is_empty() {
                Line::from(vec![
                    Span::styled(format!(" {} ", fname), Style::default().fg(Color::Black).bg(Color::Cyan)),
                    Span::raw(format!(
                        "  [{}/{}] \"{}\"  │  n/N next/prev  Esc clear",
                        app.search_cursor + 1, app.search_results.len(), app.search_query,
                    )),
                ])
            } else if let Some(i) = app.link_cursor {
                // Show focused link's href — VSCode-like bottom bar hint
                let href = &app.doc_links[i].0;
                Line::from(vec![
                    Span::styled(format!(" {} ", fname), Style::default().fg(Color::Black).bg(Color::Cyan)),
                    Span::styled(
                        format!("  → {}  ", href),
                        Style::default().fg(Color::Rgb(80, 190, 255)),
                    ),
                    Span::styled(
                        "│ Enter follow  Tab next  Shift+Tab prev  Esc deselect",
                        Style::default().fg(Color::Rgb(120, 120, 120)),
                    ),
                ])
            } else {
                Line::from(vec![
                    Span::styled(format!(" {} ", fname), Style::default().fg(Color::Black).bg(Color::Cyan)),
                    Span::raw(format!(
                        "  {}/{} ({}%)  │  j/k scroll  g/G top/bottom  Tab links  / search  e code  q quit",
                        app.scroll + 1, total, pct,
                    )),
                ])
            };
            f.render_widget(Paragraph::new(status), chunks[1]);

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
                    // Search mode input
                    (KeyCode::Char(ch), _) if app.search_mode => {
                        app.search_query.push(ch);
                    }
                    (KeyCode::Backspace, _) if app.search_mode => {
                        app.search_query.pop();
                    }
                    (KeyCode::Enter, _) if app.search_mode => {
                        app.search_mode = false;
                        // Run search against text lines only
                        let text_lines: Vec<ratatui::text::Line> = app.lines.iter().filter_map(|dl| {
                            if let DisplayLine::Text(l) = dl { Some(l.clone()) } else { None }
                        }).collect();
                        app.search_results = search_lines(&text_lines, &app.search_query);
                        app.search_cursor = 0;
                        if let Some(r) = app.search_results.first() {
                            app.scroll = r.line_index.min(app.lines.len().saturating_sub(app.viewport_height));
                        }
                    }
                    (KeyCode::Esc, _) if app.search_mode => {
                        app.search_mode = false;
                        app.search_query.clear();
                        app.search_results.clear();
                    }
                    (KeyCode::Esc, _) => {
                        app.search_results.clear();
                        app.search_cursor = 0;
                        app.link_cursor = None; // deselect focused link
                    }
                    // Search activation and navigation (normal mode)
                    (KeyCode::Char('/'), _) => {
                        app.search_mode = true;
                        app.search_query.clear();
                        app.search_results.clear();
                    }
                    (KeyCode::Char('n'), _) => {
                        if !app.search_results.is_empty() {
                            app.search_cursor = (app.search_cursor + 1) % app.search_results.len();
                            let idx = app.search_results[app.search_cursor].line_index;
                            app.scroll = idx.min(app.lines.len().saturating_sub(app.viewport_height));
                        }
                    }
                    (KeyCode::Char('N'), _) => {
                        if !app.search_results.is_empty() {
                            let len = app.search_results.len();
                            app.search_cursor = if app.search_cursor == 0 { len - 1 } else { app.search_cursor - 1 };
                            let idx = app.search_results[app.search_cursor].line_index;
                            app.scroll = idx.min(app.lines.len().saturating_sub(app.viewport_height));
                        }
                    }
                    (KeyCode::Char('j'), _) | (KeyCode::Down, _) => app.scroll_down(1),
                    (KeyCode::Char('k'), _) | (KeyCode::Up, _) => app.scroll_up(1),
                    (KeyCode::PageDown, _) | (KeyCode::Char('f'), KeyModifiers::CONTROL) => app.page_down(),
                    (KeyCode::PageUp, _) | (KeyCode::Char('b'), KeyModifiers::CONTROL) => app.page_up(),
                    (KeyCode::Char('g'), _) | (KeyCode::Home, _) => app.goto_top(),
                    (KeyCode::Char('G'), _) | (KeyCode::End, _) => app.goto_bottom(),
                    (KeyCode::Tab, _) => {
                        if app.doc_links.is_empty() { /* nothing */ }
                        else if let Some(i) = app.link_cursor {
                            app.link_cursor = Some((i + 1) % app.doc_links.len());
                        } else {
                            app.link_cursor = Some(0);
                        }
                        if let Some(i) = app.link_cursor {
                            let offset = app.doc_links[i].1;
                            app.scroll = offset.min(app.lines.len().saturating_sub(app.viewport_height));
                        }
                    }
                    (KeyCode::BackTab, _) => {
                        if !app.doc_links.is_empty() {
                            let len = app.doc_links.len();
                            app.link_cursor = Some(match app.link_cursor {
                                Some(0) | None => len - 1,
                                Some(i) => i - 1,
                            });
                            if let Some(i) = app.link_cursor {
                                let offset = app.doc_links[i].1;
                                app.scroll = offset.min(app.lines.len().saturating_sub(app.viewport_height));
                            }
                        }
                    }
                    (KeyCode::Enter, _) => {
                        if let Some(i) = app.link_cursor {
                            let href = app.doc_links[i].0.clone();
                            if href.starts_with('#') {
                                // Anchor jump within this document
                                let slug = &href[1..];
                                if let Some(&offset) = app.anchor_map.get(slug) {
                                    app.scroll = offset.min(app.lines.len().saturating_sub(app.viewport_height));
                                }
                            } else if is_local_md_link(&href) {
                                // Relative .md file — resolve and open in vellum
                                let target = resolve_path(&href, file.parent().unwrap_or(Path::new(".")));
                                let target_path = std::path::PathBuf::from(&target);
                                if target_path.is_file() {
                                    disable_raw_mode()?;
                                    execute!(terminal.backend_mut(), LeaveAlternateScreen, crossterm::event::DisableMouseCapture)?;
                                    let _ = run(&target_path);
                                    enable_raw_mode()?;
                                    execute!(terminal.backend_mut(), EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
                                    terminal.clear()?;
                                }
                            } else {
                                let _ = open_url(&href);
                            }
                        }
                    }
                    (KeyCode::Char('e'), _) => {
                        disable_raw_mode()?;
                        execute!(terminal.backend_mut(), LeaveAlternateScreen, crossterm::event::DisableMouseCapture)?;
                        let _ = open_code_view(file);
                        enable_raw_mode()?;
                        execute!(terminal.backend_mut(), EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
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
    execute!(terminal.backend_mut(), LeaveAlternateScreen, crossterm::event::DisableMouseCapture)?;
    Ok(())
}

pub fn open_code_view(file: &Path) -> anyhow::Result<()> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if which_exists("bat") { "bat".into() }
            else if which_exists("less") { "less".into() }
            else { "cat".into() }
        });
    let status = std::process::Command::new(&editor).arg(file).status()?;
    if !status.success() {
        anyhow::bail!("editor '{}' exited with {}", editor, status);
    }
    Ok(())
}

fn which_exists(cmd: &str) -> bool {
    std::process::Command::new("which").arg(cmd).output()
        .map(|o| o.status.success()).unwrap_or(false)
}

use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::Terminal;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

use crate::image::ImageCache;
use crate::links::{build_anchor_map, collect_links, open_url};
use crate::parser::{self, Element};
use crate::renderer::render_elements;
use crate::search::{search_lines, SearchResult};
use crate::theme::Theme;
use crate::video::extract_thumbnail;

// ── Navigation history ────────────────────────────────────────────────────────

/// What `run()` wants the caller to do next.
pub enum NavAction {
    Quit,
    GoTo(PathBuf),
    Back,
    Forward,
}

/// Browser-like navigation history capped at 20 entries each direction.
pub struct NavHistory {
    pub back: std::collections::VecDeque<PathBuf>,
    pub forward: std::collections::VecDeque<PathBuf>,
}

impl NavHistory {
    const MAX: usize = 20;

    pub fn new() -> Self {
        Self {
            back: Default::default(),
            forward: Default::default(),
        }
    }

    /// Navigate to a new file: push `current` onto back stack, clear forward.
    pub fn push_back(&mut self, current: PathBuf) {
        if self.back.len() == Self::MAX {
            self.back.pop_front();
        }
        self.back.push_back(current);
        self.forward.clear();
    }

    /// Go back: pop from back, push `current` onto forward.
    /// Returns `None` when already at the oldest entry.
    pub fn go_back(&mut self, current: PathBuf) -> Option<PathBuf> {
        let prev = self.back.pop_back()?;
        if self.forward.len() == Self::MAX {
            self.forward.pop_front();
        }
        self.forward.push_back(current);
        Some(prev)
    }

    /// Go forward: pop from forward, push `current` onto back.
    /// Returns `None` when there is no forward history.
    pub fn go_forward(&mut self, current: PathBuf) -> Option<PathBuf> {
        let next = self.forward.pop_back()?;
        if self.back.len() == Self::MAX {
            self.back.pop_front();
        }
        self.back.push_back(current);
        Some(next)
    }
}

/// A rendered line is either a text line or an inline image slot.
#[derive(Clone)]
pub enum DisplayLine {
    Text(Line<'static>),
    /// Image: the src path and desired height in terminal rows.
    Image {
        src: String,
        height: u16,
    },
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
    /// Maps each SearchResult.line_index (into text-only lines) back to the
    /// corresponding index in `self.lines` (which includes image entries).
    search_line_map: Vec<usize>,
    /// Sources whose load already failed — skipped on subsequent redraws.
    failed_images: std::collections::HashSet<String>,
    /// Remote URLs currently being fetched on background threads.
    pending_fetches: std::collections::HashSet<String>,
    /// Completed background fetches waiting to be integrated.
    /// Each message is `(src, Ok(image) | Err(reason))`.
    fetch_rx: mpsc::Receiver<(String, Result<image::DynamicImage, String>)>,
    /// Cloned into each spawned fetch thread.
    fetch_tx: mpsc::Sender<(String, Result<image::DynamicImage, String>)>,
}

impl App {
    fn new(
        lines: Vec<DisplayLine>,
        picker: Picker,
        doc_links: Vec<(String, usize)>,
        anchor_map: std::collections::HashMap<String, usize>,
        thumb_files: Vec<tempfile::NamedTempFile>,
    ) -> Self {
        let (fetch_tx, fetch_rx) = mpsc::channel();
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
            search_line_map: Vec::new(),
            failed_images: Default::default(),
            pending_fetches: Default::default(),
            fetch_tx,
            fetch_rx,
        }
    }

    /// Compute the maximum scroll offset in terms of *entries* (not rows),
    /// accounting for image heights.  Works backwards from the last entry,
    /// accumulating row heights until the viewport would be full; the entry
    /// index at that point is the correct start-of-viewport position.
    ///
    /// Why this is necessary: a `DisplayLine::Image { height: N }` occupies N
    /// terminal rows but only 1 slot in `self.lines`.  Using
    /// `lines.len() - viewport_height` as max_scroll (entry-count arithmetic)
    /// places an in-viewport image above the fold and displaces text that
    /// follows it, so only 1 item of a long list at the bottom may be visible.
    fn max_scroll(&self) -> usize {
        if self.viewport_height == 0 {
            return 0;
        }
        let mut rows = 0usize;
        let mut idx = self.lines.len();
        while idx > 0 {
            let entry_rows = match &self.lines[idx - 1] {
                DisplayLine::Image { height, .. } => *height as usize,
                DisplayLine::Text(_) => 1,
            };
            if rows + entry_rows > self.viewport_height {
                break;
            }
            rows += entry_rows;
            idx -= 1;
        }
        idx
    }

    fn scroll_down(&mut self, n: usize) {
        let max = self.max_scroll();
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
        self.scroll = self.max_scroll();
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
    theme: &Theme,
) -> (Vec<DisplayLine>, Vec<tempfile::NamedTempFile>) {
    let mut out = Vec::new();
    let mut thumb_files = Vec::new();

    for el in elements {
        match el {
            Element::Image { src, .. } => {
                let resolved = resolve_path(src, base_dir);
                let is_remote = resolved.starts_with("http://") || resolved.starts_with("https://");
                // Create an Image slot for local readable files and remote URLs.
                // Missing local files fall back to the styled text placeholder
                // so they don't leave an invisible blank gap.
                if is_remote || is_local_file_readable(&resolved) {
                    out.push(DisplayLine::Image {
                        src: resolved,
                        height: 10,
                    });
                    out.push(DisplayLine::Text(Line::from("")));
                } else {
                    let text_lines = render_elements(std::slice::from_ref(el), theme);
                    for l in text_lines {
                        out.push(DisplayLine::Text(l));
                    }
                }
            }
            Element::Video { src } => {
                // Parser already classified this as video; extract thumbnail.
                let resolved = resolve_path(src, base_dir);
                match extract_thumbnail(&resolved) {
                    Ok(tmp) => {
                        let path = tmp.path().to_string_lossy().to_string();
                        out.push(DisplayLine::Image {
                            src: path,
                            height: 10,
                        });
                        out.push(DisplayLine::Text(Line::from("")));
                        thumb_files.push(tmp);
                    }
                    Err(_) => {
                        // ffmpeg missing or file unreadable — text placeholder
                        let text_lines = render_elements(std::slice::from_ref(el), theme);
                        for l in text_lines {
                            out.push(DisplayLine::Text(l));
                        }
                    }
                }
            }
            _ => {
                let text_lines = render_elements(std::slice::from_ref(el), theme);
                for l in text_lines {
                    out.push(DisplayLine::Text(l));
                }
            }
        }
    }
    (out, thumb_files)
}

/// Resolve and follow `href` from the context of `file`:
/// - `#anchor` → returns `None` (handled by caller as in-page scroll)
/// - `./other.md` / `other.md` → returns `Some(resolved_path)`
/// - `https://...` → opens in system browser, returns `None`
fn follow_link(href: &str, file: &Path) -> Option<std::path::PathBuf> {
    if href.starts_with('#') {
        // Anchor jumps are handled separately by the caller
        return None;
    }
    if is_local_md_link(href) {
        let base = file.parent().unwrap_or(Path::new("."));
        let resolved = resolve_path(href, base);
        let path = std::path::PathBuf::from(&resolved);
        if path.is_file() {
            return Some(path);
        }
    } else {
        let _ = open_url(href);
    }
    None
}

/// Convert a terminal viewport row into the absolute entry index in `lines`,
/// accounting for image entries that occupy multiple terminal rows.
fn viewport_row_to_entry(lines: &[DisplayLine], scroll: usize, row: usize) -> usize {
    let mut rows = 0usize;
    for (i, dl) in lines.iter().enumerate().skip(scroll) {
        let entry_height = match dl {
            DisplayLine::Image { height, .. } => *height as usize,
            DisplayLine::Text(_) => 1,
        };
        if row < rows + entry_height {
            return i;
        }
        rows += entry_height;
    }
    lines.len().saturating_sub(1)
}

/// Return the href of the first link whose rendered line is within ±1 of
/// `target_line`.  Used to map a mouse-click row to the nearest link.
fn link_at_line(doc_links: &[(String, usize)], target_line: usize) -> Option<&str> {
    doc_links
        .iter()
        .min_by_key(|(_, line)| (*line).abs_diff(target_line))
        .filter(|(_, line)| {
            let diff = (*line).abs_diff(target_line);
            diff <= 1
        })
        .map(|(href, _)| href.as_str())
}

/// Returns `true` when `src` is a local path that exists and is a regular file.
/// Remote URLs return `false`; callers that support remote fetching check separately.
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

pub fn run(file: &Path, history: &NavHistory, theme: &Theme) -> anyhow::Result<NavAction> {
    let source = std::fs::read_to_string(file)?;
    let elements = parser::parse(&source);
    let base_dir = file.parent().unwrap_or(Path::new("."));
    let (display_lines, thumb_files) = build_display_lines(&elements, base_dir, theme);
    let doc_links = collect_links(&elements);
    let anchor_map = build_anchor_map(&elements);
    let fname = file
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Auto-detect best image protocol (Kitty → Sixel → iTerm2 → halfblock).
    // VELLUM_PROTOCOL=halfblocks skips the terminal query (useful for recording / CI).
    let picker = if std::env::var("VELLUM_PROTOCOL").as_deref() == Ok("halfblocks") {
        Picker::halfblocks()
    } else {
        Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks())
    };

    let mut app = App::new(display_lines, picker, doc_links, anchor_map, thumb_files);

    // Prime viewport_height before the first iteration so the initial image
    // prefetch covers the full visible area rather than the default of 0.
    if let Ok(size) = terminal.size() {
        app.viewport_height = size.height.saturating_sub(1) as usize;
    }

    // Labeled loop: break with NavAction to signal what to do next.
    // Terminal is cleaned up AFTER the loop.
    let action: NavAction = 'main: loop {
        // Integrate any completed background fetches.
        while let Ok((src, result)) = app.fetch_rx.try_recv() {
            app.pending_fetches.remove(&src);
            match result {
                Ok(img) => {
                    let state = app.picker.new_resize_protocol(img);
                    app.image_states.insert(src, state);
                }
                Err(_) => {
                    app.failed_images.insert(src);
                }
            }
        }

        // Schedule images that are about to scroll into view.
        // Remote URLs are fetched on background threads so the UI stays
        // responsive; local files are read synchronously (fast disk I/O).
        {
            let lookahead = app.viewport_height.max(1) + 5;
            let to_schedule: Vec<(String, bool)> = app
                .lines
                .iter()
                .skip(app.scroll)
                .take(lookahead)
                .filter_map(|dl| match dl {
                    DisplayLine::Image { src, .. }
                        if !app.image_states.contains_key(src)
                            && !app.failed_images.contains(src)
                            && !app.pending_fetches.contains(src) =>
                    {
                        let remote = src.starts_with("http://") || src.starts_with("https://");
                        Some((src.clone(), remote))
                    }
                    _ => None,
                })
                .collect();

            // Limit concurrent remote fetches to avoid spawning a thread storm
            // when a document contains many remote images.
            const MAX_CONCURRENT_FETCHES: usize = 4;

            for (src, remote) in to_schedule {
                if remote {
                    if app.pending_fetches.len() >= MAX_CONCURRENT_FETCHES {
                        // Cap reached; this URL will be retried next tick.
                        continue;
                    }
                    app.pending_fetches.insert(src.clone());
                    let tx = app.fetch_tx.clone();
                    std::thread::spawn(move || {
                        let res = crate::image::load_image_url(&src).map_err(|e| e.to_string());
                        let _ = tx.send((src, res));
                    });
                } else {
                    match app.image_cache.get_or_load(&src) {
                        Ok(img) => {
                            let state = app.picker.new_resize_protocol(img.clone());
                            app.image_states.insert(src, state);
                        }
                        Err(_) => {
                            app.failed_images.insert(src);
                        }
                    }
                }
            }
        }

        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(area);

            app.viewport_height = chunks[0].height as usize;
            let content_area = chunks[0];

            // Render visible lines — one widget per line so each text line
            // occupies exactly 1 terminal row.  The old batched-Paragraph
            // approach used Wrap which silently consumed extra rows for long
            // lines (e.g. code-block content), pushing everything below the
            // viewport and making the bottom of the document invisible.
            //
            // NOTE: do NOT pre-filter with take(viewport_height): a
            // DisplayLine::Image { height: N } consumes N rows but only 1
            // entry slot, so take() would pull too few entries and stop
            // rendering before the viewport is full.  The y_offset guard
            // below is the sole stop condition.
            let mut y_offset = 0u16;

            for dl in app.lines.iter().skip(app.scroll) {
                if y_offset >= content_area.height {
                    break;
                }
                match dl {
                    DisplayLine::Text(line) => {
                        f.render_widget(
                            Paragraph::new(line.clone())
                                .block(Block::default().borders(Borders::NONE)),
                            Rect {
                                x: content_area.x,
                                y: content_area.y + y_offset,
                                width: content_area.width,
                                height: 1,
                            },
                        );
                        y_offset += 1;
                    }
                    DisplayLine::Image { src, height } => {
                        // Images are loaded in the prefetch phase before this
                        // draw call; this closure is pure render only.
                        if app.image_states.contains_key(src) {
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
                        } else if app.pending_fetches.contains(src) {
                            f.render_widget(
                                Paragraph::new(Line::from(Span::styled(
                                    " [loading\u{2026}]",
                                    Style::default().fg(Color::DarkGray),
                                ))),
                                Rect {
                                    x: content_area.x,
                                    y: content_area.y + y_offset,
                                    width: content_area.width,
                                    height: 1,
                                },
                            );
                            y_offset += height;
                        } else if app.failed_images.contains(src) {
                            // Show a visible placeholder so failures aren't
                            // silent blank gaps, while still consuming the
                            // reserved height to keep scroll math consistent.
                            // Strip query/fragment to avoid leaking tokens in the UI.
                            // Truncate at a char boundary to avoid panics on non-ASCII URLs.
                            let safe_src = src
                                .split('?')
                                .next()
                                .and_then(|s| s.split('#').next())
                                .unwrap_or(src);
                            let label: &str = &safe_src[..safe_src
                                .char_indices()
                                .nth(60)
                                .map(|(i, _)| i)
                                .unwrap_or(safe_src.len())];
                            let dim = Style::default().fg(Color::DarkGray);
                            f.render_widget(
                                Paragraph::new(
                                    Line::from(vec![
                                        Span::raw(" [image unavailable: "),
                                        Span::raw(label),
                                        Span::raw("]"),
                                    ])
                                    .style(dim),
                                ),
                                Rect {
                                    x: content_area.x,
                                    y: content_area.y + y_offset,
                                    width: content_area.width,
                                    height: 1,
                                },
                            );
                            y_offset += height;
                        } else {
                            // Not yet loaded (first frame before prefetch runs)
                            y_offset += height;
                        }
                    }
                }
            }

            // Status bar — shows search prompt when in search mode
            let total = app.lines.len();
            let pct = ((app.scroll + app.viewport_height).min(total) * 100).checked_div(total).unwrap_or(100);
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
                        Style::default().fg(theme.inline.link.to_color()),
                    ),
                    Span::styled(
                        "│ Enter follow  Tab next  Shift+Tab prev  Esc deselect",
                        Style::default().fg(theme.inline.strikethrough.to_color()),
                    ),
                ])
            } else {
                Line::from(vec![
                    Span::styled(format!(" {} ", fname), Style::default().fg(Color::Black).bg(Color::Cyan)),
                    {
                        let back_n  = history.back.len();
                        let fwd_n   = history.forward.len();
                        let nav_hint = match (back_n, fwd_n) {
                            (0, 0) => String::new(),
                            (b, 0) => format!("  ← [{}]", b),
                            (0, f) => format!("  → [{}]", f),
                            (b, f) => format!("  ← [{}]  → [{}]", b, f),
                        };
                        Span::raw(format!(
                            "  {}/{} ({}%)  │  j/k scroll  g/G top/bottom  Tab links  / search  e code  a about  q quit{}",
                            app.scroll + 1, total, pct, nav_hint,
                        ))
                    },
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
                Event::Key(KeyEvent {
                    code, modifiers, ..
                }) => match (code, modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        break 'main NavAction::Quit;
                    }

                    // ── History navigation ───────────────────────────────────
                    (KeyCode::Left, KeyModifiers::ALT) => break 'main NavAction::Back,
                    (KeyCode::Right, KeyModifiers::ALT) => break 'main NavAction::Forward,

                    // ── Search mode input ────────────────────────────────────
                    (KeyCode::Char(ch), _) if app.search_mode => {
                        app.search_query.push(ch);
                    }
                    (KeyCode::Backspace, _) if app.search_mode => {
                        app.search_query.pop();
                    }
                    (KeyCode::Enter, _) if app.search_mode => {
                        app.search_mode = false;
                        // Build text_lines and a parallel map back to app.lines indices
                        // so search result offsets (into text-only lines) can be translated
                        // to the correct scroll position in the full DisplayLine list.
                        let mut text_lines = Vec::new();
                        let mut search_line_map = Vec::new();
                        for (i, dl) in app.lines.iter().enumerate() {
                            if let DisplayLine::Text(l) = dl {
                                text_lines.push(l.clone());
                                search_line_map.push(i);
                            }
                        }
                        app.search_results = search_lines(&text_lines, &app.search_query);
                        app.search_line_map = search_line_map;
                        app.search_cursor = 0;
                        if let Some(r) = app.search_results.first() {
                            let mapped = app
                                .search_line_map
                                .get(r.line_index)
                                .copied()
                                .unwrap_or(r.line_index);
                            app.scroll = mapped.min(app.max_scroll());
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
                        app.link_cursor = None;
                    }

                    // ── Search navigation ────────────────────────────────────
                    (KeyCode::Char('/'), _) => {
                        app.search_mode = true;
                        app.search_query.clear();
                        app.search_results.clear();
                    }
                    (KeyCode::Char('n'), _) if !app.search_results.is_empty() => {
                        app.search_cursor = (app.search_cursor + 1) % app.search_results.len();
                        let text_idx = app.search_results[app.search_cursor].line_index;
                        let idx = app
                            .search_line_map
                            .get(text_idx)
                            .copied()
                            .unwrap_or(text_idx);
                        app.scroll = idx.min(app.max_scroll());
                    }
                    (KeyCode::Char('N'), _) if !app.search_results.is_empty() => {
                        let len = app.search_results.len();
                        app.search_cursor = if app.search_cursor == 0 {
                            len - 1
                        } else {
                            app.search_cursor - 1
                        };
                        let text_idx = app.search_results[app.search_cursor].line_index;
                        let idx = app
                            .search_line_map
                            .get(text_idx)
                            .copied()
                            .unwrap_or(text_idx);
                        app.scroll = idx.min(app.max_scroll());
                    }

                    // ── Scroll ───────────────────────────────────────────────
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

                    // ── Link cycling ─────────────────────────────────────────
                    (KeyCode::Tab, _) if !app.doc_links.is_empty() => {
                        app.link_cursor = Some(match app.link_cursor {
                            Some(i) => (i + 1) % app.doc_links.len(),
                            None => 0,
                        });
                        let offset = app.doc_links[app.link_cursor.unwrap()].1;
                        app.scroll = offset.min(app.max_scroll());
                    }
                    (KeyCode::BackTab, _) if !app.doc_links.is_empty() => {
                        let len = app.doc_links.len();
                        app.link_cursor = Some(match app.link_cursor {
                            Some(0) | None => len - 1,
                            Some(i) => i - 1,
                        });
                        let offset = app.doc_links[app.link_cursor.unwrap()].1;
                        app.scroll = offset.min(app.max_scroll());
                    }

                    // ── Follow link (keyboard) ────────────────────────────────
                    (KeyCode::Enter, _) => {
                        if let Some(i) = app.link_cursor {
                            let href = app.doc_links[i].0.clone();
                            if let Some(slug) = href.strip_prefix('#') {
                                if let Some(&offset) = app.anchor_map.get(slug) {
                                    app.scroll = offset.min(app.max_scroll());
                                }
                            } else if let Some(path) = follow_link(&href, file) {
                                break 'main NavAction::GoTo(path);
                            }
                        }
                    }

                    // ── Code view ────────────────────────────────────────────
                    (KeyCode::Char('e'), _) => {
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

                    // ── About page ───────────────────────────────────────────
                    (KeyCode::Char('a'), _) => {
                        disable_raw_mode()?;
                        execute!(
                            terminal.backend_mut(),
                            LeaveAlternateScreen,
                            crossterm::event::DisableMouseCapture
                        )?;
                        crate::about::print_page();
                        println!("  Press Enter to return…");
                        let mut input = String::new();
                        let _ = std::io::stdin().read_line(&mut input);
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

                    // ── Left click: follow link under cursor ──────────────────
                    MouseEventKind::Down(MouseButton::Left) => {
                        let clicked_line =
                            viewport_row_to_entry(&app.lines, app.scroll, mouse.row as usize);
                        if let Some(href) = link_at_line(&app.doc_links, clicked_line) {
                            let href = href.to_owned();
                            if let Some(slug) = href.strip_prefix('#') {
                                if let Some(&offset) = app.anchor_map.get(slug) {
                                    app.scroll = offset.min(app.max_scroll());
                                }
                            } else if let Some(path) = follow_link(&href, file) {
                                break 'main NavAction::GoTo(path);
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }; // end 'main loop

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;

    Ok(action)
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

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

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

use crate::image::{is_remote_url, safe_error_url, ImageCache};
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

/// Rebuild a flat `(char, Style)` sequence into a styled `Line`.
/// Consecutive chars with the same `Style` are merged into one `Span`.
fn chars_to_line(chars: Vec<(char, ratatui::style::Style)>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut cur_style = ratatui::style::Style::default();

    for (c, style) in chars {
        if style != cur_style && !buf.is_empty() {
            spans.push(Span::styled(std::mem::take(&mut buf), cur_style));
        }
        cur_style = style;
        buf.push(c);
    }
    if !buf.is_empty() {
        spans.push(Span::styled(buf, cur_style));
    }
    Line::from(spans)
}

/// Word-wrap a styled `Line` into multiple `Line`s, each ≤ `max_width` chars wide.
///
/// Breaks at whitespace boundaries; falls back to a hard-break at `max_width` for
/// tokens (long code identifiers, URLs) that exceed the full terminal width.
/// Returns the original single-element vec when the line already fits.
///
/// Width is measured in Unicode scalar values (char count). CJK / combining
/// characters may wrap a column or two early — acceptable for this renderer.
pub fn wrap_line(line: Line<'static>, max_width: usize) -> Vec<Line<'static>> {
    if max_width == 0 {
        return vec![line];
    }

    let chars: Vec<(char, ratatui::style::Style)> = line
        .spans
        .iter()
        .flat_map(|s| {
            let style = s.style;
            s.content.chars().map(move |c| (c, style))
        })
        .collect();

    if chars.len() <= max_width {
        return vec![line];
    }

    let mut result: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<(char, ratatui::style::Style)> = Vec::new();
    let mut col: usize = 0;
    // Style of the first whitespace char in the inter-word gap, preserved so
    // that the re-inserted space keeps the original styling (e.g. styled space
    // after a list bullet keeps the bullet's colour/background).
    let mut pending_space: Option<ratatui::style::Style> = None;

    // Preserve leading whitespace on the first output line (indentation, bullet
    // prefixes, heading padding).  Continuation lines after a wrap break start
    // at column 0 and do not inherit the original indent.
    let mut i = 0;
    while i < chars.len() && chars[i].0.is_whitespace() {
        current.push(chars[i]);
        col += 1;
        i += 1;
    }

    while i < chars.len() {
        if chars[i].0.is_whitespace() {
            if col > 0 {
                // Capture the style of the first whitespace char in this run.
                pending_space.get_or_insert(chars[i].1);
            }
            while i < chars.len() && chars[i].0.is_whitespace() {
                i += 1;
            }
        } else {
            let word_start = i;
            while i < chars.len() && !chars[i].0.is_whitespace() {
                i += 1;
            }
            let word = &chars[word_start..i];
            let word_len = word.len();
            let space_cost = if pending_space.is_some() { 1 } else { 0 };

            if col > 0 && col + space_cost + word_len > max_width {
                result.push(chars_to_line(std::mem::take(&mut current)));
                col = 0;
                pending_space = None;
            }

            if word_len >= max_width {
                // Token wider than terminal — hard-break it.
                if col > 0 {
                    result.push(chars_to_line(std::mem::take(&mut current)));
                }
                let mut rem = word;
                while rem.len() >= max_width {
                    current.extend_from_slice(&rem[..max_width]);
                    result.push(chars_to_line(std::mem::take(&mut current)));
                    rem = &rem[max_width..];
                }
                current.extend_from_slice(rem);
                col = rem.len();
            } else {
                if let Some(sp_style) = pending_space.take() {
                    current.push((' ', sp_style));
                    col += 1;
                }
                current.extend_from_slice(word);
                col += word_len;
            }
            pending_space = None;
        }
    }

    if !current.is_empty() {
        result.push(chars_to_line(current));
    }

    if result.is_empty() {
        vec![line]
    } else {
        result
    }
}

/// Compute the terminal-row slot height for a local image.
///
/// `Resize::Fit` in ratatui-image never scales an image *up* — it only scales
/// down to fit within the slot.  The slot must therefore not exceed how many
/// rows the image actually occupies at display time.
///
/// Two constraints apply simultaneously:
///
/// 1. **Natural height**: `⌈img_h_px / cell_h_px⌉` — how tall the image is
///    at native pixel size.  `cell_h_px` comes from TIOCGWINSZ (no raw mode).
///
/// 2. **Width-limited height**: `⌈img_h_px × term_cols / (img_w_px × 2)⌉` —
///    for images wider than the terminal, `Resize::Fit` scales them down to
///    fit horizontally, reducing their rendered height.  The ÷2 accounts for
///    the typical 2:1 cell pixel ratio (cells ~2× taller than wide).
///
/// The slot height is the minimum of the two so it is never over-allocated.
/// Falls back to `fallback` for remote URLs and unreadable files.
/// Result is clamped to `[1, max_h]`.
fn image_slot_height(src: &str, term_cols: u16, cell_h: u16, max_h: u16, fallback: u16) -> u16 {
    if is_remote_url(src) {
        return fallback;
    }
    if let Ok(size) = imagesize::size(src) {
        let w = size.width as u32;
        let h = size.height as u32;
        let ch = cell_h.max(1) as u32;

        // Constraint 1: natural pixel height → rows.
        let natural = h.div_ceil(ch) as u16;

        // Constraint 2: when the image is wider than the terminal it is scaled
        // down proportionally; compute the resulting height (assumes 2:1 ratio).
        let width_limited = if w > 0 {
            (h * term_cols as u32).div_ceil(w * 2) as u16
        } else {
            natural
        };

        return natural.min(width_limited).clamp(1, max_h);
    }
    fallback
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
    term_cols: usize,
    cell_h: u16,
    img_max_h: u16,
    img_fallback_h: u16,
) -> (Vec<DisplayLine>, Vec<tempfile::NamedTempFile>) {
    let mut out = Vec::new();
    let mut thumb_files = Vec::new();

    for el in elements {
        match el {
            Element::Image { src, .. } => {
                let resolved = resolve_path(src, base_dir);
                let is_remote = is_remote_url(&resolved);
                // true only when fetching is enabled (VELLUM_NO_REMOTE_IMAGES unset);
                // blocked remote URLs get no Image slot, avoiding a blank gap.
                let remote_allowed =
                    is_remote && std::env::var_os("VELLUM_NO_REMOTE_IMAGES").is_none();
                // Create an Image slot for local readable files and allowed remote URLs.
                // Missing/blocked sources fall back to the styled text placeholder.
                if remote_allowed || is_local_file_readable(&resolved) {
                    let h = image_slot_height(
                        &resolved,
                        term_cols as u16,
                        cell_h,
                        img_max_h,
                        img_fallback_h,
                    );
                    out.push(DisplayLine::Image {
                        src: resolved,
                        height: h,
                    });
                } else {
                    let text_lines = render_elements(std::slice::from_ref(el), theme);
                    for l in text_lines {
                        for wrapped in wrap_line(l, term_cols) {
                            out.push(DisplayLine::Text(wrapped));
                        }
                    }
                }
            }
            Element::Video { src } => {
                // Parser already classified this as video; extract thumbnail.
                let resolved = resolve_path(src, base_dir);
                match extract_thumbnail(&resolved) {
                    Ok(tmp) => {
                        let path = tmp.path().to_string_lossy().to_string();
                        let h = image_slot_height(
                            &path,
                            term_cols as u16,
                            cell_h,
                            img_max_h,
                            img_fallback_h,
                        );
                        out.push(DisplayLine::Image {
                            src: path,
                            height: h,
                        });
                        thumb_files.push(tmp);
                    }
                    Err(_) => {
                        // ffmpeg missing or file unreadable — text placeholder
                        let text_lines = render_elements(std::slice::from_ref(el), theme);
                        for l in text_lines {
                            for wrapped in wrap_line(l, term_cols) {
                                out.push(DisplayLine::Text(wrapped));
                            }
                        }
                    }
                }
            }
            _ => {
                let text_lines = render_elements(std::slice::from_ref(el), theme);
                for l in text_lines {
                    for wrapped in wrap_line(l, term_cols) {
                        out.push(DisplayLine::Text(wrapped));
                    }
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
    if is_remote_url(src) {
        return false;
    }
    std::path::Path::new(src).is_file()
}

/// Returns `true` when `href` looks like a relative link to a Markdown file
/// (not an anchor, not an HTTP URL, ends with `.md` or `.markdown`).
fn is_local_md_link(href: &str) -> bool {
    if href.starts_with('#') || is_remote_url(href) || href.starts_with("mailto:") {
        return false;
    }
    let lower = href.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

/// Resolve an image/video `src` relative to the document's `base_dir`.
/// Absolute paths and `http(s)://` URLs are returned unchanged.
fn resolve_path(src: &str, base_dir: &Path) -> String {
    if is_remote_url(src) {
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

    // Query terminal dimensions before raw mode so build_display_lines can
    // size image slots correctly.  Falls back to 80×24 if unavailable.
    //
    // Resize::Fit in ratatui-image never upscales — it only scales down.
    // The correct slot height is ceil(img_h_px / cell_h_px): exactly as many
    // rows as the image needs at native pixel size.  window_size() returns
    // actual pixel dimensions via TIOCGWINSZ (works before raw mode).
    let (term_cols, term_rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let cell_h: u16 = crossterm::terminal::window_size()
        .ok()
        .and_then(|ws| {
            if ws.rows > 0 && ws.height > 0 {
                Some((ws.height / ws.rows).max(1))
            } else {
                None
            }
        })
        .unwrap_or(16); // assume 16 px per row when unavailable
    let max_h: u16 = (term_cols / 2).min(term_rows.saturating_sub(2)).max(1);
    let img_fallback_h: u16 = (term_cols / 4).clamp(1, max_h);

    let (display_lines, thumb_files) = build_display_lines(
        &elements,
        base_dir,
        theme,
        term_cols as usize,
        cell_h,
        max_h,
        img_fallback_h,
    );
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

    // Shared flag: set to true when this TUI session ends so in-flight fetch
    // threads skip sending results to the already-dropped receiver.
    let shutdown = Arc::new(AtomicBool::new(false));

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
            // Use a row-budget rather than an entry count: an Image entry
            // consumes `height` rows, so taking by entry count can
            // synchronously load far more images than fit on screen.
            let mut rows_seen = 0usize;
            let to_schedule: Vec<(String, bool)> = app
                .lines
                .iter()
                .skip(app.scroll)
                .take_while(|dl| {
                    if rows_seen >= lookahead {
                        return false;
                    }
                    rows_seen += match dl {
                        DisplayLine::Image { height, .. } => *height as usize,
                        DisplayLine::Text(_) => 1,
                    };
                    true
                })
                .filter_map(|dl| match dl {
                    DisplayLine::Image { src, .. }
                        if !app.image_states.contains_key(src)
                            && !app.failed_images.contains(src)
                            && !app.pending_fetches.contains(src) =>
                    {
                        let remote = is_remote_url(src);
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
                    let flag = Arc::clone(&shutdown);
                    std::thread::spawn(move || {
                        if flag.load(Ordering::Relaxed) {
                            return;
                        }
                        let res = crate::image::load_image_url(&src).map_err(|e| e.to_string());
                        if !flag.load(Ordering::Relaxed) {
                            let _ = tx.send((src, res));
                        }
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
                            y_offset += *height;
                        } else if app.pending_fetches.contains(src) {
                            let slot_h =
                                (*height).min(content_area.height.saturating_sub(y_offset));
                            f.render_widget(
                                Paragraph::new(Line::from(Span::styled(
                                    " [loading\u{2026}]",
                                    Style::default().fg(Color::DarkGray),
                                ))),
                                Rect {
                                    x: content_area.x,
                                    y: content_area.y + y_offset,
                                    width: content_area.width,
                                    height: slot_h,
                                },
                            );
                            y_offset += *height;
                        } else if app.failed_images.contains(src) {
                            let safe_src = safe_error_url(src);
                            let label: &str = &safe_src[..safe_src
                                .char_indices()
                                .nth(60)
                                .map(|(i, _)| i)
                                .unwrap_or(safe_src.len())];
                            let dim = Style::default().fg(Color::DarkGray);
                            let slot_h =
                                (*height).min(content_area.height.saturating_sub(y_offset));
                            f.render_widget(
                                Paragraph::new(
                                    Line::from(vec![
                                        Span::raw(" [image unavailable: "),
                                        Span::raw(label.to_owned()),
                                        Span::raw("]"),
                                    ])
                                    .style(dim),
                                ),
                                Rect {
                                    x: content_area.x,
                                    y: content_area.y + y_offset,
                                    width: content_area.width,
                                    height: slot_h,
                                },
                            );
                            y_offset += *height;
                        } else {
                            // Not yet scheduled (concurrency cap hit) or first frame.
                            // Show loading indicator so the gap is never silently blank.
                            let slot_h =
                                (*height).min(content_area.height.saturating_sub(y_offset));
                            f.render_widget(
                                Paragraph::new(Line::from(Span::styled(
                                    " [loading\u{2026}]",
                                    Style::default().fg(Color::DarkGray),
                                ))),
                                Rect {
                                    x: content_area.x,
                                    y: content_area.y + y_offset,
                                    width: content_area.width,
                                    height: slot_h,
                                },
                            );
                            y_offset += *height;
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

    // Signal any in-flight fetch threads that this session is over.
    shutdown.store(true, Ordering::Relaxed);

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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};

    // ── wrap_line ────────────────────────────────────────────────────────────────

    #[test]
    fn wrap_line_short_text_unchanged() {
        let line = Line::from("hello world");
        let result = wrap_line(line, 80);
        assert_eq!(result.len(), 1);
        let text: String = result[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "hello world");
    }

    #[test]
    fn wrap_line_exact_width_unchanged() {
        let line = Line::from("hello");
        assert_eq!(wrap_line(line, 5).len(), 1);
    }

    #[test]
    fn wrap_line_wraps_at_word_boundary() {
        // "hello world" = 11 chars; max 8 → break between words
        let line = Line::from("hello world");
        let result = wrap_line(line, 8);
        assert_eq!(result.len(), 2);
        let first: String = result[0].spans.iter().map(|s| s.content.as_ref()).collect();
        let second: String = result[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(first, "hello");
        assert_eq!(second, "world");
    }

    #[test]
    fn wrap_line_hard_breaks_long_word() {
        // "abcdefghij" = 10 chars; max 4 → 3 lines (4 + 4 + 2)
        let line = Line::from("abcdefghij");
        let result = wrap_line(line, 4);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn wrap_line_preserves_span_styles() {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let line = Line::from(vec![
            Span::raw("plain "),
            Span::styled("bold", bold),
            Span::raw(" plain"),
        ]);
        // "plain bold plain" = 16 chars; max 10 → wraps
        let result = wrap_line(line, 10);
        assert!(result.len() >= 2);
        let has_bold = result.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.style.add_modifier.contains(Modifier::BOLD))
        });
        assert!(has_bold, "bold style must survive wrapping");
    }

    // ── image_slot_height ────────────────────────────────────────────────────────
    // Tests use term_cols=80, cell_h=16 (typical 16 px per terminal row).

    #[test]
    fn image_slot_height_fallback_for_remote() {
        let h = image_slot_height("https://example.com/img.png", 80, 16, 40, 20);
        assert_eq!(h, 20, "remote URL must return fallback");
    }

    #[test]
    fn image_slot_height_fallback_for_missing_file() {
        let h = image_slot_height("/nonexistent/img.png", 80, 16, 40, 17);
        assert_eq!(h, 17, "unreadable file must return fallback");
    }

    #[test]
    fn image_slot_height_square_image() {
        // logo.png 256×256; natural=ceil(256/16)=16, width_limited=ceil(256*80/(256*2))=40 → 16
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/img/logo.png");
        let h = image_slot_height(path, 80, 16, 40, 99);
        assert_eq!(h, 16, "256×256 at 16 px/row, 80 cols → 16 rows");
    }

    #[test]
    fn image_slot_height_landscape_image() {
        // demo.png 320×120; natural=ceil(120/16)=8, width_limited=ceil(120*80/(320*2))=15 → 8
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/screenshots/demo.png");
        let h = image_slot_height(path, 80, 16, 40, 99);
        assert_eq!(h, 8, "landscape 120 px tall, 80 cols → 8 rows");
    }

    #[test]
    fn image_slot_height_clamped_by_max_h() {
        // logo.png: natural=16, max_h=10 clamps result to 10
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/img/logo.png");
        let h = image_slot_height(path, 80, 16, 10, 99);
        assert_eq!(h, 10, "result must be clamped to max_h");
    }

    #[test]
    fn image_slot_height_wide_image_width_limited() {
        // Very wide image: natural height would be large, but width constraint wins.
        // logo.png 256×256 at 20 cols: natural=16, width_limited=ceil(256*20/(256*2))=10 → 10
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/img/logo.png");
        let h = image_slot_height(path, 20, 16, 40, 99);
        assert_eq!(h, 10, "width-limited at 20 cols overrides natural height");
    }
}

//! A debugging aid: with `GPUI_LAYOUT_AUDIT=<file>` set, every frame writes
//! each painted div's box, corners, borders, padding and parent to `<file>`
//! as JSON, so a script can check the real layout (nested corners, insets,
//! clipped children) instead of guessing from the code that built it.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::{AbsoluteLength, Bounds, Pixels, Style, size};

#[derive(serde::Serialize)]
struct Rec {
    parent: i64,
    /// x, y, width, height in logical pixels.
    b: [f32; 4],
    /// Corner radii: top-left, top-right, bottom-right, bottom-left.
    r: [f32; 4],
    /// Border widths and padding: top, right, bottom, left.
    border: [f32; 4],
    pad: [f32; 4],
    bg: bool,
    /// Where the element was built, in debug builds.
    at: String,
}

#[derive(Default)]
struct State {
    recs: Vec<Rec>,
    stack: Vec<usize>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn target() -> Option<&'static PathBuf> {
    static TARGET: OnceLock<Option<PathBuf>> = OnceLock::new();
    TARGET
        .get_or_init(|| std::env::var_os("GPUI_LAYOUT_AUDIT").map(PathBuf::from))
        .as_ref()
}

pub(crate) fn enabled() -> bool {
    target().is_some()
}

pub(crate) fn enter(
    bounds: Bounds<Pixels>,
    style: &Style,
    rem: Pixels,
    at: Option<&'static core::panic::Location<'static>>,
) {
    let px = |p: Pixels| p.as_f32();
    let r = style.corner_radii.clone().to_pixels(rem);
    let border = style.border_widths.clone().to_pixels(rem);
    let parent_size = size(
        AbsoluteLength::Pixels(bounds.size.width),
        AbsoluteLength::Pixels(bounds.size.height),
    );
    let pad = style.padding.clone().to_pixels(parent_size, rem);
    let rec = Rec {
        parent: -1,
        b: [
            px(bounds.origin.x),
            px(bounds.origin.y),
            px(bounds.size.width),
            px(bounds.size.height),
        ],
        r: [px(r.top_left), px(r.top_right), px(r.bottom_right), px(r.bottom_left)],
        border: [px(border.top), px(border.right), px(border.bottom), px(border.left)],
        pad: [px(pad.top), px(pad.right), px(pad.bottom), px(pad.left)],
        bg: style.background.is_some(),
        at: at.map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default(),
    };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let parent = s.stack.last().map(|p| *p as i64).unwrap_or(-1);
        let ix = s.recs.len();
        s.recs.push(Rec { parent, ..rec });
        s.stack.push(ix);
    });
}

pub(crate) fn exit() {
    STATE.with(|s| {
        s.borrow_mut().stack.pop();
    });
}

/// Called when a frame is drawn: writes what it painted and starts over.
pub(crate) fn flush() {
    let Some(path) = target() else { return };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !s.recs.is_empty() {
            if let Ok(json) = serde_json::to_vec(&s.recs) {
                let _ = std::fs::write(path, json);
            }
        }
        s.recs.clear();
        s.stack.clear();
    });
}

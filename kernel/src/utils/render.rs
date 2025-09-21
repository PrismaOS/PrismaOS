//! Small helper utilities that write to a provided ScrollingTextRenderer.
//!
//! These helpers are intentionally tiny adapters so higher-level init code
//! can write informational lines to a renderer instance without relying on
//! global renderer macros.

use crate::scrolling_text::ScrollingTextRenderer;

/// Write a single line using the supplied renderer (advances to next line).
///
/// This mirrors the behavior of `kprintln!` but targets an explicit
/// `ScrollingTextRenderer` instance.
pub fn kprintln_with_renderer(renderer: &mut ScrollingTextRenderer, text: &str) {
    renderer.write_line(text.as_bytes());
}

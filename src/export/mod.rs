//! Export functionality for converting notes to various formats.
//!
//! Supports HTML, PDF, and static site generation with customizable
//! templates and CSS themes.

mod html;
pub mod links;
pub mod site;
pub mod template;
mod theme;

pub use html::markdown_to_html;
pub use links::{BrokenLinkHandling, LinkResolution, LinkResolver, LinkResolverOptions};
pub use site::{SiteConfig, SiteResult, generate_site};
pub use template::{DEFAULT_NOTE_TEMPLATE, render_note_html};
pub use theme::{THEME_DARK, THEME_DEFAULT, get_theme_css};

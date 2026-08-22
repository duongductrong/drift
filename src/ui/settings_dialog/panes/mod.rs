// ---------------------------------------------------------------------------
// Panes — one module per sidebar category.
//
// Every pane exposes a single `rows` function returning the rows its category
// shows, so the shell can swap one for another without knowing what is in it.
// A pane owns its category's settings entirely: nothing outside this directory
// mentions a theme, a provider or an update channel, which is what keeps
// adding a category from touching the dialog at all.
// ---------------------------------------------------------------------------

pub(super) mod about;
pub(super) mod appearance;
pub(super) mod dashboard;
pub(super) mod data_sources;
pub(super) mod scanning;
pub(super) mod updates;

mod add_to_setlist;
mod attachment_viewer;
mod capture;
// Not a screen and has no route: the one component that draws a captured page,
// mounted by `song_detail`'s card and by `attachment_viewer`. See its header —
// a shared component is how those two are made to agree about what a captured
// page looks like, the way one rasterised PNG makes them agree about a PDF.
mod captured_page;
mod chart_editor;
// Not a screen and has no route either: the one place a chart is drawn
// full-screen, mounted by `attachment_viewer` and by `performance`. See its
// header — the same "one component is how two screens are made to agree"
// argument `captured_page` makes, applied to the picture rather than to one
// kind of it.
mod chart_surface;
// The filters sheet (card G3): the sheet behind the library's `Filter` chip.
// No wireframe drew it — see its own header.
mod filter_sheet;
// First run (`1r`, card H3): what `Route::Library` renders instead of
// `Library` while the book is empty. See its own header for the whole
// argument, and `crate::derive::first_run_active` for the decision itself.
mod first_run;
// Not a screen and has no route: the pick-validate-replace-reload pipeline
// card I2's two doors — Settings' Backup row and first run's secondary line —
// both drive. See its own header for why the confirmation it draws runs
// before the file picker rather than after.
mod import_flow;
mod library;
mod performance;
// The running order (F3): the sheet behind performance mode's `≡ set` chip.
// A sheet and not a screen, and mounted in `crate::app` beside the other three
// rather than inside `performance.rs` — its header says why both ways round.
mod running_order;
// Search & filter (`1p`, card G1): the screen behind the library's search
// field, which since that card types nothing itself.
mod search;
mod setlist_detail;
mod setlist_picker;
mod setlists;
mod settings;
mod song_detail;
mod song_form;
mod sort_sheet;
mod stub;
// The default-tuning sheet (card H5): the sheet behind Settings' `Default
// tuning` row. No wireframe drew it — see its own header for why a sheet and
// not a cycling `choice_row`.
mod tuning_sheet;

pub use add_to_setlist::AddToSetlistSheet;
pub use attachment_viewer::AttachmentViewer;
pub use capture::CaptureScreen;
pub use chart_editor::ChartEditor;
pub use filter_sheet::FilterSheet;
pub use first_run::FirstRun;
pub use library::Library;
pub use performance::Performance;
pub use running_order::RunningOrderSheet;
pub use search::Search;
pub use setlist_detail::SetlistDetail;
pub use setlist_picker::SetlistPicker;
pub use setlists::Setlists;
pub use settings::Settings;
pub use song_detail::SongDetail;
pub use song_form::SongForm;
pub use sort_sheet::SortGroupSheet;
// No route points at this since H1 built the real Settings screen; its own
// header says why it is still here and when to delete it.
#[allow(unused_imports)]
pub use stub::Stub;
pub use tuning_sheet::TuningSheet;

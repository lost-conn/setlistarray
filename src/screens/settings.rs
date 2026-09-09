//! Settings — WIREFRAME (`1q`), card H1, styled with the hi-fi token set.
//!
//! Grouped rows and no account section, because there is no account: the last
//! line on the screen is the whole product promise and it is checked, verbatim,
//! against the handoff by a test at the bottom of this file.
//!
//! Reached from the gear in *both* tab headers and from nowhere else — there is
//! no nav item for it, which the handoff is explicit about and
//! `crate::bottom_nav` has said in a comment since the day it was written. That
//! is the fact the back affordance here has to survive: ← cannot assume Songs.
//! It does not — [`NavStore::back`] lands on the *current tab's* root, and the
//! gear does not change the tab, so opening Settings from Setlists and pressing
//! ← returns to Setlists. Nothing on this screen needs to know which door it
//! came in by.
//!
//! ## What is built, and what is deliberately not
//!
//! `1q` draws thirteen rows. Several of them belong to cards that do not exist
//! yet, and a row that looks tappable and does nothing is worse than a row that
//! is not there — so the ones without an engine behind them are absent, and
//! absent *on the record* rather than by oversight. The screen's shape is a
//! decision, and this is where it is written down.
//!
//! **Built, because the state exists, persists and takes effect today:**
//!
//! | row | where the state lives |
//! | --- | --- |
//! | Attachments on device | `AttachmentsStore::total_bytes` |
//! | Saved webpages | a count over `AttachmentsStore::items` |
//! | Re-check saved pages | `SettingsStore::recheck_saved_pages` |
//! | Default tuning | `SettingsStore::default_tuning` |
//! | Library sort | `LibraryViewStore::sort_field` / `sort_dir` |
//! | Library density | `LibraryViewStore::density` |
//! | Keep screen awake while playing | `SettingsStore::keep_awake` |
//! | Accent | `SettingsStore::accent` |
//! | Performance mode theme | `SettingsStore::performance_theme` |
//! | Dark mode | `SettingsStore::dark_mode` |
//! | Export library (.zip) | `crate::export::build_zip`, card I1 |
//!
//! **Left to the card that owns it:**
//!
//! * **Import from file** — card I2, which has a real decision in front of it
//!   (replace or merge) that a row here cannot make on its behalf.
//! * ~~**Last export**~~ — built by card I3, and it turned out not to want a
//!   row of its own at all. `Preferences::last_export_at` reads back on the
//!   right of the Export row itself, as the note that sits there whenever the
//!   last tap has nothing left to report. A separate row would have been a
//!   second line in the section saying something about the same action the
//!   line above it names, which is the beginning of a nag; I3's instruction
//!   was the opposite. See [`crate::derive::last_export_note`].
//! * The **`›` chevrons** on the two storage rows. `1q` draws both as gateways
//!   to a breakdown screen; no such screen exists and no card describes one, so
//!   the two rows are read-only here. The number is the whole of what they can
//!   honestly offer, and it is offered without pretending to lead anywhere.
//! * **"Dark mode follows system"** — the handoff asks for it and it cannot be
//!   built: nothing in this app or in Rinch can read the system's light/dark
//!   preference. `rinch-theme` has a `dark_mode` flag an app *sets*; there is no
//!   API that reports what Android's night mode is set to, and no `#[cfg]` here
//!   would help because the value does not exist to be read. So the row is a
//!   plain on/off switch over `SettingsStore::dark_mode`, which is what that
//!   signal has always been. This is the same shape as
//!   `AccentChoice::FromSystem`, whose own TODO records the same missing
//!   platform call, and it should land in the same card that lands that one.
//!
//! Two of these were checked rather than assumed, because the card asked and
//! the answers went opposite ways. **Library sort** is real: `LibraryViewStore`
//! has held `sort_field` and `sort_dir`, persisted, since the sort sheet (`2c`)
//! was built — so the row states them and opens that sheet, and there are not
//! two ways to change one setting. **Library density** is real too, and this
//! screen is the first thing that can reach it: `Density` is persisted, and
//! `library.rs` has been reading it to pick a compact row since it was written,
//! but `LibraryViewStore::toggle_density` had no caller at all. Card H4 is
//! *"compact density everywhere it applies, plus the alphabet scrubber rail"* —
//! the everywhere-else and the rail are still H4's; the switch that turns it on
//! is here, because the handoff says in as many words that density is "set in
//! Settings → Library density".
//!
//! ## Default tuning opens a sheet, and only prefills — it never rewrites
//!
//! Card H5. `1q` draws `Default tuning · Standard ▾` and nothing behind it
//! existed until this card: no `default_tuning` field, no line in
//! `schema.rhype`, no prefill in `song_form`. Two decisions had to be made
//! before this row could exist at all, and both are written down at the type
//! that carries them rather than here, so a reader who only has this file
//! open still gets the reasoning:
//!
//! * **The value is one of seven fixed tunings, not free text and not a list
//!   built from the book's own songs.** `Song::tuning` stays free text —
//!   [`crate::store::DefaultTuning`]'s own doc comment is where that split is
//!   argued.
//! * **Prefill, never retroactive meaning.** The preference fills a *new*
//!   song's Tuning field, once, at the moment `song_form::save` creates it;
//!   changing the preference afterwards cannot and does not reach back into a
//!   song that already exists. `song_form::blank_draft` is the one place this
//!   is read, and its own doc comment and the test beside it are where that
//!   promise is kept honest.
//!
//! What is decided here, in this file, is the control: a `link_row` — the
//! same shape "Library sort" already uses — rather than a `choice_row` of
//! seven chips. `crate::screens::tuning_sheet`'s own header has the reasoning
//! for that half.
//!
//! ## The accent row is four named chips, and still not a door
//!
//! Card H1 shipped this as four bare 24px swatches on one line — colour alone
//! naming nothing, with the row's right edge (`derive::accent_note`) the only
//! place the *chosen* one was named. Card H2 owns the accent picker itself and
//! turns each swatch into a chip: a small dot plus the accent's name (Rust ·
//! Pine · Indigo · Plum), so a red/green colour-blind reader — or anyone who
//! has not memorised which hex is which — can tell them apart without relying
//! on a caption that only ever names one of the four.
//!
//! `SettingsStore::accent` already resolves through `AccentChoice::resolve`,
//! already persists, and already repaints the entire app the moment it changes —
//! `crate::app`'s root style closure reads `accent_resolved()`, so a tap here is
//! a live theme change with nothing left to wire. That live repaint is *why* this
//! stays one tap on this screen rather than moving behind a picker sheet: a sheet
//! would hide the very thing that makes the control legible, which is watching
//! the whole app change colour under your thumb. `theme::ACCENTS` holds exactly
//! four values, so four chips is the whole picker; a row that instead opened a
//! screen H2 has not built would be the dead row H1's header was written to
//! avoid, and a row that only *printed* the accent name would be strictly less
//! than what the state can already do. See `accent_row` for the chip styling
//! itself and why it is not `crate::ui::Chip`.
//!
//! What is still deliberately absent is `AccentChoice::FromSystem` — the
//! wallpaper extraction that resolves to Rust today because Rinch exposes no
//! platform call for it (`AccentChoice::resolve`'s own comment carries the
//! rest of this). It is not offered as a fifth chip: choosing it would
//! silently mean Rust, which is a control that lies. The row's own note
//! (`derive::accent_note`) still names the colour actually on screen, so a
//! fresh install reads "Rust" and is telling the truth about the pixels.
//!
//! ## Export, and its three failure states
//!
//! Card I1. The handoff's Backup section names three rows — Export, Import,
//! Last export — and this is the first of the three to have an engine behind
//! it, which is why the section heading returns here rather than staying
//! gone the way H1 left it. `crate::export`'s own header is where the
//! database-consistency argument lives; what belongs here is what a person
//! sees when they tap the row, because the card asks for three distinct
//! outcomes and a row that only ever said "Export" either worked or did not
//! would be reporting one bit where the card wants three.
//!
//! * **Cancelled** — the user closed the save dialog. Not an error, the same
//!   rule [`Picked::Cancelled`](crate::picker::Picked::Cancelled) and
//!   [`Saved::Cancelled`] already keep everywhere else in this app: the note
//!   reads "Cancelled" in the muted ink a fact gets, not the accent a
//!   success gets or the danger red a failure gets.
//! * **Failed** — the zip could not be built ([`crate::export::ExportError`],
//!   which already carries words a person can be shown) or the platform's
//!   save itself refused ([`Saved::Failed`]). Either lands in the same danger-
//!   coloured note `trouble` uses on the song detail screen, because both are
//!   the same fact from the user's chair: the backup they asked for did not
//!   happen.
//! * **Done** — the size the card asks for, read straight off the `Vec<u8>`
//!   [`crate::export::build_zip`] returns rather than a stat call on
//!   whatever the platform actually named the file: the desktop's dialog can
//!   return a path this app could `fs::metadata` afterwards, but Android's
//!   save (`crate::picker`'s own header, "It cannot say what the file
//!   actually got named") hands back nothing to stat at all. The byte count
//!   is the one number both platforms can report honestly, and it is the
//!   same number that landed on disk either way — nothing between the zip
//!   finishing and the picker's `Saved::Done` changes its length.
//!
//! `export_status` is a plain component-local `Signal`, not a store field:
//! nothing else on this screen or any other needs to know whether the last
//! export attempt was cancelled, and a signal that outlived this component
//! would be a fact about a screen that is no longer open. It is reset to
//! `None` at the start of every tap, so a second attempt does not leave a
//! stale success sitting beside a fresh failure.
//!
//! `Preferences::last_export_at` is written the moment [`Saved::Done`]
//! arrives — before the note is set, so a crash between the two would rather
//! lose the on-screen confirmation than lose the record I3 will read — and
//! nowhere else. A cancelled or failed attempt never touches it, which is
//! the whole point of the field: it means "an export completed," not "an
//! export was attempted."
//!
//! ## Every reactive control carries its own colour
//!
//! Card F3 found this on the phone on 2026-09-01: a toggle whose background and
//! text colour both lived on one reactive style repainted the pill and left the
//! label the colour it had been, because a bare text node under a restyled
//! parent does not re-resolve an inherited `color`. Every switch, chip and
//! swatch below therefore declares `color` in a closure of its own, on the
//! element that carries the ink, and never inherits it from a parent that is
//! itself changing. The switches and the choice chips are the two shapes on this
//! screen that change colour under a finger, and both obey it.
//!
//! `onclick` and nothing else — see `src/gesture_reachability.rs`. The whole row
//! is the tap target for a switch, rather than the 42px pill: this is a phone,
//! and the pill is a quarter the width of the thing it is drawn beside.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{
    accent_note, attachments_note, density_label, last_export_note, library_sort_note, on_off,
    performance_theme_label, saved_pages_note,
};
use super::import_flow::{self, ImportStatus};
use crate::model::AttachmentKind;
use crate::picker::{SaveRequest, Saved};
use crate::store::{
    AccentChoice, AttachmentsStore, Density, LibraryViewStore, NavStore, PerformanceTheme,
    SettingsStore, SetlistsStore, SongsStore, Storage,
};
use crate::theme::{
    ACCENTS, SCREEN_PAD, T_BODY, T_CHIP, T_META, T_META_SMALL, T_SCREEN_TITLE, T_SECTION_CAPS,
};
use crate::ui::{IconButton, icon};

/// The last line on the screen, and the reason the screen has no account
/// section. Exact, and a `const` rather than a literal in the markup so the test
/// at the bottom of this file can hold it against the handoff word for word.
pub const PROMISE: &str = "SetListArray · works with no connection. Nothing is uploaded anywhere.";

/// One row: label on the left, whatever states or changes it on the right.
///
/// `hairline-soft` and not `hairline`, matching the library's own rows — a list
/// of settings is a list, and the heavier rule is what this app uses to separate
/// a bar from content rather than one row from the next.
const ROW: &str = "display: flex; align-items: center; gap: 12px; padding: 12px 0; \
    min-height: 44px; border-bottom: 1px solid var(--sla-hairline-soft);";

#[component]
pub fn Settings() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let settings = use_store::<SettingsStore>();
    let view = use_store::<LibraryViewStore>();
    let attachments = use_store::<AttachmentsStore>();
    let storage = use_store::<Storage>();
    let songs = use_store::<SongsStore>();
    let setlists = use_store::<SetlistsStore>();

    // What the last tap of "Export library" did, and nothing more durable
    // than that — see the module header's "Export, and its three failure
    // states" for why this is a plain local signal rather than a store field.
    let export_status = Signal::new(Option::<ExportStatus>::None);

    // Card I2's half of the Backup section: whether the replace-warning strip
    // is open, and what the last import attempt came back with. Both are
    // component-local for the same reason `export_status` is — see
    // `crate::screens::import_flow`'s own header for the flow these drive.
    let confirming_import = Signal::new(false);
    let import_status = Signal::new(Option::<ImportStatus>::None);

    // The phone's Back key does what the ← below does. Note what it does *not*
    // do: the import warning strip and the tuning sheet are both dismissable
    // things this screen can have open, and neither is closed here. The sheet
    // is `NavStore::tuning_sheet_open` and `press_back` takes it first, ahead
    // of this; the warning strip is an inline row on the page rather than a
    // layer over it, so Back leaves the whole screen the way ← does.
    nav.register_back(move || nav.back());

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",

            // The same back row every secondary screen in this app wears, at the
            // same 18px inset. ← and nothing else: there is no action on this
            // screen that belongs in a top bar.
            div { style: "padding: 2px 18px 8px; display: flex; align-items: center;",
                IconButton { glyph: TablerIcon::ChevronLeft, size: 19, onclick: move || nav.back() }
            }

            div { style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD} 20px;")},

                div { style: {format!("{T_SCREEN_TITLE}")}, "Settings" }

                {section(__scope, "Storage")}

                {reading_row(__scope, "Attachments on device", move || {
                    attachments_note(attachments.total_bytes())
                })}
                {reading_row(__scope, "Saved webpages", move || {
                    saved_pages_note(saved_pages(attachments))
                })}
                // The switch persisted for a while before anything read it —
                // the bet card F3 made with the keep-awake toggle and won: a
                // real, stored, tested value first, a screen to redesign
                // never. Card E6 is the reader now: `captured_page::RecheckNote`
                // asks this signal, on the one screen a captured page is
                // opened, before it ever touches a socket.
                {switch_row(__scope, "Re-check saved pages",
                    move || settings.recheck_saved_pages.get(),
                    move || settings.set_recheck_saved_pages(!settings.recheck_saved_pages.get()))}

                {section(__scope, "Backup")}

                {export_row(__scope, storage, export_status)}
                {import_row(
                    __scope, storage, songs, setlists, attachments, view, settings,
                    confirming_import, import_status,
                )}

                {section(__scope, "Defaults")}

                // Opens the tuning sheet (card H5) rather than a `choice_row`
                // of seven chips — see that sheet's own header for why seven
                // options is the point where this app switches shapes.
                // `nav.tuning_sheet_open` is toggled here and nowhere else;
                // the sheet itself closes on the same tap that picks a value.
                {link_row(__scope, "Default tuning",
                    move || settings.default_tuning.get().label().to_string(),
                    move || nav.tuning_sheet_open.set(true))}

                // Opens the sort sheet (`2c`) rather than growing a second way
                // to set the same two signals. The sheet is mounted in
                // `crate::app` as a sibling of the route, above everything, so
                // it slides over this screen exactly as it does over the
                // library — which is the whole reason the sheets live out there.
                {link_row(__scope, "Library sort",
                    move || library_sort_note(view.sort_field.get(), view.sort_dir.get()),
                    move || nav.sort_sheet_open.set(true))}

                {choice_row(__scope, "Library density",
                    [
                        (Density::Comfortable, density_label(Density::Comfortable)),
                        (Density::Compact, density_label(Density::Compact)),
                    ],
                    move || view.density.get(),
                    // Through `toggle_density`, which is the store's own
                    // persisting path, rather than writing the signal here and
                    // leaving the Preferences row behind.
                    move |wanted| if view.density.get() != wanted { view.toggle_density() })}

                {switch_row(__scope, "Keep screen awake while playing",
                    move || settings.keep_awake.get(),
                    move || settings.set_keep_awake(!settings.keep_awake.get()))}

                {accent_row(__scope, settings)}

                {choice_row(__scope, "Performance mode theme",
                    [
                        (PerformanceTheme::FollowApp, performance_theme_label(PerformanceTheme::FollowApp)),
                        (PerformanceTheme::AlwaysDark, performance_theme_label(PerformanceTheme::AlwaysDark)),
                    ],
                    move || settings.performance_theme.get(),
                    move |wanted| settings.set_performance_theme(wanted))}

                {switch_row(__scope, "Dark mode",
                    move || settings.dark_mode.get(),
                    move || settings.toggle_dark())}
            }

            // Outside the scroller, not inside it.
            //
            // `1q` pins this line to the bottom of the content column with
            // `margin-top: auto`, which is right up until the column is taller
            // than the screen — and then the promise scrolls away and the screen
            // ends on "Dark mode". Out here it is the last thing on the screen
            // whatever the list above it does, which is what a promise is for.
            div {
                style: {format!("flex-shrink: 0; padding: 13px {SCREEN_PAD} 16px; \
                                 border-top: 1px solid var(--sla-hairline); {T_META}")},
                {PROMISE}
            }
        }
    }
}

/// What the last tap of the export row produced. See the module header's
/// "Export, and its three failure states" for why these three and not a
/// bare success/failure bit.
#[derive(Clone, Debug, PartialEq)]
enum ExportStatus {
    Done(u64),
    Cancelled,
    Failed(String),
}

/// The row itself: a tap builds the zip and hands it to
/// [`crate::picker::save`], and the note on the right shows whatever the
/// last attempt came back with. Shaped like [`link_row`] — label left, note
/// right — but with no chevron, because tapping this row does not navigate
/// anywhere; it starts and finishes an action in place.
fn export_row(scope: &mut RenderScope, storage: Storage, status: Signal<Option<ExportStatus>>) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            onclick: move || start_export(storage, status),
            style: {ROW},
            span { style: {format!("{T_BODY} flex: 1;")}, "Export library (.zip)" }
            // One span, two jobs, and the order matters (card I3). While the
            // last tap of this row still has something to report, it reports
            // that. With nothing to report — a fresh launch, or a screen
            // reopened later — it falls back to when the library was last
            // exported. The date is the resting state and the status is the
            // interruption, which is the right way round: the status is about
            // the tap you just made and stops being interesting, whereas the
            // date is what somebody opening Settings cold came to find out.
            //
            // `storage.preferences()` is a `Signal` read, so the fallback
            // updates itself the moment `start_export` stamps
            // `last_export_at` — there is no second signal to keep in step.
            span {
                style: {move || format!("{T_META_SMALL} color: {};", export_status_color(status.get()))},
                {move || match status.get() {
                    Some(s) => export_status_note(Some(s)),
                    None => last_export_note(
                        storage.preferences().last_export_at,
                        crate::model::Day::today(),
                    ),
                }}
            }
        }
    }
}

/// The colour a status reads in: muted for "nothing has happened yet" and
/// for a cancel (the same rule `crate::picker`'s own `Cancelled` follows —
/// closing a dialog is not a failure), danger red for a failure, accent for
/// a success — the one time this row's note is good news rather than a
/// plain fact.
fn export_status_color(status: Option<ExportStatus>) -> &'static str {
    match status {
        None | Some(ExportStatus::Cancelled) => "var(--sla-muted)",
        Some(ExportStatus::Failed(_)) => "var(--sla-danger)",
        Some(ExportStatus::Done(_)) => "var(--sla-accent)",
    }
}

/// The words beside the row. Empty before the first tap — this is an action
/// row, not a setting with a resting value, so there is nothing honest to
/// say about it before it has been used once this session.
fn export_status_note(status: Option<ExportStatus>) -> String {
    match status {
        None => String::new(),
        Some(ExportStatus::Cancelled) => "Cancelled".to_string(),
        Some(ExportStatus::Failed(why)) => why,
        Some(ExportStatus::Done(bytes)) => crate::model::fmt_bytes(bytes),
    }
}

/// Build the zip and hand it to the save dialog. Split out of `export_row`'s
/// `onclick` because the body has three exits — no library, a build failure,
/// a save outcome — and a closure that long is harder to read inline than
/// named.
fn start_export(storage: Storage, status: Signal<Option<ExportStatus>>) {
    status.set(None);

    // No database at all — the in-memory fallback `Storage::open` degrades
    // to when the library will not open (its own doc comment), or the shape
    // tests and `--seed` screenshots use on purpose. Either way there is
    // nothing on disk to zip, and the save dialog would be asking for bytes
    // this app cannot produce.
    let Some(repo) = storage.repo() else {
        status.set(Some(ExportStatus::Failed(
            "There is no library on this device to export.".to_string(),
        )));
        return;
    };

    let bytes = match crate::export::build_zip(&repo) {
        Ok(bytes) => bytes,
        Err(e) => {
            status.set(Some(ExportStatus::Failed(e.message())));
            return;
        }
    };
    let size = bytes.len() as u64;

    crate::picker::save(
        SaveRequest {
            title: "Export library",
            kind: "Zip",
            extensions: &["zip"],
            file_name: crate::export::suggested_file_name(),
        },
        bytes,
        move |saved| match saved {
            Saved::Cancelled => status.set(Some(ExportStatus::Cancelled)),
            Saved::Failed(why) => status.set(Some(ExportStatus::Failed(why))),
            Saved::Done => {
                // Before the note, not after: a crash between the two should
                // lose the on-screen confirmation before it loses the record
                // card I3 will read — see the module header.
                storage.remember(|p| p.last_export_at = Some(crate::export::now_ms() as i64));
                status.set(Some(ExportStatus::Done(size)));
            }
        },
    );
}

/// Import's half of card I2's Backup section, beside Export. Shaped like
/// [`export_row`] — label left, status note right, no chevron since a tap
/// does not navigate — with one difference: a tap here does not go straight
/// to the file picker. It raises `confirming`, and
/// [`import_flow::confirm_strip`], drawn directly beneath, is what actually
/// opens the picker once someone has said "replace" rather than merely
/// tapped the row. See that module's own header for why the confirmation has
/// to come before the file is even chosen.
#[allow(clippy::too_many_arguments)]
fn import_row(
    scope: &mut RenderScope,
    storage: Storage,
    songs: SongsStore,
    setlists: SetlistsStore,
    attachments: AttachmentsStore,
    view: LibraryViewStore,
    settings: SettingsStore,
    confirming: Signal<bool>,
    status: Signal<Option<ImportStatus>>,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div { style: "display: flex; flex-direction: column;",
            div {
                onclick: move || confirming.set(true),
                style: {ROW},
                span { style: {format!("{T_BODY} flex: 1;")}, "Import from backup" }
                span {
                    style: {move || format!(
                        "{T_META_SMALL} color: {};",
                        import_flow::status_color(status.get()),
                    )},
                    {move || import_flow::status_note(status.get())}
                }
            }
            {import_flow::confirm_strip(__scope, confirming, move || {
                import_flow::start(storage, songs, setlists, attachments, view, settings, status);
            })}
        }
    }
}

/// How many attachments are saved webpages.
///
/// Derived on read from the list already in memory rather than counted into a
/// field: the startup scan loads every attachment's *metadata* (never its body —
/// see `AttachmentsStore`'s header), so the kind of each one is already here,
/// and a stored count is a number that can disagree with the list it counts.
fn saved_pages(attachments: AttachmentsStore) -> usize {
    attachments
        .items
        .get()
        .iter()
        .filter(|a| a.kind == AttachmentKind::CapturedPage)
        .count()
}

/// A group heading. `section-caps` is the handoff's own token for this.
fn section(scope: &mut RenderScope, label: &'static str) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            style: {format!("{T_SECTION_CAPS} color: var(--sla-muted); padding: 26px 0 2px;")},
            {label}
        }
    }
}

/// A row that states something and cannot be changed from here.
fn reading_row(
    scope: &mut RenderScope,
    label: &'static str,
    value: impl Fn() -> String + Copy + 'static,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div { style: {ROW},
            span { style: {format!("{T_BODY} flex: 1;")}, {label} }
            span { style: {format!("{T_META_SMALL}")}, {move || value()} }
        }
    }
}

/// A row that states something and opens the place it is changed.
fn link_row(
    scope: &mut RenderScope,
    label: &'static str,
    value: impl Fn() -> String + Copy + 'static,
    tap: impl Fn() + 'static,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            onclick: move || tap(),
            style: {ROW},
            span { style: {format!("{T_BODY} flex: 1;")}, {label} }
            span { style: {format!("{T_META_SMALL}")}, {move || value()} }
            span {
                style: "display: flex; align-items: center; color: var(--sla-muted);",
                {icon(__scope, TablerIcon::ChevronRight, 15)}
            }
        }
    }
}

/// A row carrying a switch. The whole row is the target; see the module header.
fn switch_row(
    scope: &mut RenderScope,
    label: &'static str,
    on: impl Fn() -> bool + Copy + 'static,
    tap: impl Fn() + 'static,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            onclick: move || tap(),
            style: {ROW},
            span { style: {format!("{T_BODY} flex: 1;")}, {label} }
            // The word, beside the shape. Its own closure, so it repaints with
            // the pill rather than a frame behind it — see the header.
            span { style: {move || format!("{T_META_SMALL} color: {};", if on() {
                "var(--sla-accent)"
            } else {
                "var(--sla-muted)"
            })}, {move || on_off(on()).to_string()} }
            {switch(__scope, on)}
        }
    }
}

/// The pill itself: accent track and paper knob when on, fill track and muted
/// knob when off, the knob sliding by `justify-content` rather than a transform
/// so there is no percentage translation for rinch's transition engine to drop.
fn switch(scope: &mut RenderScope, on: impl Fn() -> bool + Copy + 'static) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            style: {move || format!(
                "width: 42px; height: 24px; border-radius: 999px; flex-shrink: 0; \
                 display: flex; align-items: center; padding: 0 3px; \
                 justify-content: {}; background: {};",
                if on() { "flex-end" } else { "flex-start" },
                if on() { "var(--sla-accent)" } else { "var(--sla-fill)" },
            )},
            div {
                style: {move || format!(
                    "width: 18px; height: 18px; border-radius: 999px; background: {};",
                    if on() { "var(--sla-on-accent)" } else { "var(--sla-muted)" },
                )},
            }
        }
    }
}

/// A row whose value is one of a short closed list, drawn as chips.
///
/// Generic over the value, because the two rows that want it hold different
/// enums and the alternative is the same fifteen lines twice. Both lists happen
/// to be two long; nothing here assumes that.
fn choice_row<T: Copy + PartialEq + 'static>(
    scope: &mut RenderScope,
    label: &'static str,
    options: [(T, &'static str); 2],
    current: impl Fn() -> T + Copy + 'static,
    choose: impl Fn(T) + Copy + 'static,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div { style: {ROW},
            span { style: {format!("{T_BODY} flex: 1;")}, {label} }
            div { style: "display: flex; gap: 6px; flex-shrink: 0;",
                for option in options {
                    let value = option.0;
                    let text = option.1;
                    let chosen = move || current() == value;
                    div {
                        key: {text},
                        onclick: move || choose(value),
                        style: {move || format!(
                            "{T_CHIP} border-radius: 999px; padding: 6px 11px; white-space: nowrap; \
                             background: {};",
                            if chosen() { "var(--sla-accent-tint)" } else { "var(--sla-fill)" },
                        )},
                        // Its own colour, for the F3 reason in the header.
                        span {
                            style: {move || format!("color: {};", if chosen() {
                                "var(--sla-accent-on-tint)"
                            } else {
                                "var(--sla-muted)"
                            })},
                            {text}
                        }
                    }
                }
            }
        }
    }
}

/// The accent row: label and the name of the colour on screen on their own
/// line, then the four named chips wrapping beneath.
///
/// Two lines rather than one, which is what H1's single-line row of four bare
/// swatches used to fit into. A named chip (dot + word) is wider than a bare
/// 24px circle, and cramming "Accent", its note, and four of them onto one
/// 44px row either truncates the note or crowds the chips into a scroll a
/// four-option control has no business needing. So "Accent" and the note keep
/// `choice_row`'s label/value baseline on their own line, and the chips wrap
/// beneath at the handoff's own "wrapping, 7px gaps" rhythm — the same rule
/// every other chip row on this screen already follows.
///
/// This does not reuse `crate::ui::Chip`: that component paints every chip it
/// draws in one of two shared colours (ink-on-paper for `active`, fill-on-
/// muted-or-ink-2 for the rest), which is right for a row where every chip is
/// choosing among values that share a single accent — sort field, filter
/// facet — but wrong here, where the four chips *are* four different accents
/// and Rust's dot has to read as rust-coloured even while Pine is selected.
/// So this borrows `choice_row`'s shape instead — `T_CHIP`, `999px` radius,
/// `accent-tint`/`accent-on-tint` for the chosen one, `fill`/`muted` for the
/// rest — and keys the tint/on-tint pair to *that chip's own* accent rather
/// than to the one shared `--sla-accent-tint` variable, which only ever holds
/// the accent currently live. The dot's own fill is always that accent's real
/// hex for the same reason: it has to stay Pine-coloured regardless of which
/// chip is selected, which a CSS custom property that follows the live theme
/// cannot do.
///
/// No swatch-preview widget: the live repaint already is the preview. Tapping
/// a chip calls `settings.set_accent`, which flips `accent_resolved()`, which
/// `crate::app`'s root style closure reads to repaint the entire screen tree —
/// see the module header. A second, smaller preview here would just be a
/// slower copy of the one the whole screen already gives for free.
fn accent_row(scope: &mut RenderScope, settings: SettingsStore) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            style: "display: flex; flex-direction: column; gap: 10px; padding: 12px 0; \
                    border-bottom: 1px solid var(--sla-hairline-soft);",
            div { style: "display: flex; align-items: center; gap: 12px;",
                span { style: {format!("{T_BODY} flex: 1;")}, "Accent" }
                span { style: {format!("{T_META_SMALL}")}, {move || accent_note(settings.accent.get()).to_string()} }
            }
            div { style: "display: flex; flex-wrap: wrap; gap: 7px;",
                for index in 0..ACCENTS.len() {
                    let accent = ACCENTS[index];
                    let chosen = move || settings.accent_resolved() == accent;
                    div {
                        key: index,
                        onclick: move || settings.set_accent(AccentChoice::Named(index)),
                        style: {move || format!(
                            "{T_CHIP} display: flex; align-items: center; gap: 6px; \
                             border-radius: 999px; padding: 6px 11px; white-space: nowrap; \
                             background: {};",
                            if chosen() {
                                if settings.dark_mode.get() { accent.tint_dark } else { accent.tint }
                            } else {
                                "var(--sla-fill)"
                            },
                        )},
                        div {
                            style: {move || format!(
                                "width: 10px; height: 10px; border-radius: 999px; flex-shrink: 0; \
                                 background: {};",
                                if settings.dark_mode.get() { accent.base_dark } else { accent.base },
                            )},
                        }
                        // Its own colour, for the F3 reason in the module
                        // header: this span repaints with the chip, so its
                        // `color` is a closure of its own rather than an
                        // inherited value the parent's restyle would leave
                        // behind.
                        span {
                            style: {move || format!("color: {};", if chosen() {
                                if settings.dark_mode.get() { accent.on_tint_dark } else { accent.on_tint }
                            } else {
                                "var(--sla-muted)"
                            })},
                            {accent.name}
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The footer line is the app's central promise, and the handoff is where
    /// it is authored. Held against that file rather than against a copy of
    /// itself, so a reworded footer fails here instead of quietly becoming a
    /// different promise than the one the design makes.
    #[test]
    fn the_footer_is_the_promise_the_handoff_authored() {
        let handoff = include_str!("../../design_handoff_setlistarray/README.md");
        assert!(
            handoff.contains(PROMISE),
            "the footer line no longer matches the handoff: {PROMISE}"
        );
    }

    /// It is also the only sentence on the screen that mentions the network, and
    /// what it says about it is "no". A guard on the two words that carry that.
    #[test]
    fn the_promise_still_promises_no_upload() {
        assert!(PROMISE.contains("no connection"));
        assert!(PROMISE.contains("Nothing is uploaded anywhere."));
    }
}

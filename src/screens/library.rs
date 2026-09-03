//! Library (Songs tab) — HI-FI. The app's home; answers "what can I actually
//! play?".

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::model::{AttachmentKind, Song};
use crate::derive::{
    GROUP_PREVIEW, filter_chip_label, library_subtitle, present_letters, visible_songs,
};
use crate::menu::{FULL_WIDTH_TARGET, MENU_SURFACE, SongMenuItems};
use crate::store::{
    AttachmentsStore, GroupBy, LibraryViewStore, NavStore, Route, SettingsStore, SongsStore,
};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_SCREEN_TITLE};
use crate::ui::{Chip, IconButton, SongRow, group_header, group_rows_style, icon};

/// Where the alphabet scrubber's taps land: one `NodeHandle` per first-letter
/// group, captured the moment that group's header is built (see
/// [`insert_group_header`]) and read back when a letter is tapped (see
/// [`alphabet_rail`]).
///
/// A plain `Rc<RefCell<…>>`, not a `Signal` — nothing here is meant to be
/// reactive to *itself*. It is written from inside one reactive closure (the
/// grouped list's `for`) and read from inside another (the rail's `onclick`
/// handlers), and both already re-run on the signals that actually matter
/// (`group_by`, the library, the filters); wrapping the map in a `Signal` too
/// would just be a second, redundant invalidation source for the same
/// underlying change. See card H4's own writeup, in this file's module
/// comment area, for why this is reachable at all — `rinch-core`'s
/// `NodeHandle::scroll_into_view` (`rinch-core/src/dom/mod.rs:526`) exists,
/// but nothing handed app code a `NodeHandle` for a specific child *through
/// the `#[component]` DSL*; the way out was to stop asking the DSL for one.
type ScrubTargets = Rc<RefCell<HashMap<String, NodeHandle>>>;

#[component]
pub fn Library() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();
    let view = use_store::<LibraryViewStore>();
    let attachments = use_store::<AttachmentsStore>();
    let _settings = use_store::<SettingsStore>();

    // See `ScrubTargets`'s own doc comment for what this is and why a plain
    // `Rc<RefCell<…>>` rather than a store field. Fresh on every mount of this
    // component, which is exactly right: a remount (leaving and returning to
    // the tab) means every header below builds from scratch anyway, so a map
    // surviving past that point would only ever hold handles to nodes that no
    // longer exist.
    let scrub_targets: ScrubTargets = Rc::new(RefCell::new(HashMap::new()));
    // Cloned once per reactive closure that needs it — the grouped list's
    // `for` and the rail's `if` are two separate `move` closures, and a
    // `move` closure takes ownership of whatever it names, `.clone()` call or
    // not. Reusing `scrub_targets` itself in both would let the first closure
    // built move the only copy out from under the second.
    let scrub_targets_for_rows = scrub_targets.clone();
    let scrub_targets_for_rail = scrub_targets.clone();

    // Card J1: each group's last measured open height in pixels, by label.
    // Fresh on every mount of this component for the same reason
    // `scrub_targets` is — a remount rebuilds every row from scratch, so a
    // map surviving past that point would hold heights for nodes that no
    // longer exist. See the `for` loop below and [`group_rows_style`] for
    // how this drives the collapse animation, and what its one gap is.
    let group_heights: Rc<RefCell<HashMap<String, f32>>> = Rc::new(RefCell::new(HashMap::new()));

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0; position: relative;",

            // Title block. The gear is the only way into Settings from here —
            // there is no nav item for it.
            div {
                style: {format!("padding: 6px {SCREEN_PAD} 14px; display: flex; align-items: baseline; gap: 12px;")},
                div { style: "flex: 1;",
                    div { style: {format!("{T_SCREEN_TITLE}")}, "Songs" }
                    div { style: {format!("{T_META} margin-top: 7px;")},
                        {|| library_subtitle(songs.songs.get(), &view.filters.get()).lead}
                        span {
                            style: "color: var(--sla-accent); font-weight: 600;",
                            {|| format!("{} solid", library_subtitle(songs.songs.get(), &view.filters.get()).solid)}
                        }
                    }
                }
                IconButton {
                    glyph: TablerIcon::Settings,
                    size: 18,
                    onclick: move || nav.go(Route::Settings),
                }
            }

            // Search — a door, not a field.
            //
            // It used to be a real `<input>` that filtered this list as you
            // typed, through `view.grouped` and `derive::grouped`. Card G1 took
            // that out: tapping it opens the search & filter screen (`1p`,
            // `super::search`), which is where the typing, the live results,
            // the match highlighting and the count line now are, and where a
            // setlist can be a result too — which this list, being a list of
            // songs, could never have shown.
            //
            // It is a `div` with an `onclick` rather than an input carrying
            // one, and that is not cosmetic. An `<input>` on Android raises the
            // IME on focus; an input with no `oninput` would raise the keyboard
            // and then swallow every key, which is a worse field than no field.
            // The placeholder is drawn as ordinary muted text for the same
            // reason — there is nothing here for a real placeholder to be the
            // absence of.
            div {
                onclick: move || nav.go(Route::Search),
                style: {format!("margin: 0 {SCREEN_PAD}; background: var(--sla-fill); border-radius: 14px; \
                                 padding: 11px 14px; display: flex; align-items: center; gap: 9px;")},
                span { style: "color: var(--sla-muted); display: flex;", {icon(__scope, TablerIcon::Search, 17)} }
                span {
                    style: "flex: 1; font-family: var(--sla-font-ui); font-size: 15px; color: var(--sla-muted);",
                    "Search title, artist, tag…"
                }
            }

            // Chip row. Group is the active chip; sort shows field + direction.
            div {
                style: {format!("display: flex; flex-wrap: wrap; gap: 7px; padding: 12px {SCREEN_PAD} 2px;")},
                Chip {
                    label: {|| format!("Group: {}", view.group_by.get().label())},
                    active: true,
                    selected: false,
                    onclick: move || nav.sort_sheet_open.set(true),
                }
                Chip {
                    label: {|| view.sort_field.get().label().to_string()},
                    glyph: {|| Some(view.sort_dir.get().arrow())},
                    active: false,
                    selected: true,
                    onclick: move || nav.sort_sheet_open.set(true),
                }
                Chip {
                    label: {|| filter_chip_label(view.filters.get().count())},
                    active: {|| view.filters.get().is_active()},
                    selected: false,
                    onclick: move || nav.filter_sheet_open.set(true),
                }
                // The fourth chip the handoff draws (`README.md:145`) and the
                // one this card was cut to add. It is a shortcut to the same
                // signal Settings' "Library density" row writes through
                // `toggle_density` — not a second source of truth, so there is
                // nowhere for the chip and the row to disagree about which
                // density the library is in. No label, the way `1q`'s wireframe
                // draws it: `≣` alone, and `LayoutRows` is the outline glyph in
                // this icon set that reads as stacked rows rather than a
                // hamburger menu, which is what `Menu2` would have signalled
                // instead.
                Chip {
                    label: "",
                    glyph: TablerIcon::LayoutRows,
                    active: {|| view.density.get().is_compact()},
                    selected: false,
                    onclick: move || view.toggle_density(),
                }
            }

            // The grouped list. Right padding grows while the alphabet
            // scrubber is showing (card H4) — otherwise its rail sits on top
            // of the last few letters of the longest row titles, which the
            // 34px is measured to clear: 18px of rail plus the same breathing
            // room the screen pad already gives the left edge.
            div {
                style: {move || format!(
                    "flex: 1; min-height: 0; overflow-y: auto; padding: 0 {} 90px;",
                    if view.group_by.get() == GroupBy::FirstLetter { "34px" } else { SCREEN_PAD }
                )},
                for (index, group) in view.grouped(songs.songs.get()).into_iter().enumerate() {
                    // The `for` body re-runs as a closure, so everything it
                    // needs is cloned or copied up front.
                    let label = group.label.clone();
                    let total = group.songs.len();
                    let collapsed = view.is_collapsed(&label);
                    let expanded = view.is_expanded(&label);
                    let hidden = total.saturating_sub(GROUP_PREVIEW);

                    {group_entry(
                        __scope,
                        view,
                        songs,
                        attachments,
                        nav,
                        group_heights.clone(),
                        scrub_targets_for_rows.clone(),
                        index,
                        label,
                        total,
                        index == 0,
                        collapsed,
                        expanded,
                        hidden,
                    )}
                }

                // This screen used to answer "the book is empty" here too —
                // "Nothing here yet. Add the first song you know how to
                // play." — and that branch is gone (card H3). Two screens
                // answering one question was one too many, and the better
                // answer does not belong inside a scrolling list of zero
                // rows: `crate::app`'s `Route::Library` arm now renders
                // `screens::FirstRun` in place of this whole screen while
                // `derive::first_run_active` says the book is empty, so this
                // component is never even mounted for that case. What is
                // left below is a genuinely different question — the book
                // has songs, a filter has narrowed the view to none of them
                // (card G3) — and it keeps its own answer.
                if view.filters.get().is_active() && view.grouped(songs.songs.get()).is_empty() {
                    div { style: {format!("{T_META_SMALL} text-align: center; padding: 48px 0;")},
                        div { "Nothing in your book matches these filters." }
                        div {
                            onclick: move || view.clear_filters(),
                            style: "color: var(--sla-accent); font-weight: 600; margin-top: 10px;",
                            "Clear filters"
                        }
                    }
                }
            }

            // The alphabet scrubber (`1c`, card H4) — only while grouping is
            // First letter, since a scrubber beside groups called "Solid" and
            // "Rusty" would be a rail promising letters this list is not
            // sorted by. Reactive on `group_by` (read in the condition below)
            // and, inside `alphabet_rail`, on whatever `present_letters` reads
            // through `view.grouped` — the same songs/filters/sort dependency
            // the list above already tracks — so the live/dim split follows a
            // filter narrowing the book exactly as promptly as the group list
            // beside it does.
            if view.group_by.get() == GroupBy::FirstLetter {
                {alphabet_rail(
                    __scope,
                    present_letters(&view.grouped(songs.songs.get())),
                    scrub_targets_for_rail.clone(),
                )}
            }

            // FAB — creates a song.
            //
            // `z-index` is load-bearing, not decoration: without it the FAB
            // is neither drawn nor tappable. Rinch implements no CSS painting
            // step 8, so a `position: absolute; z-index: auto` box stays in the
            // in-flow phase; and it makes an `overflow: auto` box a stacking
            // context (`Node::creates_stacking_context`), so the scrolling list
            // is hoisted into the z-index-0 phase that paints last and is
            // hit-tested first. The list therefore covered the FAB and swallowed
            // every tap on it. Naming a layer moves the FAB into that same phase,
            // below the bottom sheets' 40. joeleaver/rinch#292 fixes it upstream
            // by deriving paint order and hit-test order from one sequence; this
            // stays until the pin moves onto a rinch that carries it.
            div {
                onclick: move || nav.go(Route::AddSong),
                style: "position: absolute; right: 20px; bottom: 16px; z-index: 10; \
                        width: 60px; height: 60px; \
                        border-radius: 20px; background: var(--sla-accent); color: var(--sla-on-accent); \
                        display: flex; align-items: center; justify-content: center; \
                        box-shadow: 0 8px 18px -4px rgba(181,71,36,.5);",
                {icon(__scope, TablerIcon::Plus, 24)}
            }
        }
    }
}

/// Builds one group's header and, while grouping is First letter, remembers
/// where it landed — folding [`crate::ui::group_header`] and the scrubber's
/// bookkeeping into one call so the `for` loop above stays a single
/// expression per header rather than a `let` plus a side-effecting statement,
/// which the rsx macro's children grammar does not have a clean shape for (a
/// bare `if` in that position parses as *reactive conditional content*, not a
/// plain Rust statement — see this file's own card-H4 writeup for the
/// reachability question this sidesteps).
///
/// Takes `group_by` rather than reading `LibraryViewStore` itself so this
/// stays a function of its arguments — the same reason [`songs_in_group`]
/// below takes `index` instead of re-deciding which group it means.
#[allow(clippy::too_many_arguments)]
fn insert_group_header(
    scope: &mut RenderScope,
    targets: ScrubTargets,
    group_by: GroupBy,
    label: String,
    count: usize,
    is_first: bool,
    collapsed: bool,
    compact: bool,
    onclick: impl Fn() + 'static,
) -> NodeHandle {
    let header = group_header(scope, label.clone(), count, is_first, collapsed, compact, onclick);
    if group_by == GroupBy::FirstLetter {
        targets.borrow_mut().insert(label, header.clone());
    }
    header
}

/// The alphabet scrubber (`1c`): a vertical A–Z rail on the right edge, shown
/// only while grouping is First letter. Card H4.
///
/// **Card K46 said this could not be built** — "Rinch exposes no scroll-to-
/// child call. There is no `scrollIntoView`" — and the first half of that was
/// already wrong when it was written: `NodeHandle::scroll_into_view`
/// (`rinch-core/src/dom/mod.rs:526`) has existed since before this card,
/// backed by `request_scroll_into_view` / `drain_scroll_into_view_requests`
/// and applied after layout by `rinch/src/app/mod.rs`'s `apply_scroll_into_view`.
/// What was true, and what K46 was actually running into, is the second half:
/// nothing handed app code a `NodeHandle` for a specific child. A
/// `#[component]` invoked as `Name { props }` is spliced into its parent
/// entirely inside the macro's own codegen — there is no syntax in the rsx
/// grammar for capturing the handle of one call among many, and a search of
/// `../rinch-fixes` turns up no `NodeRef`, `use_node`, `ElementRef`, or `ref:`
/// prop anywhere in the framework or its own components. That is a real gap,
/// but it is a gap in the *component* DSL, not in the DOM API underneath it —
/// a plain function that happens to return `NodeHandle`, called as an
/// ordinary Rust expression the way `settings.rs`'s row helpers already are,
/// hands its return value back like any other function call. `group_header`
/// (`crate::ui`) was converted to exactly that shape for this card, and
/// [`insert_group_header`] above is where the handle gets kept. So the
/// premise is wrong and the rail is real, not a card someone still has to
/// build once rinch grows a ref API — see this file's Cargo history for the
/// PascalCase `GroupHeader` this replaced.
///
/// Letters with no group under them are drawn, not hidden — a hole in the
/// alphabet would look like a bug — but dimmed to the hairline colour and
/// wired to nothing rather than made to jump to the nearest letter that does
/// have songs. A silent jump is the more "helpful" of the two only if the
/// user cannot tell it happened; on a one-letter-tall target the eye has
/// already left the rail by the time the list finishes moving, and the
/// honest answer to "does Q have songs" is dimming it, not moving the goalpost
/// until it does.
fn alphabet_rail(
    scope: &mut RenderScope,
    present: std::collections::BTreeSet<String>,
    targets: ScrubTargets,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            style: "position: absolute; right: 8px; top: 6px; bottom: 96px; z-index: 5; \
                    display: flex; flex-direction: column; align-items: center; \
                    justify-content: center; padding: 2px 3px;",
            for letter in ('A'..='Z').map(|c| c.to_string()) {
                let live = present.contains(&letter);
                let jump_targets = targets.clone();
                let jump_letter = letter.clone();
                span {
                    key: {letter.clone()},
                    onclick: move || {
                        if let Some(handle) = jump_targets.borrow().get(&jump_letter) {
                            handle.scroll_into_view();
                        }
                    },
                    style: {
                        let color = if live { "var(--sla-muted)" } else { "var(--sla-hairline)" };
                        format!(
                            "font-size: 9px; line-height: 1.7; font-weight: 600; color: {color};"
                        )
                    },
                    {letter.clone()}
                }
            }
        }
    }
}

/// One group: its header, its rows (if mounted), and its "Show N more" row
/// (if any) — built and wired up as a plain function rather than inline `rsx!`
/// control flow, and appended imperatively via [`NodeHandle::append_child`].
///
/// That is a deliberate downgrade from the `if !collapsed { … }` this
/// replaced, for the same reason [`group_header`] became a plain function
/// for card H4: card J1's collapse animation needs the rows' own
/// `NodeHandle` back so the header's `onclick` can read `scroll_height()`
/// off it, and reactive `if`/`if let` sugar inside `rsx!` re-runs its body as
/// a closure that has to be callable more than once — which is exactly what
/// broke here first: an `Option<NodeHandle>` built once and *moved* into
/// that closure could not also be read back out for the "Show more" row
/// beside it. Building the container by hand and appending each piece once
/// sidesteps the question entirely; nothing here needs to re-run on its own,
/// because the whole group is already inside the `for` loop in [`Library`]
/// that recreates it from scratch on every relevant change.
///
/// Card J1's collapse animation. `group_heights` (declared in [`Library`],
/// alongside `scrub_targets`, for the same fresh-per-mount reason) remembers
/// each group's last measured open height in pixels, keyed by label — and a
/// group that has never been in that map yet renders exactly as it did
/// before this card: mounted only while `!collapsed`, nothing measured,
/// nothing animated. See [`crate::ui::group_rows_style`] for why a
/// *measured* group stays mounted even while collapsed, and its doc comment
/// plus `crate::ui`'s `SHEET_EASE` block for why `rinch`'s transition engine
/// leaves exactly one gap in that plan: the very first close of any group,
/// every session, snaps instead of animating, because there is no previous
/// *pixel* height on record yet for the diff engine to interpolate away
/// from — only that one transition; every open and close after it, for that
/// same group, animates.
#[allow(clippy::too_many_arguments)]
fn group_entry(
    scope: &mut RenderScope,
    view: LibraryViewStore,
    songs: SongsStore,
    attachments: AttachmentsStore,
    nav: NavStore,
    group_heights: Rc<RefCell<HashMap<String, f32>>>,
    scrub_targets: ScrubTargets,
    index: usize,
    label: String,
    total: usize,
    is_first: bool,
    collapsed: bool,
    expanded: bool,
    hidden: usize,
) -> NodeHandle {
    let __scope = scope;

    let known_height = group_heights.borrow().get(&label).copied();
    let mount_rows = !collapsed || known_height.is_some();

    let rows_handle = if mount_rows {
        let style = match known_height {
            Some(h) => group_rows_style(collapsed, h),
            None => String::new(),
        };
        Some(group_rows(__scope, view, songs, attachments, nav, index, style))
    } else {
        None
    };

    let onclick = {
        let label = label.clone();
        let rows_handle = rows_handle.clone();
        move || {
            // Measured *before* the toggle flips, while the rows (if
            // mounted) still reflect whatever this group's layout settled
            // to last frame — `NodeHandle::scroll_height` is a live query
            // against that resolved layout, not a snapshot taken when the
            // handle was built, so this is accurate however long ago that
            // frame was.
            if let Some(handle) = &rows_handle {
                let measured = handle.scroll_height() as f32;
                if measured > 0.0 {
                    group_heights.borrow_mut().insert(label.clone(), measured);
                }
            }
            view.toggle_collapsed(label.clone());
        }
    };

    let header = insert_group_header(
        __scope,
        scrub_targets,
        view.group_by.get(),
        label.clone(),
        total,
        is_first,
        collapsed,
        view.density.get().is_compact(),
        onclick,
    );

    let container = rsx! { div { key: {label.clone()} } };
    container.append_child(&header);
    if let Some(rows) = &rows_handle {
        container.append_child(rows);
    }

    // Per-group truncation, until the user asks for the rest.
    if !collapsed && total > GROUP_PREVIEW && !expanded {
        let more = rsx! {
            div {
                onclick: move || expand_group(view, songs, index),
                style: "color: var(--sla-accent); font-weight: 500; font-size: 13px; padding: 11px 0;",
                {format!("Show {hidden} more")}
            }
        };
        container.append_child(&more);
    }

    container
}

/// One group's mounted song rows, wrapped in the container whose `height`
/// card J1's collapse animation transitions.
///
/// A plain function rather than a `#[component]`, for the same reason
/// [`group_header`] is (see its doc comment, and [`insert_group_header`]
/// above): the `for` loop in [`Library`] needs the real `NodeHandle` back so
/// it can read `scroll_height()` off it when the group is toggled, and a
/// `#[component]` call is spliced away by the macro's own codegen with no
/// handle ever handed back to the caller.
#[allow(clippy::too_many_arguments)]
fn group_rows(
    scope: &mut RenderScope,
    view: LibraryViewStore,
    songs: SongsStore,
    attachments: AttachmentsStore,
    nav: NavStore,
    index: usize,
    style: String,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            style: {style.clone()},
            for song in songs_in_group(view, songs, attachments, index) {
                let id = song.id;
                let kind = primary_kind(attachments, &song);
                let menu_open = Signal::new(false);
                // Right-click stands in for long-press — see the note in
                // `crate::menu`.
                div {
                    key: id,
                    oncontextmenu: move || menu_open.set(true),
                    DropdownMenu {
                        opened_fn: move || menu_open.get(),
                        on_close: move || menu_open.set(false),
                        position: "bottom-start",
                        style: {FULL_WIDTH_TARGET},
                        DropdownMenuTarget {
                            style: {FULL_WIDTH_TARGET},
                            SongRow {
                                song: {song.clone()},
                                kind: {kind},
                                compact: {view.density.get().is_compact()},
                                onclick: move || nav.go(Route::SongDetail(id)),
                            }
                        }
                        DropdownMenuDropdown {
                            style: {MENU_SURFACE},
                            SongMenuItems { id: id }
                        }
                    }
                }
            }
        }
    }
}

/// The rows one group shows, recomputed from the stores. Takes only `Copy`
/// arguments so it can be called from inside a reactive closure.
fn songs_in_group(
    view: LibraryViewStore,
    songs: SongsStore,
    _attachments: AttachmentsStore,
    index: usize,
) -> Vec<Song> {
    match view.grouped(songs.songs.get()).into_iter().nth(index) {
        Some(group) => visible_songs(&group.songs, view.is_expanded(&group.label)),
        None => Vec::new(),
    }
}

/// Expand the group at `index` past its preview limit.
fn expand_group(view: LibraryViewStore, songs: SongsStore, index: usize) {
    if let Some(group) = view.grouped(songs.songs.get()).into_iter().nth(index) {
        view.expand(group.label);
    }
}

/// The badge a row's thumb shows, or `None` for the dashed empty thumb.
///
/// `pub(super)` since card G1, because the search screen (`1p`) draws the same
/// row and has to reach the same answer. One copy of the rule rather than two —
/// a second implementation that read `primary_attachment` directly would give a
/// different badge for a song whose primary chart names nothing, and the two
/// screens would disagree about a library they are both looking at.
pub(super) fn primary_kind(attachments: AttachmentsStore, song: &Song) -> Option<AttachmentKind> {
    // `Song::primary` is the same rule the card on song detail reads, including
    // its fallback for a `primary_attachment` that names nothing — so a row's
    // badge and the card it opens can never disagree about which chart is the
    // song's. Note it reads the *kind* and nothing else: the body stays on
    // disk, which is what lets three hundred of these rows be built.
    song.primary()
        .and_then(|id| attachments.get(id))
        .map(|a| a.kind)
}

#[cfg(test)]
mod tests {
    /// Card H4: the `≣` chip is a shortcut to the same control Settings'
    /// "Library density" row already writes through, not a second source of
    /// truth — so both call sites have to reach `toggle_density` and neither
    /// is allowed to grow its own `density.set(...)`, which would let the chip
    /// and the row disagree about what density the library is actually in.
    /// Read as source text rather than driven through a store, because there
    /// is no window in a `cargo test` run to click either control in — this is
    /// the cheap half that can be checked without one, the same shape as
    /// `settings::tests::the_footer_is_the_promise_the_handoff_authored`
    /// checking prose rather than pixels.
    #[test]
    fn the_density_chip_and_the_settings_row_write_through_the_same_call() {
        let library = include_str!("library.rs");
        let settings = include_str!("settings.rs");
        assert!(
            library.contains("view.toggle_density()"),
            "the library's ≣ chip no longer calls toggle_density — it may have grown its own signal write"
        );
        assert!(
            settings.contains("view.toggle_density()"),
            "the settings row no longer calls toggle_density — it may have grown its own signal write"
        );
        // Spelled in two halves so that this file, which is one of the files
        // being searched, is not itself a hit — the same trick
        // `tests::the_http_client_is_named_in_exactly_one_file` uses in
        // `lib.rs`, for the same reason.
        let direct_write = concat!("view.density", ".set(");
        assert!(
            !library.contains(direct_write) && !settings.contains(direct_write),
            "a call site is writing `density` directly instead of going through the persisting `toggle_density`"
        );
    }
}

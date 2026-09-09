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
    AttachmentsStore, Group, GroupBy, LibraryViewStore, NavStore, Route, SettingsStore, SongsStore,
};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_SCREEN_TITLE};
use crate::ui::{Chip, IconButton, SongRow, group_header, icon};

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

    // Card J2. `view.grouped(...)` filters the whole book, buckets it and
    // sorts every bucket — and this screen used to ask for that answer from
    // five separate places: the group list's own `for`, the "nothing
    // matches" check below it, the alphabet rail, and once more inside
    // `songs_in_group` *and* `expand_group`, each called once per mounted
    // group rather than once per screen. None of those call sites was
    // wrong to ask; none of them knew another one had just asked the same
    // question. Measured at the 300-song target this card names, First
    // letter grouping with nothing collapsed — the default state, since
    // `LibraryViewStore::is_collapsed` starts every group open — made 28
    // full filter-bucket-sort passes over the book for one frame nobody
    // had scrolled or typed into; see
    // `derive::tests::librarys_repeated_grouped_calls_cost_as_many_full_passes_as_it_makes`
    // for the real numbers.
    //
    // That is well inside a 16ms frame budget even multiplied out (the
    // measured total was low single-digit milliseconds), so this was never
    // a stutter anyone could see — but it is 15-16x more filtering,
    // bucketing and sorting than the screen needs to do, for free, using a
    // primitive already sitting in `rinch_core::reactive` unused by the
    // rest of this app. A `Memo` recomputes only when one of its own
    // reads — here, `songs`, `group_by`, `sort_field`, `sort_dir` or
    // `filters` — actually changes, and caches the answer for every other
    // reader in between. Every place below that used to call
    // `view.grouped(songs.songs.get())` reads `grouped_songs.get()`
    // instead, and gets the same list back at a fraction of the cost.
    //
    // This is the "cheaper fix" half of card J2's own question, chosen
    // over virtualising the list. See that card's other half, on
    // `rendered_row_count` and the timing tests beside it in `derive.rs`,
    // for why virtualising was measured and declined: the row counts a
    // 300-song book actually renders (`GROUP_PREVIEW` truncates every
    // group to 6 until asked for more) never got past the low hundreds,
    // and a virtual list would have silently broken card H4's alphabet
    // scrubber, which needs a real `NodeHandle` for every group header
    // including ones a virtual window would leave unmounted. (Card J1 also
    // built a collapse animation here that measured a group's full height
    // in pixels — card K51's own device pass took that back out; see
    // [`group_entry`]'s doc comment for why. That reason for declining
    // virtualisation is gone with it, but the scrubber's is not.)
    let grouped_songs: Memo<Vec<Group>> = Memo::new(move || view.grouped(songs.songs.get()));

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
                for (index, group) in grouped_songs.get().into_iter().enumerate() {
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
                        grouped_songs,
                        attachments,
                        nav,
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
                if view.filters.get().is_active() && grouped_songs.get().is_empty() {
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
                    present_letters(&grouped_songs.get()),
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
                        box-shadow: 0 8px 18px -4px var(--sla-accent-shadow);",
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
    collapsed_fn: impl Fn() -> bool + 'static,
    compact: bool,
    onclick: impl Fn() + 'static,
) -> NodeHandle {
    let header = group_header(scope, label.clone(), count, is_first, collapsed_fn, compact, onclick);
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

/// One group: its header, its rows (mounted only while the group is open),
/// and its "Show N more" row (if any) — a plain function so [`Library`]'s own
/// `for` loop stays a single expression per header rather than a `let` plus a
/// side-effecting statement, which the rsx macro's children grammar does not
/// have a clean shape for (a bare `if` in that position parses as *reactive
/// conditional content*, not a plain Rust statement — see this file's own
/// card-H4 writeup for the reachability question that sidesteps).
///
/// **Card J1 built a collapse animation here, and card K51's own device pass
/// took it back out.** J1 wrapped a group's rows in a container whose
/// `height` transitioned between two measured pixel values (`crate::ui::
/// group_rows_style`, now deleted) instead of mounting and unmounting them
/// outright, because `rinch`'s transition engine only interpolates `height`
/// between two concrete pixel lengths and snaps on `auto` — see that
/// function's own former doc comment, preserved in this file's git history,
/// for the framework read that motivated it. That worked for *closing* a
/// group. It did not work for reopening one, and nobody could see that from
/// the diff: verifying K51's actual fix — that a tap on a header repaints at
/// all — on the device turned up a second bug the animation had been hiding
/// underneath the first the whole time. Rinch's transition engine
/// (`rinch-dom/src/transition/*`) updates a node's `computed_style` for
/// paint but never pushes the new value into Taffy and never marks the
/// node's layout dirty. So a group animated shut kept a real, correct, 0px
/// *layout* box forever after — reopening it changed the `style` string the
/// paint step reads, and changed nothing Taffy would ever lay out again.
/// The header would say "12 songs," correctly, sitting above rows that were
/// legitimately still there in the DOM and permanently zero pixels tall.
/// That is not a bug in how this screen used the engine; it is a hole in
/// the engine itself, on the layout side rather than the paint side of the
/// two `../rinch-fixes` (`8526ce6`) already fixed there — see
/// [`crate::ui`]'s remaining comment on this, in the spot `group_rows_style`
/// used to be, for that other half.
///
/// So collapse is instant again, deliberately: mounting and unmounting a
/// group's rows outright, the way this screen did before J1, rather than
/// animating a height the framework cannot promise to bring back. Card J1's
/// own closing line — "Nothing else animates — this app is read, not
/// watched" — turns out to have been the answer to reopening a group all
/// along, not just an aesthetic preference: the two sheets this app does
/// animate (`crate::ui::sheet_panel_style`, `sheet_scrim_style`) never
/// unmount the node they are sliding, which is exactly the property this
/// list cannot have and still be a list that scrolls.
///
/// `collapsed` is still a plain `bool` here, read once by [`Library`]'s
/// `for` loop before this function is even called, and that is fine for
/// what it decides: the "Show N more" row's visibility, a decision already
/// rebuilt from scratch on every render this group participates in. What
/// must **not** go back to being a frozen read is whether the rows below
/// are mounted *at all* — that is card K51's fix, and it lives in the `if`
/// inside this function's own `rsx!` below, not in this parameter: the
/// condition there calls `view.is_collapsed(&label)` itself, so the
/// `move || { … }` the macro wraps every `if` condition in
/// (`generate_if_block`) closes over a live signal read rather than a
/// value copied out of one before this function returned — the same shape
/// [`group_header`]'s `collapsed_fn` uses for the badge beside it.
#[allow(clippy::too_many_arguments)]
fn group_entry(
    scope: &mut RenderScope,
    view: LibraryViewStore,
    grouped: Memo<Vec<Group>>,
    attachments: AttachmentsStore,
    nav: NavStore,
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

    let onclick = {
        let label = label.clone();
        move || view.toggle_collapsed(label.clone())
    };

    let header = insert_group_header(
        __scope,
        scrub_targets,
        view.group_by.get(),
        label.clone(),
        total,
        is_first,
        {
            let label = label.clone();
            move || view.is_collapsed(&label)
        },
        view.density.get().is_compact(),
        onclick,
    );

    rsx! {
        div { key: {label.clone()},
            {header}
            if !view.is_collapsed(&label) {
                {group_rows(__scope, view, grouped, attachments, nav, index)}
            }

            // Per-group truncation, until the user asks for the rest.
            if !collapsed && total > GROUP_PREVIEW && !expanded {
                div {
                    onclick: move || expand_group(view, grouped, index),
                    style: "color: var(--sla-accent); font-weight: 500; font-size: 13px; padding: 11px 0;",
                    {format!("Show {hidden} more")}
                }
            }
        }
    }
}

/// One group's mounted song rows — a plain function for the same reason
/// [`group_header`] is (see its doc comment, and [`insert_group_header`]
/// above): [`group_entry`] is itself a plain function rather than a
/// `#[component]`, so a `rsx!` call nested inside it still needs an
/// ordinary Rust expression to splice into the `if` block that decides
/// whether these rows exist at all — a `#[component]` call is spliced away
/// by the macro's own codegen with no handle ever handed back to the
/// caller, and [`group_entry`] does not need one back from this: unlike
/// card J1's version, nothing here reads `scroll_height()` off it any more.
#[allow(clippy::too_many_arguments)]
fn group_rows(
    scope: &mut RenderScope,
    view: LibraryViewStore,
    grouped: Memo<Vec<Group>>,
    attachments: AttachmentsStore,
    nav: NavStore,
    index: usize,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            for song in songs_in_group(grouped, view, index) {
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

/// The rows one group shows. Takes only `Copy` arguments — `grouped` among
/// them, since card J2 made it one — so it can be called from inside a
/// reactive closure.
///
/// Before card J2 this re-derived the whole book's groups itself
/// (`view.grouped(songs.songs.get())`), once per call, which is once per
/// mounted group per render — see `grouped_songs`'s own comment in
/// [`Library`] for what that cost and why a `Memo` is the fix. `index` is
/// still how this finds *its* group rather than being handed the group
/// directly: the memo's cached `Vec<Group>` is the one shared answer every
/// caller reads, and re-deriving nothing past `.get()` and `.nth(index)` is
/// what makes that answer cheap to ask for repeatedly.
fn songs_in_group(grouped: Memo<Vec<Group>>, view: LibraryViewStore, index: usize) -> Vec<Song> {
    match grouped.get().into_iter().nth(index) {
        Some(group) => visible_songs(&group.songs, view.is_expanded(&group.label)),
        None => Vec::new(),
    }
}

/// Expand the group at `index` past its preview limit. See
/// [`songs_in_group`] for why this reads the shared `grouped` memo rather
/// than recomputing the book's groups itself.
fn expand_group(view: LibraryViewStore, grouped: Memo<Vec<Group>>, index: usize) {
    if let Some(group) = grouped.get().into_iter().nth(index) {
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

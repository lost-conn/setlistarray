//! Search & filter — WIREFRAME (`1p`), card G1, styled with the hi-fi token
//! set.
//!
//! One field, and everything in the book that matches it, live as you type,
//! under a heading per kind of thing it found. The library's search field is
//! the way in and does nothing else any more.
//!
//! ## What `1p` draws that is not here
//!
//! The wireframe is a picture of the finished screen, and two thirds of what it
//! draws belong to cards that do not exist yet. Both are absent on the record
//! rather than by oversight, and the second one is a decision this card was
//! asked to make and defend.
//!
//! **The facet chips** — `Key ▾`, `Tuning ▾`, `Capo ▾`, `Tag ▾`, `Tempo ▾` and
//! the two active pills — are card **G3**. They are a filter engine with a
//! popover per facet and an active-state model of their own; drawing the row
//! without it would be seven controls that open nothing.
//!
//! **The "Inside attachments" group** — matching text inside typed charts and
//! captured pages — is card **G2**, and the question this card had to answer is
//! whether to draw its *heading* now, over nothing, and let G2 fill it in.
//!
//! **It is not drawn.** A heading with nothing under it is the dead-row problem
//! H1 spent its own paragraphs avoiding one screen over, and here it is worse
//! than a row that does nothing, because of what the empty shell would be
//! *saying*. "Inside attachments" followed by nothing is a sentence: we looked
//! inside your charts and your saved pages, and the word is not in any of them.
//! That is a false negative about the user's own library — the one lie a search
//! screen cannot tell and stay useful, because the whole value of a search is
//! that a blank answer means the thing is not there. A user who trusts it stops
//! opening the chart they would have found it in.
//!
//! The cost of leaving it out is that the group is invisible until G2 lands,
//! and that cost is paid by nobody but the next developer, who has this
//! paragraph. So instead of a heading, the empty states below say in words what
//! search does look at — titles, artists, tags, set names — which is a true
//! statement today, is the honest boundary of what has been searched, and is
//! one line for G2 to extend rather than a promise for it to make good.
//!
//! ## The route carries nothing; the query is library view state
//!
//! [`Route::Search`](crate::store::Route::Search) has no payload and
//! [`LibraryViewStore::query`](crate::store::LibraryViewStore::query) is what
//! the field types into. The reasoning for each half is written where it can be
//! read from — the route's own doc comment for why a `String` does not go on a
//! `Copy` enum that is matched on every frame, and the signal's for why a query
//! the library no longer filters by nonetheless stays in the persisted view
//! state rather than moving to `NavStore` (short version: retiring a
//! `Preferences` field means turning on rhypedb's one-way
//! `allow_schema_shrink` for every library, forever, and this card is not
//! entitled to spend that).
//!
//! One consequence worth stating out loud, because it is visible: the query
//! survives leaving the screen. Come back to search and the last thing you
//! looked for is still in the field, with the ✕ beside it. That is the right
//! way round for this app's navigation, which is shallow — ← from a song
//! detail lands on the *library*, not back on the search that found it (see
//! [`NavStore::back`](crate::store::NavStore::back)) — so a query that cleared
//! itself on the way out would mean retyping it to open the second result.
//!
//! ## What tapping the library's field used to do
//!
//! It filtered the library list in place, through `LibraryViewStore::grouped`
//! and `derive::grouped`. That is gone: `derive::grouped` no longer takes a
//! query at all, and its header carries the argument for why a library left
//! filtered behind a field that no longer types is the worse of the two
//! screens. The field on the library is now a tap target with a placeholder in
//! it and no `oninput`.
//!
//! ## Where the fiddly part lives
//!
//! In [`crate::derive`], per card X1, and it is most of the card:
//! [`highlight`](crate::derive::highlight) is the substring segmentation —
//! case-insensitive against a case-preserving display string, over a byte map
//! rather than a lowercased copy, because `str::to_lowercase` is not
//! length-preserving and slicing the original at a shifted offset is a panic on
//! a multi-byte character rather than a wrong answer;
//! [`search_songs`](crate::derive::search_songs) and
//! [`search_setlists`](crate::derive::search_setlists) are the matcher and the
//! order; [`search_count_line`](crate::derive::search_count_line) is the line
//! under the field, which counts each group against its own total; and
//! [`search_rows`](crate::derive::search_rows) is the whole scrolling area —
//! headings, results and both empty states — as one list of keyed values.
//!
//! That last one is a layout the phone forced. The area was four sibling `if`
//! blocks first, and the "nothing matched" one did not reliably appear when its
//! condition turned true; [`SearchRow`](crate::derive::SearchRow) records the
//! reproduction. It is a better shape anyway: what is on screen in each of the
//! three states is now a value with tests over it rather than four conditions
//! evaluated in a column.
//!
//! ## Handlers
//!
//! `onclick` and `oninput` only — the two the touch recogniser delivers. See
//! `src/gesture_reachability.rs`; nothing on this screen wants anything else,
//! and the ✕ is a tap rather than a swipe-to-clear for exactly that reason.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{
    Highlight, SearchRow, SetlistHit, SongHit, search_count_line, search_rows, setlist_hits,
    song_hits,
};
use crate::store::{
    AttachmentsStore, LibraryViewStore, NavStore, Route, SetlistsStore, SongsStore,
};
use crate::theme::{SCREEN_PAD, T_LABEL_CAPS, T_META, T_META_SMALL, T_ROW_TITLE};
use crate::ui::{AttachmentThumb, IconButton, icon};

/// The matched run's treatment: accent ink at 600, on the paper the rest of the
/// row is on.
///
/// **`1p` draws a yellow highlighter pen and this is not one, and the reason is
/// a measurement rather than a preference.** The obvious hi-fi translation of
/// that pen is the handoff's own tint pair — `--sla-accent-tint` behind
/// `--sla-accent-on-tint`, "accent at ~12% over paper" with "readable
/// accent-family text on the tint", the pair the selected metadata chip on song
/// detail already wears. It was built that way and put on the phone, and rinch
/// does not paint the background box of an *inline* span where the glyphs are:
/// the tint came out as a pale sliver about a quarter of the line's height,
/// sitting above the letter it was supposed to be behind. Forcing a real box
/// with `display: inline-block` gave the tint the right size and then took the
/// run off the baseline — every marked substring rode a few pixels high, so
/// `Ju`**`ne`**`grass` read as a typographic accident rather than a mark.
///
/// Colour and weight are the two properties that do flow correctly on an inline
/// run here, and this app already marks a fact that way and knows it works: the
/// library's `N solid` is `var(--sla-accent)` at 600 inside a muted line, three
/// screens back. So the mark is the same gesture, which also means a match
/// inside the grey meta line is legible for the same reason that one is. It
/// costs the highlighter's block of colour and keeps the thing the block was
/// for — the eye landing on the matched letters before it reads the row.
///
/// If a later rinch paints inline backgrounds on the baseline, the tint pair
/// above is what to come back to; nothing else here would have to move.
const MATCHED: &str = "color: var(--sla-accent); font-weight: 600;";

/// The row rule every result sits on, songs and setlists alike.
const ROW: &str = "display: flex; align-items: center; gap: 13px; padding: 11px 0; \
                   border-bottom: 1px solid var(--sla-hairline-soft);";

/// One line of a result, clipped rather than wrapped: two rows that each wrap to
/// three lines are a list you cannot scan.
///
/// **`pre` rather than the `nowrap` every other one-line row in this app uses,
/// and the difference is one space.** A highlighted line is several sibling
/// `<span>`s, and a run boundary falls wherever the *match* does — so `Bob
/// Dylan` searched for `dylan` is `"Bob "` followed by `"Dylan"`, with the
/// space at the very end of a text node. Under collapsing white space that
/// space is dropped, and the row read `BobDylan` on the phone: not a
/// typographic nicety, but the difference between an artist's name and a word
/// nobody has.
///
/// It has to be here rather than on the runs themselves, which is what was
/// tried first and did nothing. rinch reads `white-space` off the *root of the
/// inline formatting context* and applies one collapse mode to the whole of it
/// (`rinch-dom/src/ifc.rs`: `root_computed.white_space` decides
/// `WhiteSpaceCollapse`); a `white-space` on an inline child is never consulted.
/// So the line's own box is the only place the switch exists.
///
/// Nothing is given up. The same file treats `Pre` and `NoWrap` identically for
/// wrapping — both set no max width — and its ellipsis path tests for the pair
/// together, so this line still refuses to wrap and still truncates with `…`.
const ONE_LINE: &str = "overflow: hidden; text-overflow: ellipsis; white-space: pre;";

#[component]
pub fn Search() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();
    let setlists = use_store::<SetlistsStore>();
    let view = use_store::<LibraryViewStore>();

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",

            // ← and the field, on one row, exactly as `1p` has them. The back
            // arrow is the same one every secondary screen in this app wears,
            // at the same inset; `NavStore::back` lands on the current tab's
            // root, and the only door into this screen is the library's field,
            // so ← returns where the user came from.
            div {
                style: {format!("padding: 2px {SCREEN_PAD} 10px; display: flex; align-items: center; gap: 8px;")},
                IconButton { glyph: TablerIcon::ChevronLeft, size: 19, onclick: move || nav.back() }

                div {
                    style: "flex: 1; background: var(--sla-fill); border-radius: 14px; \
                            padding: 6px 6px 6px 14px; display: flex; align-items: center; gap: 9px;",
                    span { style: "color: var(--sla-muted); display: flex;", {icon(__scope, TablerIcon::Search, 17)} }
                    input {
                        r#type: "text",
                        style: "flex: 1; border: none; outline: none; background: transparent; \
                                font-family: var(--sla-font-ui); font-size: 15px; color: var(--sla-ink); \
                                padding: 6px 0;",
                        placeholder: "Search title, artist, tag…",
                        // Reactive, not a one-shot: it is what makes the ✕
                        // below actually empty the visible field rather than
                        // only the signal behind it.
                        value: {|| view.query.get()},
                        oninput: move |value: String| view.set_query(value),
                    }
                    // The way out of a query without leaving the screen. Only
                    // there when there is something to clear: a ✕ that clears
                    // nothing is furniture, and it sits where a real control
                    // would, so it teaches the wrong thing about the one beside
                    // it. 36 square, which is the touch target rather than the
                    // glyph — the same reason `IconButton` is a 40 box around a
                    // 19px chevron.
                    //
                    // Measured on the phone: tapping it empties the field and
                    // takes the caret with it, so the keyboard stays up over a
                    // field that is no longer taking keys until the field is
                    // tapped again. That is not fixable from here — rinch has
                    // no API that gives a node focus (there is no `focus()`
                    // anywhere in `rinch-core` or `rinch-dom`; the only thing
                    // in that area is `set_focus_visible`, which paints a ring
                    // rather than moving the caret), so the alternatives are
                    // this or no clear button at all. Worth an upstream ask the
                    // next time this app needs to move a caret for any other
                    // reason; not worth blocking a search screen on.
                    if !view.query.get().is_empty() {
                        div {
                            onclick: move || view.set_query(String::new()),
                            style: "width: 36px; height: 36px; flex-shrink: 0; color: var(--sla-muted); \
                                    display: flex; align-items: center; justify-content: center;",
                            {icon(__scope, TablerIcon::X, 16)}
                        }
                    }
                }
            }

            // The count line. Absent while nothing is typed, because there is
            // nothing for it to be a count *of* — the screen below is its own
            // empty state at that point, and a line reading "0 of 25 songs"
            // over it would be an answer to a question nobody has asked yet.
            if !view.query.get().trim().is_empty() {
                div {
                    style: {format!("{T_META_SMALL} padding: 0 {SCREEN_PAD} 4px;")},
                    {move || count_line(view, songs, setlists)}
                }
            }

            // Everything that scrolls, as one keyed list — headings, results and
            // both empty states. `SearchRow`'s own header carries the reason it
            // is a list and not four sibling `if` blocks, and it is a fault
            // found on the phone rather than a preference.
            div {
                style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD} 90px;")},
                for row in rows(view, songs, setlists) {
                    // The `for` body re-runs as a closure, so everything it
                    // keeps is cloned or copied up front — the same rule the
                    // library list is written to. Each row is a component so
                    // that the bindings a row needs live in a function body
                    // rather than inside a `match` arm.
                    let key = row.key();
                    div { key: {key.clone()},
                        Row { row: {row.clone()} }
                    }
                }
            }
        }
    }
}

/// One row, whichever kind it is.
///
/// The `match` lives in here rather than in the `for` body above, and that is
/// the macro's constraint rather than a preference: the scrutinee of an rsx
/// `match` is captured by a `move` closure, so a `match` on the loop item and a
/// `key:` computed from that same item cannot both have it. A component prop is
/// evaluated eagerly, so handing the row over one level down leaves the key
/// expression the only thing in the body that touches it.
#[component]
fn Row(row: SearchRow) -> NodeHandle {
    // A plain Rust `match` over five `rsx!` blocks rather than a `match` inside
    // one, and the difference is not style. An rsx `match` is reactive: it
    // captures its scrutinee in a `move` closure so it can re-evaluate it, and
    // a non-`Copy` scrutinee is therefore moved into that closure and cannot be
    // read anywhere else — which is exactly what a `match` over an owned prop
    // wants to do. Out here it is a value matched once, which is also the truth
    // about this row: a row never changes which kind of thing it is. The `for`
    // above rebuilds it when its data moves, and rebuilds it as this function
    // running again from the top.
    match row {
        SearchRow::Heading {
            label,
            count,
            first,
        } => rsx! {
            Heading { label: {label.to_string()}, count: {count}, first: {first} }
        },
        SearchRow::Song(hit) => rsx! { SongResult { hit: {hit} } },
        SearchRow::Setlist(hit) => rsx! { SetlistResult { hit: {hit} } },
        SearchRow::Prompt => rsx! { Prompt {} },
        SearchRow::NoMatch(query) => rsx! { NoMatch { query: {query} } },
    }
}

/// A group heading: label, count, then a hairline filling the rest of the row.
///
/// A hand-rolled twin of [`crate::ui::group_header`], which is the shape it
/// copies down to the padding. The library's version takes a `collapsed` flag
/// and an `onclick` that collapses the group, and — since card H4 — an
/// `impl Fn()` `onclick` closure only that function's own `for` loop knows how
/// to build, keyed to whichever letter's header it is. Neither fits here — a
/// search group has nothing to collapse, and no per-row identity for a
/// scrubber to jump to — so this screen keeps its own twelve lines rather than
/// growing a second shape into a component two other screens depend on.
#[component]
fn Heading(label: String, count: usize, first: bool) -> NodeHandle {
    let colour = if first {
        "var(--sla-accent)"
    } else {
        "var(--sla-muted)"
    };

    rsx! {
        div {
            style: "display: flex; align-items: center; gap: 8px; padding: 16px 0 8px;",
            span { style: {format!("{T_LABEL_CAPS} color: {colour};")}, {label.clone()} }
            span { style: {format!("{T_META_SMALL}")}, {count.to_string()} }
            div { style: "flex: 1; height: 1px; background: var(--sla-hairline);" }
        }
    }
}

/// One song result: the library's own row, with the query marked in both lines.
#[component]
fn SongResult(hit: SongHit) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let attachments = use_store::<AttachmentsStore>();

    let id = hit.song.id;
    // The same rule the library row reads, so a song's badge cannot differ
    // between the two screens — see `library::primary_kind`.
    let kind = super::library::primary_kind(attachments, &hit.song);

    rsx! {
        div {
            onclick: move || nav.go(Route::SongDetail(id)),
            style: {ROW},
            AttachmentThumb { kind: kind }
            div { style: "flex: 1; min-width: 0;",
                div {
                    style: {format!("{T_ROW_TITLE} {ONE_LINE}")},
                    Highlighted { runs: {hit.title.clone()} }
                }
                div {
                    style: {format!("{T_META} margin-top: 2px; {ONE_LINE}")},
                    Highlighted { runs: {hit.meta.clone()} }
                }
            }
        }
    }
}

/// One setlist result: the set's name marked, and the `9 songs · 32:04` under
/// it that the add-to-setlist sheet prints too.
#[component]
fn SetlistResult(hit: SetlistHit) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let id = hit.setlist.id;

    rsx! {
        div {
            onclick: move || nav.go(Route::SetlistDetail(id)),
            style: {ROW},
            // `1p` draws the set's thumb as an empty square. The nav's own
            // setlist glyph instead: an empty box beside a row of songs each
            // carrying TXT, PDF or WEB reads as a chart that failed to load.
            div {
                style: "width: 38px; height: 38px; border-radius: 10px; \
                        background: var(--sla-fill); color: var(--sla-muted); flex-shrink: 0; \
                        display: flex; align-items: center; justify-content: center;",
                {icon(__scope, TablerIcon::List, 18)}
            }
            div { style: "flex: 1; min-width: 0;",
                div {
                    style: {format!("{T_ROW_TITLE} {ONE_LINE}")},
                    Highlighted { runs: {hit.name.clone()} }
                }
                div { style: {format!("{T_META} margin-top: 2px;")}, {hit.summary.clone()} }
            }
        }
    }
}

/// Nothing typed yet. Not a blank screen: this is where the screen says what it
/// can be asked, which is also the honest edge of what it searches until card
/// G2 widens it.
#[component]
fn Prompt() -> NodeHandle {
    rsx! {
        div { style: "padding: 60px 8px; text-align: center;",
            div { style: "color: var(--sla-muted); display: flex; justify-content: center;",
                {icon(__scope, TablerIcon::Search, 26)}
            }
            div { style: {format!("{T_ROW_TITLE} margin-top: 14px;")}, "Find something to play" }
            div { style: {format!("{T_META} margin-top: 6px;")},
                "Type a title, an artist, a tag, or the name of a set."
            }
        }
    }
}

/// Typed, and nothing in the book answers to it.
///
/// It names the query back — a search that says only "no results" leaves you
/// wondering whether it heard you — and then says what it looked at, which is
/// the same sentence [`Prompt`] opens with and the same one card G2 gets to
/// extend. Saying it here is the other half of leaving the "Inside attachments"
/// heading out: this screen must not let a blank answer be read as "and not in
/// your charts either".
#[component]
fn NoMatch(query: String) -> NodeHandle {
    rsx! {
        div { style: "padding: 60px 8px; text-align: center;",
            div { style: {format!("{T_ROW_TITLE}")}, {format!("Nothing matches “{query}”")} }
            div { style: {format!("{T_META} margin-top: 6px;")},
                "Search looks at song titles, artists and tags, and at set names."
            }
        }
    }
}

/// One display string, drawn with its matched runs marked.
///
/// It takes the runs rather than the text and the query, and never reads a
/// signal: the segmentation is already part of the row's data, so this is a
/// static draw of a value the `for` above has decided is current. See
/// [`SongHit`](crate::derive::SongHit) for why it has to be that way round —
/// the short version is that a row whose key survives a keystroke keeps its
/// DOM, so a highlight computed in here would freeze on the query it was first
/// built with.
#[component]
fn Highlighted(runs: Vec<Highlight>) -> NodeHandle {
    rsx! {
        span {
            for (index, run) in runs.clone().into_iter().enumerate() {
                span {
                    key: {index},
                    style: {if run.matched { MATCHED } else { "" }},
                    {run.text.clone()}
                }
            }
        }
    }
}

/// Everything the screen scrolls. Takes only `Copy` arguments so it can be
/// called from inside a reactive closure — the same shape as
/// `library::songs_in_group`.
fn rows(view: LibraryViewStore, songs: SongsStore, setlists: SetlistsStore) -> Vec<SearchRow> {
    search_rows(
        songs.songs.get(),
        setlists.setlists.get(),
        &view.query.get(),
    )
}

/// The line under the field: how much of the book is on screen.
///
/// It asks the matchers again rather than counting [`rows`], because the
/// numbers it prints are not the same numbers: the line states each group's
/// total whether or not that group has a heading on screen, and a book with no
/// matching setlists has no Setlists row to count.
fn count_line(view: LibraryViewStore, songs: SongsStore, setlists: SetlistsStore) -> String {
    let query = view.query.get();
    let library = songs.songs.get();
    let sets = setlists.setlists.get();
    search_count_line(
        song_hits(library.clone(), &query).len(),
        library.len(),
        setlist_hits(sets.clone(), &library, &query).len(),
        sets.len(),
    )
}

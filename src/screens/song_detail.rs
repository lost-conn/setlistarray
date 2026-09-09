//! Song detail — HI-FI.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::menu::{AttachmentMenuItems, FULL_WIDTH_TARGET, MENU_SURFACE, SongMenuItems};
use crate::model::{
    Attachment, AttachmentId, AttachmentKind, Confidence, Song, SongId, fmt_bytes, fmt_duration,
};
use crate::picker::Picked;
use crate::store::{AttachmentsStore, NavStore, Route, SetlistsStore, SongsStore, SystemStore};
use crate::theme::{SCREEN_PAD, T_BODY, T_CHART, T_DETAIL_TITLE, T_META, T_META_SMALL};
use crate::ui::{AttachmentThumb, ConfidenceDots, IconButton, MetaChip, icon};

use super::captured_page::CapturedPageView;

/// One row of the add-attachment chooser: badge, what it does, chevron.
///
/// `1j` draws these as bordered boxes; here they wear the card fill the rest of
/// this screen uses, because on song detail they sit under a card and a second
/// outlined shape would read as a competing one.
const PRODUCER_ROW: &str = "display: flex; align-items: center; gap: 11px; \
    padding: 10px 12px; border-radius: 12px; background: var(--sla-card); \
    box-shadow: var(--sla-card-shadow);";

#[component]
pub fn SongDetail(id: Option<SongId>) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();
    let setlists = use_store::<SetlistsStore>();
    let attachments = use_store::<AttachmentsStore>();
    let system = use_store::<SystemStore>();
    // How wide the window is, as a closure rather than a number, and card K53
    // is the whole reason for the parentheses. Every page width on this screen
    // is derived from this (see `card_page_width`), and until K53 it came from
    // `platform::viewport_width()`, which cached its answer for the life of the
    // process — so a window that changed size under a running app kept drawing
    // its pages at the width the app started at, forever. Read from the signal,
    // inside the render closures below, it is a subscription instead: the width
    // changes, the closures re-run, the pages are re-fitted.
    let viewport = move || system.viewport_width.get();
    let menu_open = Signal::new(false);
    // Whether the add-attachment chooser is showing its rows.
    let adding = Signal::new(false);
    // What the last import attempt had to say, if it failed. Written from
    // inside the picker's callback, which on Android runs long after the tap
    // and possibly after this screen is gone — a write to a signal whose scope
    // has been disposed is a warn-once no-op rather than a panic, which is what
    // makes that safe (see `crate::picker`).
    let trouble = Signal::new(Option::<String>::None);
    // Which collapsed rows are open. Purely view state: expanding a row inlines
    // its content and nothing else — it does not promote it, and it is not
    // remembered past this screen. The mutation seam is `SongsStore::attach` /
    // `detach` / `set_primary`, and none of it can be reached from here.
    let expanded = Signal::new(Vec::<AttachmentId>::new());

    let id = id.unwrap_or_default();

    // The phone's Back key does what this screen's ← does, and finds out by
    // being told rather than by a `match` on the route kept somewhere else.
    // Mount-time work, so it belongs in the body next to the D4 call below and
    // not in a render closure — the same line card K54 drew, and the note
    // further down says the rest of it.
    //
    // Above the gone-song guard on purpose: that guard returns a screen with no
    // ← on it at all, and a Back that did nothing on the one frame between a
    // delete committing and J7's effect firing would be a small dead end for no
    // reason.
    nav.register_back(move || nav.back());

    // A net, not the only answer, since card J7: `crate::app` carries an
    // effect that leaves this route the moment `songs.get(id)` starts coming
    // back `None`, so in the ordinary case nobody sees this sentence at all —
    // deleting from the ⋮ menu right here lands you back on the library
    // before this line would ever run again. What it still catches is the one
    // frame between the delete committing and that effect firing, and a route
    // arrived at some other way with a song already gone (a stale deep link,
    // one day; nothing today constructs one).
    let Some(song) = songs.get(id) else {
        return rsx! {
            div { style: {format!("padding: {SCREEN_PAD}; {T_META}")}, "This song is gone." }
        };
    };

    // D4: draw the primary chart's first page if it is not already drawn.
    //
    // Here, in the component body, which runs once when the screen mounts —
    // never from a render closure, which runs on every redraw. This is the
    // self-healing half of the cache and it exists for two states that
    // `pdf::import` cannot fix on its own: a chart imported before D4 landed,
    // which has a `chart.pdf` and no pages at all, and one whose import-time
    // render failed or was killed halfway. Both look identical from here —
    // `chart.pdf` present, `page-1.png` absent — and both want the same thing.
    //
    // Synchronous, and on the thread that draws. That is the deliberate cheap
    // option: one page is a frame's work, it only happens at all when the cache
    // is cold, and the alternative is `set_timeout`, which parks a callback on
    // the main thread through machinery this app has never used and docs/PDF.md
    // lists as an open risk. A once-per-mount stall of a frame is a smaller
    // thing to accept than a first use of untried framework plumbing.
    if let Some(primary) = song.primary()
        && attachments.get(primary).map(|a| a.kind) == Some(AttachmentKind::Pdf)
        && let Some(directory) = attachments.directory(primary)
    {
        crate::pdf::pages::ensure_page(&directory, 1);
    }

    // Nothing the header draws is computed here, and that is card K54.
    //
    // It used to be: `chips`, `set_count` and `status_tail` were bound in this
    // body next to the D4 call above, and the title, the artist and the
    // confidence word were read straight off the `song` the guard destructured.
    // The body runs once, when the screen mounts, so all six were a photograph
    // of the library taken at the moment the route changed. On the device that
    // showed up as `Set confidence › Rusty` in the ⋮ menu running its handler,
    // writing a row that survived a relaunch, and leaving the header saying
    // `Solid` until the screen was left and come back to. The write was never
    // lost; there was simply no reader watching for it. `src/store/songs.rs`
    // carries two tests that pass on the broken build and say so.
    //
    // So every displayed field below is read inside a `{move || …}` closure or
    // an `rsx!` `for`, which is the same thing the primary-attachment block
    // further down has always done ("derived on read rather than at mount") and
    // what the whole of `setlist_detail` does, for the same reason — the song
    // picker slides over that screen and edits the set it is showing.
    //
    // What stays in the body is the work that must happen once and only once:
    // the gone-song net, and the D4 `ensure_page` call above it, which is
    // commented at length about why it is not allowed to run per redraw. That
    // is the whole distinction — mount-time *work* here, displayed *values* in
    // a closure — and the reason this could not be a mechanical move of the
    // block.

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",

            div { style: "padding: 2px 18px 8px; display: flex; align-items: center; gap: 6px;",
                IconButton { glyph: TablerIcon::ChevronLeft, size: 19, onclick: move || nav.back() }
                div { style: "flex: 1;" }
                IconButton {
                    glyph: TablerIcon::Pencil,
                    size: 17,
                    onclick: move || nav.go(Route::EditSong(id)),
                }
                DropdownMenu {
                    opened_fn: move || menu_open.get(),
                    on_close: move || menu_open.set(false),
                    position: "bottom-end",
                    DropdownMenuTarget {
                        IconButton {
                            glyph: TablerIcon::DotsVertical,
                            size: 17,
                            onclick: move || menu_open.update(|v| *v = !*v),
                        }
                    }
                    DropdownMenuDropdown {
                        style: {MENU_SURFACE},
                        SongMenuItems { id: id }
                    }
                }
            }

            div { style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD};")},

                div { style: {format!("{T_DETAIL_TITLE}")}, {move || title_of(songs, id)} }
                div { style: {format!("{T_BODY} color: var(--sla-muted); margin-top: 5px;")}, {move || artist_of(songs, id)} }

                // Filled fields only — never an empty slot or a placeholder dash.
                div { style: "display: flex; flex-wrap: wrap; gap: 7px; margin-top: 13px;",
                    for chip in chips_of(songs, id) {
                        MetaChip { key: {chip.0.clone()}, label: {chip.0.clone()}, is_key: {chip.1} }
                    }
                }

                // Status line: confidence, dots, then the facts that follow from it.
                div {
                    style: "display: flex; align-items: center; gap: 8px; margin-top: 16px; \
                            padding-top: 13px; border-top: 1px solid var(--sla-hairline);",
                    span {
                        style: "font-weight: 600; font-size: 13px; color: var(--sla-ink-2);",
                        {move || confidence_of(songs, id).map(|c| c.label()).unwrap_or("Unrated")}
                    }
                    ConfidenceDots { confidence: {move || confidence_of(songs, id)} }
                    span { style: {format!("{T_META}")}, {move || status_tail_of(songs, setlists, id)} }
                }

                // Primary attachment card, and the collapsed rows under it.
                //
                // Both are derived on read rather than at mount: attaching a
                // chart, removing one or promoting another has to redraw this
                // block, and a value computed once when the screen opened
                // cannot. `for` over a nought-or-one vector stands in for the
                // `if let` the macro has no reactive form of, and every helper
                // below takes only `Copy` arguments so a nested closure can
                // call it.
                for att in primary_of(songs, attachments, id) {
                    let menu_open = Signal::new(false);
                    div {
                        key: {att.id},
                        oncontextmenu: move || menu_open.set(true),
                        DropdownMenu {
                            opened_fn: move || menu_open.get(),
                            on_close: move || menu_open.set(false),
                            position: "bottom-start",
                            style: {FULL_WIDTH_TARGET},
                            DropdownMenuTarget {
                                style: {FULL_WIDTH_TARGET},
                                div {
                                    // "Tap primary attachment card → Attachment
                                    // viewer", which is the handoff's own
                                    // interaction table and what the card's
                                    // own footer line has been promising since
                                    // this screen was built. D5 is the screen
                                    // that finally exists to be opened.
                                    //
                                    // On the whole card rather than on the ⤢
                                    // glyph: the footer says "Tap to open full
                                    // screen" without qualifying where, and a
                                    // 17px icon is not the target a phone wants
                                    // for the main action on the screen. The
                                    // long-press menu still comes off the same
                                    // box — `oncontextmenu` is dispatched
                                    // separately from `onclick` and does not
                                    // fire it (see `menu`'s header).
                                    onclick: move || nav.go(Route::ViewAttachment {
                                        song: id,
                                        attachment: att.id,
                                    }),
                                    style: "background: var(--sla-card); border-radius: 16px; \
                                            padding: 16px 18px; margin-top: 14px; \
                                            box-shadow: var(--sla-card-shadow);",
                                    div { style: "display: flex; align-items: center; gap: 8px;",
                                        div { style: "flex: 1; min-width: 0;",
                                            div { style: "font-weight: 600; font-size: 15px;", {att.title.clone()} }
                                            div { style: {format!("{T_META_SMALL} margin-top: 2px;")},
                                                {primary_subtitle(&att)}
                                            }
                                        }
                                        span { style: "color: var(--sla-accent); display: flex;",
                                            {icon(__scope, TablerIcon::Maximize, 18)}
                                        }
                                    }

                                    // The chart itself, as far as it exists.
                                    // Nothing here is a skeleton: a kind with
                                    // no renderable content says so.
                                    div { style: "margin-top: 14px; display: flex; flex-direction: column;",
                                        // D4: a PDF's first page, rasterised.
                                        // `width: 100%` and nothing else —
                                        // Rinch's layout takes the height from
                                        // the decoded image's aspect ratio when
                                        // only one axis is constrained, so a
                                        // portrait page and a landscape one
                                        // both come out the right shape without
                                        // this screen knowing which it has. The
                                        // radius matches the card's own 16 px
                                        // less its 18 px of padding, so the
                                        // page's corners sit concentric inside
                                        // the card's rather than proud of them.
                                        for (src, width, height) in page_of_primary(songs, attachments, id, viewport()) {
                                            img {
                                                key: {src.clone()},
                                                src: {src.clone()},
                                                style: {format!("width: {width}px; height: {height}px; border-radius: 6px;")},
                                            }
                                        }
                                        // E5: a captured page, drawn as the
                                        // page rather than as its text. The
                                        // box is exactly as tall as the eight
                                        // lines of `T_CHART` it replaces —
                                        // `PREVIEW_LINES` at 12.5 px on a 1.5
                                        // line — and clips, because a card is a
                                        // look at the top of a chart and the
                                        // whole of it is one tap away. The same
                                        // component full-screen is what the tap
                                        // opens.
                                        for att_id in capture_of_primary(songs, attachments, id) {
                                            CapturedPageView {
                                                key: {att_id},
                                                attachment: {att_id},
                                                base_px: {CARD_BASE_PX},
                                                column_px: {card_page_width(viewport())},
                                                budget: {crate::capture::render::CARD_ELEMENTS},
                                                style: {format!(
                                                    "max-height: {CARD_PREVIEW_HEIGHT}px; overflow: hidden;"
                                                )},
                                            }
                                        }
                                        for (index, line, note) in preview_of_primary(songs, attachments, id, viewport()) {
                                            div {
                                                key: {index},
                                                style: {if note {
                                                    format!("{T_META_SMALL} margin-top: 4px;")
                                                } else {
                                                    T_CHART.to_string()
                                                }},
                                                {line.clone()}
                                            }
                                        }
                                    }

                                    div { style: {format!("{T_META_SMALL} margin-top: 14px;")},
                                        "Tap to open full screen"
                                    }
                                }
                            }
                            DropdownMenuDropdown {
                                style: {MENU_SURFACE},
                                AttachmentMenuItems { song: {id}, attachment: {att.id} }
                            }
                        }
                    }
                }

                // Other attachments, collapsed. Tapping one inlines it; it does
                // not become primary. Long-press opens the menu that does.
                for (index, label, open) in other_rows(songs, attachments, id, expanded) {
                    let menu_open = Signal::new(false);
                    div {
                        key: {index},
                        oncontextmenu: move || menu_open.set(true),
                        style: "border-bottom: 1px solid var(--sla-hairline-soft);",
                        DropdownMenu {
                            opened_fn: move || menu_open.get(),
                            on_close: move || menu_open.set(false),
                            position: "bottom-start",
                            style: {FULL_WIDTH_TARGET},
                            DropdownMenuTarget {
                                style: {FULL_WIDTH_TARGET},
                                div {
                                    onclick: move || toggle_expanded(songs, attachments, id, index, expanded),
                                    style: "display: flex; align-items: center; gap: 8px; padding: 12px 0;",
                                    span { style: "font-weight: 500; font-size: 15px; flex: 1;",
                                        {label.clone()}
                                    }
                                    span { style: "color: var(--sla-muted); display: flex;",
                                        {icon(__scope, if open { TablerIcon::ChevronUp } else { TablerIcon::ChevronDown }, 17)}
                                    }
                                }
                                for (src, width, height) in page_of_row(songs, attachments, id, index, expanded, viewport()) {
                                    img {
                                        key: {src.clone()},
                                        src: {src.clone()},
                                        style: {format!("width: {width}px; height: {height}px; \
                                                         border-radius: 6px; margin-bottom: 10px;")},
                                    }
                                }
                                for att_id in capture_of_row(songs, attachments, id, index, expanded) {
                                    CapturedPageView {
                                        key: {att_id},
                                        attachment: {att_id},
                                        base_px: {CARD_BASE_PX},
                                        column_px: {row_page_width(viewport())},
                                        budget: {crate::capture::render::CARD_ELEMENTS},
                                        style: {format!(
                                            "max-height: {CARD_PREVIEW_HEIGHT}px; overflow: hidden; \
                                             padding-bottom: 10px;"
                                        )},
                                    }
                                }
                                for (line_index, line, note) in preview_of_row(songs, attachments, id, index, expanded, viewport()) {
                                    div {
                                        key: {line_index},
                                        style: {if note {
                                            format!("{T_META_SMALL} padding-bottom: 4px;")
                                        } else {
                                            T_CHART.to_string()
                                        }},
                                        {line.clone()}
                                    }
                                }
                            }
                            DropdownMenuDropdown {
                                style: {MENU_SURFACE},
                                AttachmentMenuItems {
                                    song: {id},
                                    attachment: {other_id(songs, attachments, id, index).unwrap_or_default()},
                                }
                            }
                        }
                    }
                }

                // The chooser `1j` draws, now that D3 has given it a second
                // thing to choose.
                //
                // Until this card there was one producer, so this was the
                // hi-fi's single accent line with a muted sub-line naming what
                // it did. `1j` draws three boxed rows — `PDF · Pick a PDF`,
                // `WEB · Save a webpage offline`, `TXT · Type lyrics / chords`
                // — and two of them now exist. The third is E2 and is left out
                // rather than drawn inert: a row that looks live and does
                // nothing is worse than the gap, which is the same call
                // `song_form` made about this section and the reason it is
                // still empty there.
                //
                // The accent line stays as the way in, because the hi-fi draws
                // it and the wireframe's boxed rows are what is behind it. It
                // opens them rather than navigating, so a song that already has
                // charts is not one tap further from a second one.
                div {
                    style: "padding: 14px 0 22px;",
                    div {
                        onclick: move || {
                            trouble.set(None);
                            adding.update(|open| *open = !*open);
                        },
                        style: "color: var(--sla-accent); font-weight: 600; font-size: 15px; \
                                padding: 4px 0;",
                        "+ Add attachment"
                    }

                    if adding.get() {
                        div { style: "margin-top: 8px; display: flex; flex-direction: column; gap: 8px;",

                            // D3. The picker is asked and the answer comes back
                            // through a callback that may run now (desktop) or
                            // after a trip through another app (Android) — see
                            // `crate::picker`. Nothing here assumes the screen
                            // is still on screen when it does.
                            div {
                                onclick: move || {
                                    adding.set(false);
                                    trouble.set(None);
                                    crate::picker::pick(crate::pdf::PICK_REQUEST, move |picked| {
                                        match picked {
                                            // Not an error. The user closed a
                                            // dialog, which they are allowed to
                                            // do, and the screen says nothing.
                                            Picked::Cancelled => {}
                                            Picked::Failed(why) => trouble.set(Some(why)),
                                            Picked::Chose(file) => {
                                                if let Err(e) = crate::pdf::import(songs, id, file) {
                                                    trouble.set(Some(e.message()));
                                                }
                                            }
                                        }
                                    });
                                },
                                style: {PRODUCER_ROW},
                                AttachmentThumb { kind: {Some(AttachmentKind::Pdf)} }
                                span { style: "flex: 1; font-weight: 500; font-size: 15px;", "Pick a PDF" }
                                span { style: "color: var(--sla-muted); display: flex;",
                                    {icon(__scope, TablerIcon::ChevronRight, 17)}
                                }
                            }

                            // E2. The screen it opens does the network work on
                            // a thread of its own; nothing about pressing this
                            // row blocks, and the capture cannot outlive the
                            // screen it is drawn on — see `screens::capture`.
                            div {
                                onclick: move || nav.go(Route::CaptureWebpage { song: id }),
                                style: {PRODUCER_ROW},
                                AttachmentThumb { kind: {Some(AttachmentKind::CapturedPage)} }
                                span { style: "flex: 1; font-weight: 500; font-size: 15px;", "Save a webpage offline" }
                                span { style: "color: var(--sla-muted); display: flex;",
                                    {icon(__scope, TablerIcon::ChevronRight, 17)}
                                }
                            }

                            // D2, which used to be what the accent line did on
                            // its own.
                            div {
                                onclick: move || nav.go(Route::TypeChart { song: id, chart: None }),
                                style: {PRODUCER_ROW},
                                AttachmentThumb { kind: {Some(AttachmentKind::Text)} }
                                span { style: "flex: 1; font-weight: 500; font-size: 15px;", "Type lyrics / chords" }
                                span { style: "color: var(--sla-muted); display: flex;",
                                    {icon(__scope, TablerIcon::ChevronRight, 17)}
                                }
                            }
                        }
                    }

                    // What went wrong, if anything did. An inline strip in the
                    // flow rather than a dialog, for the reason `chart_editor`
                    // gives: there is no modal anywhere in this app. It clears
                    // itself the next time the chooser is opened, so a
                    // complaint never outlives the attempt that caused it.
                    if trouble.get().is_some() {
                        div {
                            style: {format!("{T_META_SMALL} color: var(--sla-danger); margin-top: 10px;")},
                            {move || trouble.get().unwrap_or_default()}
                        }
                    }
                }
            }

            // Footer: add to setlist, plus a one-song performance start.
            div {
                style: {format!("display: flex; gap: 10px; padding: 12px {SCREEN_PAD} 24px; \
                                 border-top: 1px solid var(--sla-hairline);")},
                div {
                    onclick: move || nav.add_to_setlist_for.set(Some(id)),
                    style: "flex: 1; background: var(--sla-ink); color: var(--sla-paper); \
                            border-radius: 14px; padding: 14px; text-align: center; \
                            font-weight: 600; font-size: 16px;",
                    "Add to setlist"
                }
                div {
                    style: "width: 52px; background: var(--sla-fill); border-radius: 14px; \
                            display: flex; align-items: center; justify-content: center; color: var(--sla-ink-2);",
                    {icon(__scope, TablerIcon::PlayerPlay, 19)}
                }
            }
        }
    }
}

/// Order as authored: key · tuning · capo · bpm · duration · tags. The key
/// chip is the only tinted one.
fn metadata_chips(song: &Song) -> Vec<(String, bool)> {
    let mut chips = Vec::new();
    if let Some(k) = &song.key {
        chips.push((k.clone(), true));
    }
    if let Some(t) = &song.tuning {
        chips.push((t.clone(), false));
    }
    if let Some(c) = song.capo {
        chips.push((format!("capo {c}"), false));
    }
    if let Some(b) = song.tempo {
        chips.push((format!("{b} bpm"), false));
    }
    if let Some(d) = song.duration {
        chips.push((fmt_duration(d), false));
    }
    for tag in &song.tags {
        chips.push((tag.clone(), false));
    }
    chips
}

/// The header's five derived values, each as a plain `fn` over `Copy` store
/// handles so it can be called from inside a reactive closure — the shape
/// `primary_of` below and `songs_in_group` in `library.rs` already use, and
/// the shape card K54 needed the top of this screen to use as well.
///
/// A song that is gone reads as empty rather than as its last known value.
/// Nobody should see that: `crate::app`'s J7 effect leaves this route the
/// moment `songs.get(id)` starts coming back `None`, and the guard in the
/// component body catches the frame before it fires. Empty is still the right
/// answer for the frame that does not exist, because the alternative — holding
/// the last value — is the bug this card is about.
fn title_of(songs: SongsStore, id: SongId) -> String {
    songs.get(id).map(|song| song.title).unwrap_or_default()
}

fn artist_of(songs: SongsStore, id: SongId) -> String {
    songs.get(id).map(|song| song.artist).unwrap_or_default()
}

fn confidence_of(songs: SongsStore, id: SongId) -> Option<Confidence> {
    songs.get(id).and_then(|song| song.confidence)
}

/// The chips, read now rather than at mount. `MetaChip`'s `key` is the label
/// itself, so a chip whose text changes is a different key and is drawn again
/// — which matters, because a `for` reuses the DOM of an item whose key is
/// unchanged and would otherwise leave an edited key or tempo reading the old
/// number.
fn chips_of(songs: SongsStore, id: SongId) -> Vec<(String, bool)> {
    songs.get(id).map(|song| metadata_chips(&song)).unwrap_or_default()
}

/// Facts that follow the confidence word, each only when it exists.
///
/// Reads both stores, and has to: `Mark as played` writes the day through
/// `SongsStore`, while adding this song to a set writes through
/// `SetlistsStore` from a sheet that opens over this very screen. Either one
/// alone would have left half this line frozen.
fn status_tail_of(songs: SongsStore, setlists: SetlistsStore, id: SongId) -> String {
    let Some(song) = songs.get(id) else {
        return String::new();
    };
    let set_count = setlists.containing(id).len();

    let mut parts = Vec::new();
    if let Some(d) = song.last_played {
        parts.push(format!("Played {}", d.short()));
    }
    if set_count > 0 {
        parts.push(format!("{set_count} setlists"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("· {}", parts.join(" · "))
    }
}

/// How many lines of a chart the card shows before it stops. The card is a
/// preview — the viewer (D5) is where a chart is read.
const PREVIEW_LINES: usize = 8;

/// The primary attachment, as a nought-or-one vector so `rsx!`'s `for` can
/// stand in for an `if let`. Recomputed from the stores, so it takes only
/// `Copy` arguments and can be called from inside a reactive closure.
fn primary_of(
    songs: SongsStore,
    attachments: AttachmentsStore,
    id: SongId,
) -> Vec<Attachment> {
    songs
        .get(id)
        .and_then(|song| song.primary())
        .and_then(|primary| attachments.get(primary))
        .into_iter()
        .collect()
}

/// Everything except the primary chart, in the order it was attached.
fn others(songs: SongsStore, attachments: AttachmentsStore, id: SongId) -> Vec<Attachment> {
    let Some(song) = songs.get(id) else {
        return Vec::new();
    };
    let primary = song.primary();
    attachments
        .many(&song.attachments)
        .into_iter()
        .filter(|a| Some(a.id) != primary)
        .collect()
}

/// One collapsed row per non-primary chart: `(index, label, is expanded)`.
///
/// Rows are addressed by index rather than by the attachment itself, because
/// the `rsx!` closures nested inside a row can only capture `Copy` values —
/// the pattern `songs_in_group` in the library uses, for the same reason.
/// Reading `expanded` here is what re-runs the list when a row opens.
fn other_rows(
    songs: SongsStore,
    attachments: AttachmentsStore,
    id: SongId,
    expanded: Signal<Vec<AttachmentId>>,
) -> Vec<(usize, String, bool)> {
    let open = expanded.get();
    others(songs, attachments, id)
        .into_iter()
        .enumerate()
        .map(|(index, a)| {
            (
                index,
                format!("{} · {}", a.title, a.kind.descriptor()),
                open.contains(&a.id),
            )
        })
        .collect()
}

fn other_id(
    songs: SongsStore,
    attachments: AttachmentsStore,
    id: SongId,
    index: usize,
) -> Option<AttachmentId> {
    others(songs, attachments, id).get(index).map(|a| a.id)
}

/// Open or close one collapsed row. A signal, a screen, and no write: this is
/// the whole of "expanding a row does not make it primary".
fn toggle_expanded(
    songs: SongsStore,
    attachments: AttachmentsStore,
    id: SongId,
    index: usize,
    expanded: Signal<Vec<AttachmentId>>,
) {
    let Some(attachment) = other_id(songs, attachments, id, index) else {
        return;
    };
    expanded.update(|open| match open.iter().position(|a| *a == attachment) {
        Some(at) => {
            open.remove(at);
        }
        None => open.push(attachment),
    });
}

/// `primary · 2 pages · 412 KB`, or `primary · saved page` for a kind that has
/// no pages to count.
///
/// The hi-fi draws `primary · 2 pages` under a `tab.pdf`, and that is what the
/// first two facts are. The size is D3's addition and only a PDF gets one: an
/// imported chart is the one attachment kind whose bytes came from outside and
/// were chosen, the card that asked for the import asked for the size to be
/// visible with it, and the storage screen that would otherwise be the only
/// place to see it (`5c`, card H1) does not exist yet. A typed chart's size is
/// the length of what is on the screen already, and a capture's is not
/// something anybody picked, so neither grows a number the design did not ask
/// for.
///
/// The count is pluralised. `page_count` has been in the model since D1 and
/// nothing could set it from a real file until now, so `1 pages` was a string
/// no run of this app had ever produced — and a one-page chart is the single
/// most likely thing somebody imports.
fn primary_subtitle(attachment: &Attachment) -> String {
    let mut parts = vec!["primary".to_string()];
    match attachment.page_count {
        Some(n) => parts.push(format!("{n} {}", if n == 1 { "page" } else { "pages" })),
        None => parts.push(attachment.kind.descriptor().to_string()),
    }
    if attachment.kind == AttachmentKind::Pdf {
        parts.push(fmt_bytes(attachment.bytes_on_disk));
    }
    parts.join(" · ")
}

fn preview_of_primary(
    songs: SongsStore,
    attachments: AttachmentsStore,
    id: SongId,
    viewport: f32,
) -> Vec<(usize, String, bool)> {
    match songs.get(id).and_then(|song| song.primary()) {
        Some(primary) => preview(attachments, primary, viewport),
        None => Vec::new(),
    }
}

/// The inlined content of an expanded row, and nothing at all for a closed one.
fn preview_of_row(
    songs: SongsStore,
    attachments: AttachmentsStore,
    id: SongId,
    index: usize,
    expanded: Signal<Vec<AttachmentId>>,
    viewport: f32,
) -> Vec<(usize, String, bool)> {
    let Some(attachment) = other_id(songs, attachments, id, index) else {
        return Vec::new();
    };
    if !expanded.get().contains(&attachment) {
        return Vec::new();
    }
    preview(attachments, attachment, viewport)
}

/// What one chart shows: `(index, text, is a note)`.
///
/// A note is the muted sentence that stands in when there is nothing to render
/// — which is most of Phase D's job still to come, and the reason this replaced
/// six grey bars. A skeleton says "loading"; three of the six lines here would
/// have been a lie about a PDF that has never been rasterised.
///
/// This was the one place in the app that read an attachment body until card
/// G2's search screen became the second — see `screens::search::
/// attachment_texts`, which reads every one of them, once per keystroke,
/// rather than once per redraw of one open card. Both cost one object read
/// per attachment; the performance budget's rule is about *rows* — the
/// library builds three hundred of them and must not touch a body for any —
/// and the row list still cannot: `AttachmentsStore::items` never carries one.
fn preview(attachments: AttachmentsStore, id: AttachmentId, viewport: f32) -> Vec<(usize, String, bool)> {
    let Some(attachment) = attachments.get(id) else {
        return Vec::new();
    };
    // E5: a captured page is not eight lines of its extracted text any more.
    // `captured_page::CapturedPageView`, mounted beside this in the card, draws
    // the saved markup itself — and owns the text fallback and all three of the
    // sentences that go with it, so that the card and the full-screen viewer
    // cannot say different things about the same chart. Returning early here is
    // what stops the two of them drawing it twice.
    if attachment.kind == AttachmentKind::CapturedPage {
        return Vec::new();
    }
    let body = attachments.body(id).unwrap_or_default();

    // Leading blank lines are an artefact of however the text was written and
    // waste a preview that is only eight lines tall. Blank lines *inside* the
    // chart separate its verses, so they are kept — as a space, because an
    // empty text node collapses and the gap it stands for disappears with it.
    let all: Vec<&str> = body.lines().skip_while(|l| l.trim().is_empty()).collect();
    let mut lines: Vec<(usize, String, bool)> = all
        .iter()
        .take(PREVIEW_LINES)
        .enumerate()
        .map(|(index, line)| {
            let text = if line.trim().is_empty() {
                " ".to_string()
            } else {
                line.to_string()
            };
            (index, text, false)
        })
        .collect();

    // A chart that is showing its first page is not a chart with nothing to
    // show, so the sentence that says so is suppressed — otherwise a PDF whose
    // page rasterised would draw the picture *and* "No page preview yet."
    // underneath it. The `+N more lines` half of `note` cannot fire here
    // anyway, because a PDF has no extracted body to have more lines of, but
    // the whole call is skipped rather than half of it so that G2's text
    // extraction does not have to remember this when it gives one a body.
    if page_image(attachments, id, card_page_width(viewport)).is_none() {
        if let Some(note) = note(&attachment, lines.len(), all.len()) {
            lines.push((lines.len(), note, true));
        }
    }
    lines
}

/// How wide a rasterised page is drawn, in CSS pixels, in each of the two
/// places one appears.
///
/// Arithmetic rather than taste, and spelled out because the numbers it is made
/// of are elsewhere: the window is whatever the platform gave the app
/// (`platform::viewport_width`), the scrolling column insets it by `SCREEN_PAD`
/// on each side (src/theme.rs), and the primary card adds 18 px of padding of
/// its own — the literal in the card's own `style` a hundred lines above. A
/// collapsed row has no card, so it gets the column width.
///
/// A number, rather than `100%`, because of [`page_image`] — see there.
///
/// **Functions rather than the two `const`s they were, and that is the whole of
/// card K31's edit in this file.** They were `crate::WIDTH - …`, and
/// `crate::WIDTH` is 393 because 393 is the design handoff's canvas, not
/// because it is the window: on the moto g stylus 5G the window is 432 logical
/// pixels, so a card page sat 313 wide where it had 352 to fill.
/// A width that is only known once there is a surface cannot be a `const`, and
/// the alternative — pushing the arithmetic out to the four call sites — would
/// have put the same subtraction in four places so that it could stay spelled
/// with an `=` instead of a `()`.
///
/// **They take the width rather than going and getting it, and that is card
/// K53's edit.** K31 left them calling `platform::viewport_width()`, which was
/// cheap because that shim cached its Android reading — and frozen, because it
/// cached it in a `OnceLock` for the life of the process. The width now
/// arrives from `SystemStore::viewport_width`, read in the render closures at
/// the top of this file, so a window that changes size re-fits these pages
/// instead of keeping the number the app launched with. It also makes both of
/// these plain arithmetic over an argument, which is why the test at the
/// bottom of this file can state a page width without a window.
///
/// Nothing here re-*rasterises*: `pdf::pages` draws every page once at a fixed
/// 1080 px and these two only decide the box it is fitted into. See
/// `crate::store::system`'s header for why that means a width change has
/// nothing on disk to invalidate.
fn card_page_width(viewport: f32) -> u32 {
    row_page_width(viewport).saturating_sub(2 * 18)
}

fn row_page_width(viewport: f32) -> u32 {
    (viewport as u32).saturating_sub(2 * SCREEN_PAD_PX)
}

/// The type size a captured page is drawn at inside a card.
///
/// The same 12.5 px `theme::T_CHART` gives the text preview this replaced, so
/// that a captured page and a typed one still read at the same size on the same
/// screen. Everything in the page's own fragment is in `em` and resolves against
/// this — see `capture::render`'s header.
const CARD_BASE_PX: f32 = 12.5;

/// How tall the captured-page preview is, in CSS pixels.
///
/// [`PREVIEW_LINES`] lines of `T_CHART`, which is 12.5 px on a 1.5 line: the
/// box is exactly the height of the eight lines of extracted text it replaced,
/// so a song with a typed chart and a captured one still has two cards of the
/// same shape. It clips rather than scrolls, because a card that scrolled would
/// be a second place to read a chart competing with the one the tap opens.
const CARD_PREVIEW_HEIGHT: u32 = (PREVIEW_LINES as f32 * CARD_BASE_PX * 1.5) as u32;

/// `SCREEN_PAD` as a number. `crate::theme::SCREEN_PAD` is the string `"22px"`,
/// because everything else that uses it is interpolating it into CSS; this is
/// the one place that has to do sums with it.
const SCREEN_PAD_PX: u32 = 22;

/// The rasterised first page of a chart, as `(src, width, height)` in CSS
/// pixels — or nothing, for every kind that is not a PDF and every PDF whose
/// page has not been drawn.
///
/// D4. `pages::cached_page` is one `stat` and `pages::page_pixels` is 24 bytes
/// off the front of a file, and neither **ever renders**. That is what makes
/// them safe to call from here: this runs inside the reactive closure that
/// rebuilds the attachment card, so it runs on every redraw of it, and a
/// function that could spend 74 ms of a phone's main thread rasterising a page
/// has no business on that path. Drawing happens in exactly two places, neither
/// of them a render closure — `pdf::import`, for page one, and
/// `pages::ensure_page` from [`SongDetail`]'s own body when the screen mounts.
///
/// ## Why the size is computed here instead of written as `width: 100%`
///
/// Because `width: 100%` on an `<img>` gives Rinch a page at its natural height
/// — measured on 2026-08-28 at 373 x 1398 in a 393 px window, a page fourteen
/// hundred pixels tall spilling out of a card. Rinch's Taffy measure function
/// derives the missing axis from the image's aspect ratio only when the other
/// axis arrives as a `known_dimension`, and a percentage width does not: the
/// node ends up laid out at the style's width and the bitmap's *unscaled*
/// height. Stating both axes sidesteps the measure function altogether — Taffy
/// does not call it when neither dimension is in question — so the page is
/// exactly as tall as it is wide times its own shape, on the first frame, with
/// no dependence on how a percentage happened to resolve.
///
/// That leaves this function needing the page's real proportions, which is what
/// `page_pixels` reads out of the PNG header without decoding it.
///
/// ## Why the path is bare
///
/// An absolute path rather than a `file://` URL: Rinch's `FileImageLoader`
/// strips that scheme if it is there and reads the rest as a path either way,
/// and a path that has been through URL encoding is a path a library name with
/// a space in it can break.
fn page_image(
    attachments: AttachmentsStore,
    id: AttachmentId,
    width: u32,
) -> Option<(String, u32, u32)> {
    let attachment = attachments.get(id)?;
    if attachment.kind != AttachmentKind::Pdf {
        return None;
    }
    let directory = attachments.directory(id)?;
    let path = crate::pdf::pages::cached_page(&directory, 1)?;
    let (page_width, page_height) = crate::pdf::pages::page_pixels(&path)?;
    // Rounded up, so a page is never a pixel shorter than its own shape and
    // never leaves a hairline of card showing under its bottom edge.
    let height = (width as u64 * page_height as u64).div_ceil(page_width as u64) as u32;
    Some((path.to_string_lossy().into_owned(), width, height))
}

/// The primary chart's id, but only when it is a captured page.
///
/// Nought-or-one for the reason everything else on this screen is: `rsx!`'s
/// `for` is the only reactive conditional the macro has. It is an id rather
/// than a rendered page because the rendering happens inside
/// `CapturedPageView`'s own body, which runs once per mount — this runs on
/// every redraw of the card, and a full html5ever parse of somebody's saved
/// page has no business on that path. It is the same division of labour
/// `page_image` keeps for a PDF: the cheap question here, the expensive work
/// somewhere that happens once.
fn capture_of_primary(
    songs: SongsStore,
    attachments: AttachmentsStore,
    id: SongId,
) -> Vec<AttachmentId> {
    songs
        .get(id)
        .and_then(|song| song.primary())
        .filter(|primary| {
            attachments.get(*primary).map(|a| a.kind) == Some(AttachmentKind::CapturedPage)
        })
        .into_iter()
        .collect()
}

/// The same, for an expanded row.
fn capture_of_row(
    songs: SongsStore,
    attachments: AttachmentsStore,
    id: SongId,
    index: usize,
    expanded: Signal<Vec<AttachmentId>>,
) -> Vec<AttachmentId> {
    let Some(attachment) = other_id(songs, attachments, id, index) else {
        return Vec::new();
    };
    if !expanded.get().contains(&attachment) {
        return Vec::new();
    }
    if attachments.get(attachment).map(|a| a.kind) != Some(AttachmentKind::CapturedPage) {
        return Vec::new();
    }
    vec![attachment]
}

/// The same thing as a nought-or-one vector, because `rsx!`'s `for` is the only
/// reactive conditional the macro has — the pattern `primary_of` above uses,
/// and for the same reason.
fn page_of_primary(
    songs: SongsStore,
    attachments: AttachmentsStore,
    id: SongId,
    viewport: f32,
) -> Vec<(String, u32, u32)> {
    songs
        .get(id)
        .and_then(|song| song.primary())
        .and_then(|primary| page_image(attachments, primary, card_page_width(viewport)))
        .into_iter()
        .collect()
}

/// The first page of an expanded row's chart, or nothing when the row is shut.
///
/// A collapsed row gets the same picture the card does when it is opened. The
/// alternative — a page in the card and the old sentence in the rows — would
/// have said two different things about two charts on one screen, and neither
/// of them would have been about which chart was primary.
fn page_of_row(
    songs: SongsStore,
    attachments: AttachmentsStore,
    id: SongId,
    index: usize,
    expanded: Signal<Vec<AttachmentId>>,
    viewport: f32,
) -> Vec<(String, u32, u32)> {
    let Some(attachment) = other_id(songs, attachments, id, index) else {
        return Vec::new();
    };
    if !expanded.get().contains(&attachment) {
        return Vec::new();
    }
    page_image(attachments, attachment, row_page_width(viewport))
        .into_iter()
        .collect()
}

/// The honest sentence under a preview: how much was left out, or why there was
/// nothing to show.
fn note(attachment: &Attachment, shown: usize, total: usize) -> Option<String> {
    if total > shown {
        let rest = total - shown;
        let lines = if rest == 1 { "line" } else { "lines" };
        return Some(format!("+{rest} more {lines}"));
    }
    if shown > 0 {
        return None;
    }
    // Nothing rendered. Say which of the three reasons it is, rather than
    // drawing bars that imply something is on its way.
    Some(
        match attachment.kind {
            AttachmentKind::Text => "Nothing typed yet.",
            // D4 rasterises a PDF's first page to a PNG beside it, and when
            // there is one `preview` shows the picture and never gets here. So
            // this sentence now means one of two things: hayro could not draw
            // the page, or this chart was imported before D4 existed and the
            // screen has not been opened since — `SongDetail` draws a missing
            // page on mount, so the second heals itself the moment anybody
            // looks. They are indistinguishable on disk and get one sentence
            // between them; telling them apart would need a marker file for a
            // difference the reader cannot act on either way.
            AttachmentKind::Pdf => "No page preview yet.",
            // Unreachable, and left as a `return` rather than a sentence so
            // that it stays that way. E5 moved everything a captured page says
            // about itself into `captured_page`, which owns the page, the text
            // it falls back to and the two sentences for when there is neither;
            // `preview` above returns before it can get here. A sentence in
            // this arm would be a fourth wording of the same state, in a file
            // that no longer knows enough to choose between them.
            AttachmentKind::CapturedPage => return None,
        }
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The width every page-width assertion in this module is made at.
    ///
    /// The desktop's real one, because that is the window `scripts/
    /// screenshot.sh` measures and several of its checks count absolute pixels
    /// recorded at exactly 393. It is a named constant rather than a literal
    /// because card K53 turned these helpers into arithmetic over an argument
    /// — before it, they went and got this number themselves, and a test could
    /// not have stated it.
    const TEST_VIEWPORT: f32 = crate::WIDTH as f32;
    use crate::model::{Attachment, AttachmentKind, Song};

    fn text(body: Option<&str>) -> Attachment {
        Attachment {
            id: 0,
            kind: AttachmentKind::Text,
            title: "chords.txt".into(),
            bytes_on_disk: 40,
            page_count: None,
            source_url: None,
            captured_at: None,
            body: body.map(str::to_string),
            rechecked_at: None,
        }
    }

    fn of_kind(kind: AttachmentKind) -> Attachment {
        Attachment {
            kind,
            body: None,
            ..text(None)
        }
    }

    /// The stores are `Copy` handles over signals, which is all these helpers
    /// need — no window, no database, no component.
    fn library(charts: Vec<Attachment>) -> (SongsStore, AttachmentsStore, SongId) {
        let songs = SongsStore::new(vec![Song::new(1, "Carolina", "M. Ward")]);
        for chart in charts {
            songs.attach(1, chart).expect("attached");
        }
        (songs, songs.attachments(), 1)
    }

    #[test]
    fn the_card_shows_the_primary_chart_and_the_rows_show_the_rest() {
        let (songs, attachments, id) = library(vec![
            text(Some("Capo 3")),
            Attachment { title: "lyrics.txt".into(), ..text(None) },
        ]);

        let card = primary_of(songs, attachments, id);
        assert_eq!(card.len(), 1);
        assert_eq!(card[0].title, "chords.txt");

        let rows = other_rows(songs, attachments, id, Signal::new(Vec::new()));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "lyrics.txt · typed");
        assert!(!rows[0].2, "and it starts closed");
    }

    #[test]
    fn a_song_with_no_charts_draws_neither_a_card_nor_a_row() {
        let (songs, attachments, id) = library(Vec::new());
        assert!(primary_of(songs, attachments, id).is_empty());
        assert!(other_rows(songs, attachments, id, Signal::new(Vec::new())).is_empty());
    }

    #[test]
    fn expanding_a_row_inlines_its_content_and_does_not_make_it_primary() {
        let (songs, attachments, id) = library(vec![
            text(Some("Capo 3")),
            Attachment {
                title: "lyrics.txt".into(),
                ..text(Some("Blue jean baby"))
            },
        ]);
        let expanded = Signal::new(Vec::new());
        let primary_before = songs.get(id).unwrap().primary_attachment;

        assert!(preview_of_row(songs, attachments, id, 0, expanded, TEST_VIEWPORT).is_empty());

        toggle_expanded(songs, attachments, id, 0, expanded);

        let lines = preview_of_row(songs, attachments, id, 0, expanded, TEST_VIEWPORT);
        assert_eq!(lines[0].1, "Blue jean baby");
        assert_eq!(
            songs.get(id).unwrap().primary_attachment,
            primary_before,
            "expanding is view state and writes nothing"
        );
        assert!(other_rows(songs, attachments, id, expanded)[0].2, "the chevron flips");

        // And it closes again.
        toggle_expanded(songs, attachments, id, 0, expanded);
        assert!(preview_of_row(songs, attachments, id, 0, expanded, TEST_VIEWPORT).is_empty());
    }

    #[test]
    fn a_typed_chart_renders_its_own_text() {
        let (songs, attachments, id) = library(vec![text(Some("G       D
Carolina"))]);
        let lines = preview_of_primary(songs, attachments, id, TEST_VIEWPORT);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].1, "G       D");
        assert_eq!(lines[1].1, "Carolina");
        assert!(lines.iter().all(|(_, _, note)| !note), "no note is needed");
    }

    #[test]
    fn a_long_chart_stops_and_says_how_much_is_left() {
        let body: String = (0..20).map(|n| format!("line {n}
")).collect();
        let (songs, attachments, id) = library(vec![text(Some(&body))]);
        let lines = preview_of_primary(songs, attachments, id, TEST_VIEWPORT);
        assert_eq!(lines.len(), PREVIEW_LINES + 1);
        assert_eq!(lines[PREVIEW_LINES], (PREVIEW_LINES, "+12 more lines".into(), true));
    }

    #[test]
    fn a_blank_line_between_verses_survives_but_a_leading_one_does_not() {
        let (songs, attachments, id) = library(vec![text(Some("

verse one

verse two"))]);
        let lines = preview_of_primary(songs, attachments, id, TEST_VIEWPORT);
        assert_eq!(lines[0].1, "verse one");
        assert_eq!(lines[1].1, " ", "the gap is kept, as something with a height");
        assert_eq!(lines[2].1, "verse two");
    }

    /// The card the handoff draws is a preview of *content*. Where there is
    /// none, this app says so rather than drawing the six grey bars the hi-fi
    /// uses as a placeholder — those would claim a PDF had a preview coming.
    #[test]
    fn a_kind_with_nothing_to_render_says_so_instead_of_faking_it() {
        for (kind, expected) in [
            (AttachmentKind::Pdf, "No page preview yet."),
            (AttachmentKind::Text, "Nothing typed yet."),
        ] {
            let (songs, attachments, id) = library(vec![of_kind(kind)]);
            let lines = preview_of_primary(songs, attachments, id, TEST_VIEWPORT);
            assert_eq!(lines.len(), 1, "one note, and no skeleton bars");
            assert_eq!(lines[0].1, expected);
            assert!(lines[0].2, "and it is styled as a note, not as a chart");
        }
    }

    /// The third kind is deliberately not in the list above any more. E5 moved
    /// a captured page out of this preview and into
    /// `captured_page::CapturedPageView`, which draws the saved markup and owns
    /// every sentence it can say instead — so a captured page contributing a
    /// line here would be the card saying the same thing twice, in two
    /// wordings, from two files.
    #[test]
    fn a_captured_page_draws_itself_and_leaves_this_preview_empty() {
        let (songs, attachments, id) = library(vec![Attachment {
            body: Some("Verse one".into()),
            ..of_kind(AttachmentKind::CapturedPage)
        }]);
        assert!(preview_of_primary(songs, attachments, id, TEST_VIEWPORT).is_empty());
        assert_eq!(capture_of_primary(songs, attachments, id).len(), 1);
        assert_eq!(
            note(&of_kind(AttachmentKind::CapturedPage), 0, 0),
            None,
            "and nothing reaches the sentence this file used to keep for it"
        );
    }

    #[test]
    fn a_pdf_names_its_page_count_and_a_typed_chart_names_itself() {
        let pdf = Attachment {
            title: "tab.pdf".into(),
            page_count: Some(2),
            bytes_on_disk: 412_000,
            ..of_kind(AttachmentKind::Pdf)
        };
        assert_eq!(primary_subtitle(&pdf), "primary · 2 pages · 412 KB");
        // Only a PDF carries a size. See `primary_subtitle` for why.
        assert_eq!(primary_subtitle(&text(None)), "primary · typed");
        assert_eq!(
            primary_subtitle(&of_kind(AttachmentKind::CapturedPage)),
            "primary · saved page"
        );
    }

    /// `1 pages` was unreachable until D3 gave `page_count` a real source, and
    /// a one-page chart is the most likely thing there is to import.
    #[test]
    fn one_page_is_a_page() {
        let one = Attachment {
            page_count: Some(1),
            bytes_on_disk: 40_000,
            ..of_kind(AttachmentKind::Pdf)
        };
        assert_eq!(primary_subtitle(&one), "primary · 1 page · 40 KB");
    }

    /// A PDF hayro could not parse still has to describe itself. `crate::pdf`
    /// imports it deliberately, so this is the row it gets.
    #[test]
    fn a_pdf_with_no_page_count_still_says_what_it_is_and_what_it_costs() {
        let unknown = Attachment {
            page_count: None,
            bytes_on_disk: 1_400_000,
            ..of_kind(AttachmentKind::Pdf)
        };
        assert_eq!(primary_subtitle(&unknown), "primary · pdf · 1.4 MB");
    }

    #[test]
    fn removing_the_primary_chart_moves_the_row_under_it_into_the_card() {
        let (songs, attachments, id) = library(vec![
            text(Some("Capo 3")),
            Attachment { title: "lyrics.txt".into(), ..text(None) },
        ]);
        let first = songs.get(id).unwrap().attachments[0];

        songs.detach(id, first);

        let card = primary_of(songs, attachments, id);
        assert_eq!(card[0].title, "lyrics.txt");
        assert!(other_rows(songs, attachments, id, Signal::new(Vec::new())).is_empty());
    }

    // ── K54: the header is a reader, not a photograph ───────────────────────

    /// A song and the sets it can be in, sharing one `Storage` the way the app
    /// does. `library` above cannot serve here: `SongsStore::new` builds its
    /// own `SetlistsStore` internally and hands out no way to reach it, and
    /// `status_tail_of` takes the one the screen pulls out of the context.
    fn library_and_sets() -> (SongsStore, SetlistsStore, SongId) {
        let storage = crate::store::Storage::in_memory();
        let attachments = AttachmentsStore::restored(storage, Vec::new());
        let setlists = SetlistsStore::restored(storage, Vec::new());
        let songs = SongsStore::restored(
            storage,
            attachments,
            setlists,
            vec![Song::new(1, "Carolina", "M. Ward")],
        );
        (songs, setlists, 1)
    }

    /// **The regression card K54 is.**
    ///
    /// Tapping `Set confidence › Rusty` in the ⋮ menu wrote the row and left
    /// the header reading `Solid`, because `SongDetail` had bound the song in
    /// its component body — which runs once, at mount — and the header drew
    /// from that. The write was never lost; nothing was watching for it.
    ///
    /// So the assertion is on what a reader *already watching* sees, exactly
    /// as `store::songs`' twin tests put it: asking `confidence_of` again
    /// afterwards passed on the broken build too, because the store was never
    /// the problem. What could not happen on the broken build is this — a
    /// reader established before the write, re-running because of it.
    #[test]
    fn a_confidence_change_reaches_a_header_that_is_already_watching() {
        use rinch::reactive::Effect;
        use std::cell::RefCell;
        use std::rc::Rc;

        let (songs, _setlists, id) = library_and_sets();
        let seen = Rc::new(RefCell::new(None));

        let s = seen.clone();
        let _header = Effect::new(move || *s.borrow_mut() = confidence_of(songs, id));

        assert_eq!(*seen.borrow(), None, "the song starts unrated");
        songs.set_confidence(id, Some(Confidence::Rusty));
        assert_eq!(
            *seen.borrow(),
            Some(Confidence::Rusty),
            "the word and the dots are drawn from this, and the phone's said \
             Solid until the screen was left and come back to"
        );
    }

    /// The rest of the header, which broke the same way and would have gone on
    /// breaking quietly: `Edit song…` navigates, so a title that only updated
    /// on remount looked correct, and would keep looking correct right up
    /// until something edited a song without changing the route.
    #[test]
    fn an_edit_reaches_the_title_the_artist_and_the_chips() {
        use rinch::reactive::Effect;
        use std::cell::RefCell;
        use std::rc::Rc;

        let (songs, _setlists, id) = library_and_sets();
        let seen = Rc::new(RefCell::new((String::new(), String::new(), Vec::new())));

        let s = seen.clone();
        let _header = Effect::new(move || {
            *s.borrow_mut() = (title_of(songs, id), artist_of(songs, id), chips_of(songs, id));
        });

        assert_eq!(seen.borrow().0, "Carolina");
        assert!(seen.borrow().2.is_empty(), "and it carries no metadata yet");

        songs.edit(id, |song| {
            song.title = "Chinese Translation".into();
            song.key = Some("G".into());
        });

        let header = seen.borrow();
        assert_eq!(header.0, "Chinese Translation");
        assert_eq!(header.1, "M. Ward", "the artist is untouched and stays");
        assert_eq!(
            header.2,
            vec![("G".to_string(), true)],
            "the key chip is the tinted one, and it arrived without a remount"
        );
    }

    /// The status line reads through both stores, and has to. `Mark as played`
    /// writes the day through `SongsStore`; `Add to setlist…` writes through
    /// `SetlistsStore`, from a sheet that opens *over* this screen and leaves
    /// it mounted. A version of this line derived from either store alone
    /// would freeze on the other one's writes.
    #[test]
    fn the_status_tail_follows_a_play_and_a_setlist() {
        use rinch::reactive::Effect;
        use std::cell::RefCell;
        use std::rc::Rc;

        let (songs, setlists, id) = library_and_sets();
        let seen = Rc::new(RefCell::new(String::new()));

        let s = seen.clone();
        let _tail = Effect::new(move || *s.borrow_mut() = status_tail_of(songs, setlists, id));

        assert_eq!(*seen.borrow(), "", "an unplayed song in no sets says nothing");

        songs.mark_played(id, crate::model::Day::new(2026, 9, 9));
        assert_eq!(*seen.borrow(), "· Played Sep 9");

        let friday = setlists.add("Friday");
        setlists.add_song(friday, id);
        assert_eq!(
            *seen.borrow(),
            "· Played Sep 9 · 1 setlists",
            "and the membership arrives from the other store, without a remount"
        );
    }

    /// A song that is gone reads as empty rather than as its last known value.
    /// The component body still returns "This song is gone." for the frame it
    /// catches; these are what the header would draw in the frame it does not.
    #[test]
    fn a_deleted_song_leaves_the_header_empty_rather_than_stale() {
        let (songs, setlists, id) = library_and_sets();
        songs.set_confidence(id, Some(Confidence::Solid));
        songs.delete(id);

        assert_eq!(title_of(songs, id), "");
        assert_eq!(artist_of(songs, id), "");
        assert_eq!(confidence_of(songs, id), None);
        assert!(chips_of(songs, id).is_empty());
        assert_eq!(status_tail_of(songs, setlists, id), "");
    }

    // ── D4: the page in the card ────────────────────────────────────────────

    /// The tests above run on in-memory stores, which is all a preview of
    /// *text* needs. A rasterised page is a file, so these need a library with
    /// a real directory under it — the same shape `crate::pdf`'s own tests use,
    /// and for the same reason.
    fn on_disk(name: &str) -> (SongsStore, AttachmentsStore, SongId) {
        let dir = crate::db::scratch(name);
        let storage = crate::store::Storage::open(&dir);
        let attachments = AttachmentsStore::restored(storage, Vec::new());
        let setlists = SetlistsStore::restored(storage, Vec::new());
        let songs = SongsStore::restored(storage, attachments, setlists, Vec::new());
        songs.create(Song::new(0, "Carolina", "M. Ward")).expect("a song");
        (songs, attachments, 1)
    }

    fn import_chart(songs: SongsStore, id: SongId, pages: usize) -> AttachmentId {
        crate::pdf::import(
            songs,
            id,
            crate::picker::PickedFile {
                name: Some("tab.pdf".to_string()),
                bytes: crate::pdf::test_support::inked_pdf(pages),
            },
        )
        .expect("imported")
    }

    /// The whole of what card D4 promised the card would do.
    #[test]
    fn an_imported_pdf_shows_its_first_page_instead_of_saying_there_is_none() {
        let (songs, attachments, id) = on_disk("song_detail_page_in_card");
        import_chart(songs, id, 2);

        let page = page_of_primary(songs, attachments, id, TEST_VIEWPORT);
        assert_eq!(page.len(), 1, "one page, and it is page one");
        let (src, width, height) = page[0].clone();
        assert!(
            std::path::Path::new(&src).is_file(),
            "the src an <img> is handed has to be a file that exists: {src}"
        );
        assert!(src.ends_with("page-1.png"));

        // Both axes are stated, and the shape is the page's own: US Letter
        // cached at 1080 x 1398, so 313 px of card is 313 x 1398 / 1080 = 405.1,
        // rounded up. See `page_image` for why a percentage width will not do
        // and why the rounding goes up.
        assert_eq!((width, height), (card_page_width(TEST_VIEWPORT), 406));
        assert_eq!(
            width, 313,
            "the 393 window this test platform really has, less 22 of column and 18 of card, twice"
        );

        assert!(
            preview_of_primary(songs, attachments, id, TEST_VIEWPORT).is_empty(),
            "and the sentence that stood in for it is gone, rather than sitting under it"
        );
    }

    /// The hole this card was cut to fill, still open for everything that is
    /// not a drawn PDF page.
    #[test]
    fn a_pdf_with_no_drawn_page_still_says_so(){
        let (songs, attachments, id) = on_disk("song_detail_page_missing");
        let chart = import_chart(songs, id, 1);
        let directory = attachments.directory(chart).expect("a directory");
        std::fs::remove_file(directory.join(crate::pdf::pages::page_file(1))).expect("the page");

        assert!(page_of_primary(songs, attachments, id, TEST_VIEWPORT).is_empty());
        let lines = preview_of_primary(songs, attachments, id, TEST_VIEWPORT);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].1, "No page preview yet.");
        assert!(lines[0].2, "and it is a note, not a chart");
    }

    /// A `.part` is what a process killed mid-write leaves behind. The card
    /// must not hand one to an image decoder that will only fail on it.
    #[test]
    fn an_interrupted_render_is_not_shown_as_a_page() {
        let (songs, attachments, id) = on_disk("song_detail_page_part");
        let chart = import_chart(songs, id, 1);
        let directory = attachments.directory(chart).expect("a directory");
        let page = directory.join(crate::pdf::pages::page_file(1));
        std::fs::rename(&page, page.with_extension("png.part")).expect("interrupt it");

        assert!(page_of_primary(songs, attachments, id, TEST_VIEWPORT).is_empty());
        assert_eq!(
            preview_of_primary(songs, attachments, id, TEST_VIEWPORT)[0].1,
            "No page preview yet."
        );
    }

    /// A typed chart has a directory too, and nothing in it to draw. The
    /// picture path must not claim one.
    #[test]
    fn only_a_pdf_gets_a_page() {
        let (songs, _attachments, id) = on_disk("song_detail_page_kinds");
        songs.attach(id, text(Some("Capo 3"))).expect("attached");
        let attachments = songs.attachments();

        assert!(page_of_primary(songs, attachments, id, TEST_VIEWPORT).is_empty());
        assert_eq!(preview_of_primary(songs, attachments, id, TEST_VIEWPORT)[0].1, "Capo 3");
    }

    /// A chart in a collapsed row gets the picture when the row is opened and
    /// not before, exactly as its text does.
    #[test]
    fn an_expanded_row_shows_its_page_and_a_shut_one_shows_nothing() {
        let (songs, attachments, id) = on_disk("song_detail_page_row");
        songs.attach(id, text(Some("Capo 3"))).expect("the primary");
        import_chart(songs, id, 1);
        let expanded = Signal::new(Vec::new());

        assert!(page_of_row(songs, attachments, id, 0, expanded, TEST_VIEWPORT).is_empty());
        toggle_expanded(songs, attachments, id, 0, expanded);
        assert_eq!(page_of_row(songs, attachments, id, 0, expanded, TEST_VIEWPORT).len(), 1);
    }
}

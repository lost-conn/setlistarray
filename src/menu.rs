//! The song overflow menu (C2) and the setlist card menu (C7).
//!
//! Both menus are the same shape: a `DropdownMenu` whose target is either an
//! explicit trigger (song detail's ⋮ button) or a whole row/card, opened by
//! right-click where there's no dedicated button (see "Long-press" below).
//!
//! `DropdownMenu`/`DropdownMenuItem` come from `rinch-components` rather than
//! being hand-rolled: they already solve open/close state, click-outside
//! dismissal, and viewport-edge flipping, which this app has no reason to
//! reimplement. Everything visual is overridden with `var(--sla-*)` tokens —
//! the component ships its own theme (`--rinch-color-*`), which this app
//! never wires up.
//!
//! ## Long-press
//!
//! Rinch has no press-and-hold gesture, and it can't be faked with
//! `onmousedown` + a timer either: `onclick` fires synchronously inside the
//! `MouseDown` handler (see `rinch/src/app/event_dispatch.rs`), before any
//! timer set from that same handler could run. A row's normal tap navigation
//! has already fired by the time a "held for 500ms" timer would check
//! anything — there's no gap to detect a hold in. The only way to get a
//! distinguishable "hold" signal today is to delay every tap's navigation
//! behind its own timer so a later long-press timer has a chance to cancel
//! it first, which would make every ordinary tap in the library feel laggy
//! to save a gesture the design only asks for as a shortcut.
//!
//! So: right-click (`oncontextmenu`) stands in for long-press. It's a real,
//! separately-dispatched event on this desktop backend — distinct from
//! `onclick`, doesn't fire navigation, and needs no polling. It's not what a
//! phone user will do, but the app only runs in a desktop window today (see
//! the crate root docs); when a touch backend exists this is the one place
//! that needs to change, and the menu content below is unaffected either
//! way.
//!
//! **That last sentence was overtaken twice in one day, and is now true
//! again.** Card C6's spike found that the Android backend does exist and
//! never synthesised `oncontextmenu` from touch at all — every touch was a tap
//! or a scroll, and nothing else. So for a while this was not a stand-in
//! awaiting a backend: on a phone it was a menu with no way in except song
//! detail's ⋮ button.
//!
//! That gap is what [joeleaver/rinch#266] closes. The Android recogniser now
//! holds a 500ms press timer and synthesises the same right-button press a
//! desktop right-click does, through the same dispatch — so this file did not
//! have to change, which is the outcome the paragraph above predicted. Both
//! menus have been opened by long press on a moto g stylus 5G running Android
//! 13, and the press does not also fire the tap underneath.
//!
//! Two caveats worth carrying. The fix lives on the branch this repo pins, not
//! on rinch `main`, so a build against upstream still has no way into these
//! menus. And opening one on a phone is what exposed the popup-viewport fault
//! ([#268], card K16): the menu flipped upward out of the scroll box because
//! `ClickContext` believed the screen was 394px tall. Fixing one Android input
//! fault revealed the next.
//!
//! ## And then the items were dead (K22)
//!
//! With the way in fixed and the placement fixed, every tap on an item closed
//! the menu and ran nothing — both menus, every coordinate, and on the desktop
//! with a mouse too, which is what said it was not a touch fault at all.
//!
//! `DropdownMenu` renders its dismiss backdrop as a `position: fixed`
//! full-viewport box with a `z-index` one below the panel's, which is what
//! anyone would write and what a browser would honour. Rinch does not: a fixed
//! box is viewport-level content, hoisted to the body out of every ancestor
//! clip — and since an overflow clip *is* a stacking context there, out of
//! every ancestor stacking context with it. The panel is `position: absolute`
//! and stays where the page put it, so behind this app's `overflow: hidden`
//! root the 99 and the 100 were never compared with each other and the
//! backdrop was simply on top, swallowing the tap and firing `on_close`.
//!
//! Fixed upstream in [#317] by making the backdrop `position: absolute`, in
//! the panel's own stacking context. Nothing in this file changed. The one
//! consequence worth knowing about here: the backdrop is now clipped by
//! whatever clips the panel, so with a menu open on the library screen a tap
//! in the header or the tab bar no longer dismisses it — a tap anywhere in the
//! list still does, and so does picking an item.
//!
//! [joeleaver/rinch#266]: https://github.com/joeleaver/rinch/pull/266
//! [#268]: https://github.com/joeleaver/rinch/pull/268
//! [#317]: https://github.com/joeleaver/rinch/pull/317

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::model::{AttachmentId, AttachmentKind, Confidence, Day, SetlistId, SongId};
use crate::store::{AttachmentsStore, NavStore, Route, SetlistsStore, SongsStore};

/// Surface for a dropdown/context menu panel — the app's `card` token, not
/// the component's own default theme.
pub const MENU_SURFACE: &str = "background: var(--sla-card); border: 1px solid var(--sla-hairline); \
    border-radius: 14px; box-shadow: var(--sla-card-shadow); padding: 6px 0; min-width: 210px; \
    font-family: var(--sla-font-ui);";

/// A `DropdownMenu`/`DropdownMenuTarget` sizes itself to its content
/// (`display: inline-block`) — fine for a small button, wrong for a target
/// that's a full-width row or card. Force it back to block-and-stretch.
pub const FULL_WIDTH_TARGET: &str = "display: block; width: 100%;";

const ITEM: &str =
    "font-size: 15px; font-weight: 500; padding: 11px 14px; color: var(--sla-ink-2);";
const ITEM_HIGHLIGHT: &str = "font-size: 15px; font-weight: 600; padding: 11px 14px; \
    background: var(--sla-accent-tint); color: var(--sla-accent-on-tint);";
const ITEM_DANGER: &str =
    "font-size: 15px; font-weight: 500; padding: 11px 14px; color: var(--sla-danger);";
const LABEL: &str = "font-size: 12px; font-weight: 500; letter-spacing: 0.1em; \
    text-transform: uppercase; padding: 10px 14px 4px; color: var(--sla-muted);";
const SUBITEM: &str =
    "font-size: 14px; font-weight: 500; padding: 9px 14px 9px 32px; color: var(--sla-ink-2);";

/// Solid · Rusty · Learning · Unrated, in the order the confidence dots fill.
fn confidence_choices() -> [(&'static str, Option<Confidence>); 4] {
    [
        ("Solid", Some(Confidence::Solid)),
        ("Rusty", Some(Confidence::Rusty)),
        ("Learning", Some(Confidence::Learning)),
        ("Unrated", None),
    ]
}

/// The song overflow menu — wireframe `2d`, order matters: **Add to
/// setlist…** (first, highlighted) · Edit song… · Set confidence · Mark
/// played today · Duplicate · Delete song (destructive). The same menu
/// reachable from song detail's ⋮ and from a library row's right-click.
///
/// "Edit song…" is the one row `2d` does not draw. The add/edit screen (`1j`)
/// exists and has to be reachable from a library row as well as from song
/// detail's pencil, and this menu is the only thing a library row opens. It
/// sits second because the wireframe is deliberate about Add-to-setlist being
/// first, and because everything below the label is a one-tap change to a
/// single field — Edit belongs with the whole-song actions at the top, not
/// buried under them.
///
/// "Set confidence" is spelled out as four always-visible rows rather than a
/// collapsible submenu: a submenu toggled by a signal would insert its items
/// via a *nested* reactive `if`, and `DropdownMenuItem`'s auto-close-on-click
/// only works for items that exist at the menu's initial render — the
/// thread-local it reads is cleared right after `DropdownMenu::render` runs,
/// long before a later toggle would insert new items into a reactive `if`.
/// Flattening avoids relying on a signal to open after the thread-local
/// closes.
#[component]
pub fn SongMenuItems(id: SongId) -> NodeHandle {
    let songs = use_store::<SongsStore>();
    let nav = use_store::<NavStore>();

    rsx! {
        div {
            DropdownMenuItem {
                left_section: TablerIcon::PlaylistAdd,
                style: {ITEM_HIGHLIGHT},
                onclick: move || nav.add_to_setlist_for.set(Some(id)),
                "Add to setlist…"
            }
            DropdownMenuItem {
                left_section: TablerIcon::Pencil,
                style: {ITEM},
                onclick: move || nav.go(Route::EditSong(id)),
                "Edit song…"
            }
            DropdownMenuLabel { style: {LABEL}, "Set confidence" }
            for choice in confidence_choices() {
                let label = choice.0;
                let value = choice.1;
                DropdownMenuItem {
                    key: {label},
                    style: {SUBITEM},
                    onclick: move || songs.set_confidence(id, value),
                    {label}
                }
            }
            DropdownMenuItem {
                left_section: TablerIcon::CalendarCheck,
                style: {ITEM},
                onclick: move || songs.mark_played(id, Day::today()),
                "Mark played today"
            }
            DropdownMenuItem {
                left_section: TablerIcon::Copy,
                style: {ITEM},
                onclick: move || { songs.duplicate(id); },
                "Duplicate"
            }
            DropdownMenuItem {
                left_section: TablerIcon::Trash,
                style: {ITEM_DANGER},
                onclick: move || songs.delete(id),
                "Delete song"
            }
        }
    }
}

/// The menu one attachment gets — **Set as primary** and **Remove
/// attachment** — opened by long-pressing the primary card or one of the
/// collapsed rows beneath it.
///
/// ## Why this is not an entry in the song's ⋮ menu
///
/// Card D1 says "set-primary from the ⋮ menu or long-press", and the obvious
/// reading is that a row joins `SongMenuItems` above. Wireframe `2d` says
/// otherwise, and it is deliberate about order: its five entries are Add to
/// setlist… · Set confidence · Mark played today · Duplicate · Delete song, and
/// every one of them is an action on *the song*. The handoff puts set-primary
/// somewhere else entirely — in the sentence describing the collapsed
/// attachment rows: "Expanding one inlines its content; it does not become
/// primary. Long-press or the ⋮ menu sets primary."
///
/// It has to be there, because "Set as primary" in the song's menu has no
/// object. A song with three charts would need a submenu naming all three, and
/// `SongMenuItems` already explains why a submenu cannot work here. So the
/// answer to "where in `2d`'s order does it go" is: nowhere. It goes on the
/// thing it is about, and `2d` keeps the five entries it was drawn with.
///
/// The way in is long-press — `oncontextmenu`, which rinch#266 now synthesises
/// from a held touch — exactly as it is for a library row and a setlist card.
/// The hi-fi draws these rows with a chevron and no ⋮ button, and adding one
/// would be the only overflow affordance in the app that is visible.
///
/// A chart that is already primary gets a one-item menu: there is nothing to
/// promote it to.
///
/// ## Edit lyrics / chords (D2)
///
/// A typed chart gets a third entry, and it is first. The card that built the
/// editor asked whether an existing typed chart is edited in the same screen —
/// it is, and this menu is the way back into it, because it is already the way
/// a chart is acted on. The primary card's own tap is spoken for: it says "Tap
/// to open full screen", which is the viewer (`1k`, card D5).
///
/// It only appears for `AttachmentKind::Text`. A PDF and a captured page are
/// not text this app wrote and it will not offer to rewrite them, so the branch
/// is taken here, at build time — the four shapes of this menu are four
/// `rsx!` blocks rather than a reactive `if`, for the same reason `is_primary`
/// is read here: `DropdownMenuItem`'s close-on-click only binds items that
/// exist at the menu's first render.
#[component]
pub fn AttachmentMenuItems(song: Option<SongId>, attachment: Option<AttachmentId>) -> NodeHandle {
    let songs = use_store::<SongsStore>();
    let attachments = use_store::<AttachmentsStore>();
    let song = song.unwrap_or_default();
    let attachment = attachment.unwrap_or_default();

    // Read at build time rather than in a closure: this component is rebuilt
    // whenever the row it hangs off is, and `DropdownMenuItem`'s close-on-click
    // only binds items that exist at the menu's first render.
    let is_primary = songs
        .get(song)
        .map(|s| s.primary() == Some(attachment))
        .unwrap_or(false);
    // The row, not the body: this is the list signal, which never carries one.
    let typed = attachments
        .get(attachment)
        .map(|a| a.kind == AttachmentKind::Text)
        .unwrap_or(false);

    match (typed, is_primary) {
        (true, true) => rsx! {
            div {
                EditChartItem { song: {song}, attachment: {attachment} }
                RemoveChartItem { song: {song}, attachment: {attachment} }
            }
        },
        (false, true) => rsx! {
            div {
                RemoveChartItem { song: {song}, attachment: {attachment} }
            }
        },
        (true, false) => rsx! {
            div {
                EditChartItem { song: {song}, attachment: {attachment} }
                SetPrimaryItem { song: {song}, attachment: {attachment} }
                RemoveChartItem { song: {song}, attachment: {attachment} }
            }
        },
        (false, false) => rsx! {
            div {
                SetPrimaryItem { song: {song}, attachment: {attachment} }
                RemoveChartItem { song: {song}, attachment: {attachment} }
            }
        },
    }
}

/// Reopen a typed chart in the editor that wrote it (D2). Navigating takes the
/// whole screen — and the menu with it — so this item does not depend on
/// close-on-click having bound it.
#[component]
fn EditChartItem(song: Option<SongId>, attachment: Option<AttachmentId>) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let song = song.unwrap_or_default();
    let attachment = attachment.unwrap_or_default();

    rsx! {
        DropdownMenuItem {
            left_section: TablerIcon::Pencil,
            style: {ITEM},
            onclick: move || nav.go(Route::TypeChart { song, chart: Some(attachment) }),
            "Edit lyrics / chords…"
        }
    }
}

#[component]
fn SetPrimaryItem(song: Option<SongId>, attachment: Option<AttachmentId>) -> NodeHandle {
    let songs = use_store::<SongsStore>();
    let song = song.unwrap_or_default();
    let attachment = attachment.unwrap_or_default();

    rsx! {
        DropdownMenuItem {
            left_section: TablerIcon::Star,
            style: {ITEM_HIGHLIGHT},
            onclick: move || { songs.set_primary(song, attachment); },
            "Set as primary"
        }
    }
}

#[component]
fn RemoveChartItem(song: Option<SongId>, attachment: Option<AttachmentId>) -> NodeHandle {
    let songs = use_store::<SongsStore>();
    let song = song.unwrap_or_default();
    let attachment = attachment.unwrap_or_default();

    rsx! {
        DropdownMenuItem {
            left_section: TablerIcon::Trash,
            style: {ITEM_DANGER},
            onclick: move || { songs.detach(song, attachment); },
            "Remove attachment"
        }
    }
}

/// The setlist card menu (C7): duplicate, rename, delete. "Rename" hands off
/// to `NavStore::renaming_setlist` — the Setlists screen owns the inline
/// text field, since a menu item can't itself host one (its own click would
/// immediately close the menu it's in).
#[component]
pub fn SetlistMenuItems(id: SetlistId) -> NodeHandle {
    let setlists = use_store::<SetlistsStore>();
    let nav = use_store::<NavStore>();

    rsx! {
        div {
            DropdownMenuItem {
                left_section: TablerIcon::Copy,
                style: {ITEM},
                onclick: move || { setlists.duplicate(id); },
                "Duplicate"
            }
            DropdownMenuItem {
                left_section: TablerIcon::Pencil,
                style: {ITEM},
                onclick: move || {
                    if let Some(sl) = setlists.get(id) {
                        nav.rename_draft.set(sl.name);
                    }
                    nav.renaming_setlist.set(Some(id));
                },
                "Rename"
            }
            DropdownMenuItem {
                left_section: TablerIcon::Trash,
                style: {ITEM_DANGER},
                onclick: move || setlists.delete(id),
                "Delete setlist"
            }
        }
    }
}

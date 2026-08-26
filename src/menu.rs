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

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::model::{Confidence, Day, SetlistId, SongId};
use crate::store::{NavStore, Route, SetlistsStore, SongsStore};

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
            for (label, value) in confidence_choices() {
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

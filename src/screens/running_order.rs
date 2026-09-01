//! The running order — NOT IN THE WIREFRAME. Card F3's own decision.
//!
//! `1o` draws a `≡ set` chip in the bottom bar of performance mode and draws
//! nothing for what it opens. F3 decided that too, and decided it as a **bottom
//! sheet**: the surface this app already has three of (`add_to_setlist` `2e`,
//! `sort_sheet` `2c`, `setlist_picker` `1i`), with the same panel, the same
//! scrim, the same tap-outside-to-dismiss and the same 220ms slide. Everything
//! in here comes from `ui::sheet_*` for that reason — a fourth sheet that
//! rounded its corners differently would be a fourth thing to look at rather
//! than the one this app has taught three times.
//!
//! Why a sheet at all, over a route: the set is being *played*. A screen would
//! take the chart off the stand to show a list, and the whole argument for `1i`
//! over the two-step wizard was that the thing you are working on should stay
//! visible behind the thing you are choosing from. That argument is stronger
//! here than it was there.
//!
//! ## Why it is mounted in `crate::app` and not inside `performance.rs`
//!
//! Because performance mode is `Route::full_screen`. Nested in that screen's
//! own column, a sheet would slide up *inside* the chart's box instead of over
//! the whole window, and its scrim would darken the chart and not the top bar.
//! Every sheet in this app is a sibling *after* the route in `crate::app`'s
//! tree, above everything, and that is what makes them work over a full-screen
//! route at all. The signal that opens it therefore has to be somewhere both
//! ends can see, which is `NavStore::running_order_for` — see its note.
//!
//! ## What it does not do yet
//!
//! It opens at the top of the set rather than scrolled to the song on the
//! stand. On a three-song set that is the same thing; on the twenty-two-song
//! one this was driven against, song 8 is on screen but song 18 is a swipe
//! away. Fixing it needs a way to scroll a box to a child, which is a DOM call
//! Rinch does not expose, and the workaround — mounting the list already
//! offset — would fight the sheet's own slide. It is worth a card rather than
//! a hack: the list is short enough to swipe and the mark is unmistakable when
//! you reach it.
//!
//! ## What it lists
//!
//! `performance::ordered`, and not `Setlist::song_ids`. A song deleted out of
//! the library underneath a running gig is gone from that list, so the numbers
//! down the left of this sheet are the same numbers the top bar's `2 / 4`
//! counts and the same segments the progress strip draws. Resolving the set a
//! second time here would be a second answer to "how long is this set", and the
//! first tap on a row would prove they disagreed.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{playing_at, setlist_summary};
use crate::model::{SetlistId, SongId};
use crate::store::{AttachmentsStore, NavStore, PlaybackStore, SetlistsStore, SongsStore};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_ROW_TITLE};
use crate::ui::{
    SheetFooter, SheetHandle, icon, sheet_panel_style, sheet_root_style, sheet_scrim_style,
};

use super::performance::{EMPTY_SET, ordered, realise_chart};

/// The same share of the window the sort sheet takes.
///
/// Deliberately the same rather than coincidentally: both are a list of rows to
/// pick one of, and this one has the extra job of showing where in the set you
/// are — which wants as many rows above and below the current song as it can
/// get. 68% of an 852px viewport is about ten rows, so a twelve-song set is
/// nearly all visible at once and a long one still scrolls to the song on the
/// stand's neighbours rather than to the top of the set.
const SHEET_HEIGHT: u32 = 68;

#[component]
pub fn RunningOrderSheet() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let setlists = use_store::<SetlistsStore>();
    let songs = use_store::<SongsStore>();
    let attachments = use_store::<AttachmentsStore>();
    let playback = use_store::<PlaybackStore>();

    let close = move || nav.running_order_for.set(None);

    rsx! {
        div {
            style: {move || sheet_root_style(nav.running_order_for.get().is_some())},

            div {
                onclick: close,
                style: {move || sheet_scrim_style(nav.running_order_for.get().is_some())},
            }

            div {
                style: {move || sheet_panel_style(nav.running_order_for.get().is_some(), SHEET_HEIGHT)},

                SheetHandle {}

                div {
                    style: {format!("padding: 2px {SCREEN_PAD} 12px; flex-shrink: 0;")},
                    div { style: {format!("{T_ROW_TITLE} font-size: 20px;")}, "Running order" }
                    // The set's own name and shape, so a person who opened this
                    // to check they are in the right gig gets the answer in the
                    // header rather than by reading the list.
                    div {
                        style: {format!("{T_META} margin-top: 3px;")},
                        {move || header_line(setlists, songs, nav)}
                    }
                }

                div {
                    style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD} 8px;")},

                    for row in rows(setlists, songs, playback, nav) {
                        // The `for` body re-runs as its own closure, so every
                        // value it draws is computed and owned up front.
                        let index = row.index;
                        let now = row.now;
                        // The song on the stand is marked the way the sort
                        // sheet marks its active row — a tint, not a weight —
                        // so the eye finds it without the list turning into two
                        // typefaces.
                        let band = if now {
                            "background: var(--sla-accent-tint); color: var(--sla-accent-on-tint); \
                             border-radius: 10px; border-bottom-color: transparent;"
                        } else {
                            ""
                        };
                        let number_color = if now {
                            "var(--sla-accent-on-tint)"
                        } else {
                            "var(--sla-muted)"
                        };
                        let meta_color = if now {
                            "var(--sla-accent-on-tint)"
                        } else {
                            "var(--sla-muted)"
                        };

                        div {
                            key: {row.id},
                            // Jump, then close. In that order, and with the
                            // page drawn in between: `realise_chart` is the
                            // self-healing pass a PDF chart needs before the
                            // frame that shows it, and this is a tap handler,
                            // which is the only place it is allowed to run
                            // from. See its note in `performance.rs`.
                            onclick: move || {
                                playback.index.set(index);
                                realise_chart(setlists, songs, attachments, playback, set_id(nav));
                                nav.running_order_for.set(None);
                            },
                            // Centred and not baseline-aligned, which is the
                            // one place this row departs from the setlist
                            // screen's. A row here can be two lines tall (title
                            // and the key/capo/tempo line) or one, and with the
                            // number baseline-aligned against a two-line column
                            // it came out level with the *second* line — a `3`
                            // sitting beside "capo 2" rather than beside the
                            // song it numbers. Centring is right for both
                            // heights and does not depend on which line an
                            // engine calls the item's baseline.
                            style: {format!(
                                "{band} display: flex; align-items: center; gap: 11px; \
                                 margin: 0 -10px; padding: 11px 10px; \
                                 border-bottom: 1px solid var(--sla-hairline-soft);"
                            )},
                            span {
                                style: {format!(
                                    "width: 18px; flex-shrink: 0; font-weight: 600; \
                                     font-size: 13px; color: {number_color};"
                                )},
                                {row.position.clone()}
                            }
                            div { style: "flex: 1; min-width: 0;",
                                div {
                                    style: {format!(
                                        "{T_ROW_TITLE} overflow: hidden; text-overflow: ellipsis; \
                                         white-space: nowrap;"
                                    )},
                                    {row.title.clone()}
                                }
                                // Nought-or-one: a song with no key, no capo
                                // and no tempo gets no second line, exactly as
                                // it gets none under the title on the screen
                                // behind this sheet.
                                for meta in row.meta.clone() {
                                    div {
                                        key: {meta.clone()},
                                        style: {format!(
                                            "{T_META_SMALL} margin-top: 2px; color: {meta_color};"
                                        )},
                                        {meta.clone()}
                                    }
                                }
                            }
                            // The mark, and it is a glyph rather than the word
                            // "now": this list is read at a glance, mid-song,
                            // and the play triangle is what the setlist cards
                            // and the Play set button already mean by "this one
                            // is the one being played".
                            if now {
                                span {
                                    style: "display: flex; align-items: center; \
                                            color: var(--sla-accent-on-tint); flex-shrink: 0;",
                                    {icon(__scope, TablerIcon::PlayerPlay, 15)}
                                }
                            }
                        }
                    }

                    // Reachable, just: the set can be emptied from another
                    // screen while this sheet is open. The bar that opens it is
                    // hidden for an empty set, so this is the state that arrives
                    // rather than the state you can ask for — and it says the
                    // same sentence the screen behind it is already saying.
                    for sentence in empty_note(setlists, songs, nav) {
                        div {
                            key: {sentence},
                            style: {format!("{T_META} padding: 28px 0; text-align: center;")},
                            {sentence}
                        }
                    }
                }

                SheetFooter {
                    note: "Tap a song to jump to it",
                    action: "Done",
                    enabled: true,
                    onclick: close,
                }
            }
        }
    }
}

/// One drawn row.
#[derive(Clone, PartialEq)]
struct Row {
    id: SongId,
    /// The index into the *resolved* set — what `PlaybackStore::index` is
    /// counted in, and so what a tap on this row sets it to.
    index: usize,
    /// `3`, one-based, as the eye counts a setlist.
    position: String,
    title: String,
    /// `G · capo 2 · 96 bpm`, as a nought-or-one vector — the same line the
    /// screen behind draws under the title, and empty for a song carrying none
    /// of the three.
    meta: Vec<String>,
    /// The song on the stand.
    now: bool,
}

/// The set this sheet is open over, or 0 when it is closed.
///
/// 0 is not a sentinel anybody has to check: `SetlistsStore::get` returns `None`
/// for it, `ordered` returns an empty list, and a closed sheet draws no rows —
/// which is what a closed sheet should draw. The same shape `performance.rs`
/// uses for its own `setlist.unwrap_or_default()`.
fn set_id(nav: NavStore) -> SetlistId {
    nav.running_order_for.get().unwrap_or_default()
}

/// The set's name and shape, under the sheet's title.
fn header_line(setlists: SetlistsStore, songs: SongsStore, nav: NavStore) -> String {
    let Some(setlist) = setlists.get(set_id(nav)) else {
        return String::new();
    };
    format!(
        "{} · {}",
        setlist.name,
        setlist_summary(&setlist, &songs.songs.get())
    )
}

/// The rows, recomputed from the stores. Takes only `Copy` arguments so it can
/// be called from inside a reactive closure.
fn rows(
    setlists: SetlistsStore,
    songs: SongsStore,
    playback: PlaybackStore,
    nav: NavStore,
) -> Vec<Row> {
    let set = ordered(setlists, songs, set_id(nav));
    // The same clamp the counter and the chevrons are drawn from, so the row
    // marked "now" is the song actually on the stand even when the index is
    // past the end of a set that shrank underneath the gig.
    let playing = playing_at(playback.index.get(), set.len()).map(|(index, _)| index);

    set.iter()
        .enumerate()
        .map(|(index, song)| Row {
            id: song.id,
            index,
            position: format!("{}", index + 1),
            title: song.title.clone(),
            meta: {
                let meta = crate::derive::performance_meta(song);
                if meta.is_empty() {
                    Vec::new()
                } else {
                    vec![meta]
                }
            },
            now: playing == Some(index),
        })
        .collect()
}

/// The sentence for a set with nothing playable left in it, nought-or-one.
fn empty_note(setlists: SetlistsStore, songs: SongsStore, nav: NavStore) -> Vec<&'static str> {
    if nav.running_order_for.get().is_some() && ordered(setlists, songs, set_id(nav)).is_empty() {
        vec![EMPTY_SET]
    } else {
        Vec::new()
    }
}

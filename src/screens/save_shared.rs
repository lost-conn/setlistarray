//! "Save this to…" — where a share out of another app lands.
//!
//! No wireframe drew this screen, because the handoff never imagined the app
//! being launched by somebody else. It is built out of pieces two existing
//! screens already agreed on: `search`'s header-and-field row, and
//! `setlist_picker`'s list of songs off [`picker_songs`] with the same
//! `Artist · key` sub-line. Nothing new was invented for it, deliberately —
//! a screen a user meets *before* they have ever opened the app themselves
//! (a share can cold-start it) is the worst possible place to teach a second
//! visual language.
//!
//! ## The one question it asks
//!
//! Which song is this? Everything on the screen is in service of that: the
//! shared thing at the top, so the answer is given with the question still
//! visible; the book underneath, searchable, because a 300-song library is not
//! a list you scroll; and `+ New song from this…` at the bottom, because the
//! most likely answer for a link somebody just found is "none of them yet".
//!
//! It does **not** ask anything else. There is no "save as reader text /
//! full page" here even though a link ends up on the capture screen where that
//! choice lives, and no title field even though every path invents a title. A
//! share is an interruption — the user was in another app a second ago — and
//! the way to respect that is to ask once and get out of the way. Both of those
//! settings are one screen further on, where the user is looking at the thing
//! they are about to keep.
//!
//! ## Where each answer goes
//!
//! | shared          | filed as                                            |
//! | --------------- | --------------------------------------------------- |
//! | a URL           | `Route::CaptureWebpage`, address field prefilled     |
//! | any other text  | an `AttachmentKind::Text` chart, body and all        |
//! | a PDF           | `crate::pdf::import`, exactly as `song_detail` does   |
//!
//! The middle row is the one worth defending, and `crate::share`'s header does:
//! declaring `text/plain` in the manifest puts this app in the share sheet for
//! *every* text share on the phone, so a shared verse is a share this app asked
//! for, and refusing it with an error would be a complaint about an entry the
//! app itself put in the chooser.
//!
//! ## The bytes are already here
//!
//! A shared PDF arrived as bytes, not as a URI to open when the user gets
//! round to picking a song — `share::read_stream` reads it inside the intent
//! handler because the read grant is scoped to the activity and is not this
//! screen's to rely on. So nothing on this screen can fail for lack of a file,
//! and the failure that *can* happen (a stream this app could not read at all)
//! arrives as `SharedItem::Unreadable` and is drawn instead of the list, with
//! no song rows under it: there is nothing to file, so there is no question to
//! ask.
//!
//! ## Handlers
//!
//! `onclick` and `oninput`, the two the Android touch recogniser delivers —
//! `src/gesture_reachability.rs`. The list rows are taps, like the picker's.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{SongFilter, picker_songs};
use crate::model::{Attachment, AttachmentKind, Day, SongId};
use crate::picker::PickedFile;
use crate::share::{SharedCapture, SharedItem, song_name_for};
use crate::store::{NavStore, Route, SongsStore};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_ROW_TITLE};
use crate::ui::{AttachmentThumb, IconButton, icon};

/// The `+ New song from this…` row, and the two rows above the list share its
/// shape. Tinted with the accent rather than drawn as a plain row because it is
/// the one control here that *creates* something, and on a screen whose whole
/// body is a list of things that already exist it would otherwise read as one
/// more of them.
const NEW_SONG_ROW: &str = "display: flex; align-items: center; gap: 10px; \
    background: var(--sla-accent-tint); color: var(--sla-accent-on-tint); \
    border-radius: 14px; padding: 13px 15px; font-weight: 600; font-size: 15px;";

#[component]
pub fn SaveShared() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();

    let query = Signal::new(String::new());
    // What a failed file said, if anything. Cleared by the next attempt, so a
    // complaint never outlives the problem — `song_detail`'s rule for the same
    // strip, and `capture`'s.
    let trouble = Signal::new(Option::<String>::None);

    // Card K32's lesson, applied to the one field on this screen: the soft
    // keyboard opened to search the library would otherwise cover the
    // `+ New song from this…` row entirely, which on a share of a link nobody
    // has a song for yet is the row the user actually came here to press.
    let keyboard_inset = crate::platform::keyboard_inset();

    // Leaving abandons the share, and that is the honest thing for it to do.
    // Nothing has been written yet — no song, no attachment, no bytes on disk —
    // so there is nothing to half-keep, and a share left pending would sit in
    // the store waiting to reopen this screen the next time anything navigated.
    // The shared thing is still in the other app; sharing again costs two taps.
    let leave = move || {
        nav.pending_share.set(None);
        nav.back();
    };

    // The phone's Back key is this screen's ←, the abandonment and all. It is
    // the same closure rather than a copy of it, per `register_back`'s rule —
    // and it matters more here than on most screens, because Back is what a
    // person reaches for by reflex when a share sheet has just thrown them into
    // an app they were not in a moment ago.
    nav.register_back(leave);

    rsx! {
        div { style: {move || format!(
            "flex: 1; display: flex; flex-direction: column; min-height: 0; \
             padding-bottom: {}px;",
            keyboard_inset.get(),
        )},

            // ← · heading. The same arrow at the same inset every secondary
            // screen in this app wears.
            div {
                style: {format!("padding: 2px {SCREEN_PAD} 8px; display: flex; align-items: center; gap: 8px; \
                                 border-bottom: 1px solid var(--sla-hairline);")},
                IconButton { glyph: TablerIcon::ChevronLeft, size: 19, onclick: leave }
                div { style: {format!("{T_ROW_TITLE} font-size: 19px; flex: 1;")}, "Save this to…" }
            }

            // What was shared, drawn as the row it is about to become on song
            // detail — same thumb, same `title · descriptor` shape. That is not
            // decoration: the user pressed Share in another app and this is the
            // app's first chance to show it understood what was sent.
            div {
                style: {format!("padding: 14px {SCREEN_PAD} 12px; display: flex; align-items: center; \
                                 gap: 12px; flex-shrink: 0;")},
                AttachmentThumb { kind: {move || nav.pending_share.get().and_then(|item| item.kind())} }
                div { style: "flex: 1; min-width: 0;",
                    div {
                        style: {format!("{T_ROW_TITLE} overflow: hidden; text-overflow: ellipsis; \
                                         white-space: nowrap;")},
                        {move || headline(nav)}
                    }
                    div { style: {format!("{T_META_SMALL} margin-top: 2px;")}, {move || destination(nav)} }
                }
            }

            // The one strip on this screen that says something went wrong.
            if let Some(why) = trouble.get() {
                div {
                    style: {format!("margin: 0 {SCREEN_PAD} 10px; padding: 10px 13px; \
                                     background: var(--sla-fill); border-radius: 12px; \
                                     {T_META} color: var(--sla-danger);")},
                    {why}
                }
            }

            // A share with nothing in it gets no library underneath it. There
            // is no question to ask — see the module header — so the screen
            // says what happened and stops, which is card J4's rule about
            // states with nothing the reader can do.
            //
            // The reason goes here, small and grey, rather than in the title
            // above: `SharedItem::headline`'s own comment is the argument for
            // that, and it was written after seeing
            // `readContentUri returned null (IO error or invalid URI)` set in
            // 19px display type on a real phone.
            if !savable(nav) {
                div {
                    style: {format!("{T_META} padding: 8px {SCREEN_PAD}; \
                                     display: flex; flex-direction: column; gap: 6px;")},
                    div { {move || reason(nav)} }
                    div {
                        "Nothing was saved. Try sharing it again, or open it in the other app \
                         and share the file itself."
                    }
                }
            }

            if savable(nav) {
                div {
                    style: {format!("margin: 0 {SCREEN_PAD} 10px; background: var(--sla-fill); border-radius: 14px; \
                                     padding: 10px 13px; display: flex; align-items: center; gap: 9px; flex-shrink: 0;")},
                    span { style: "color: var(--sla-muted); display: flex;", {icon(__scope, TablerIcon::Search, 16)} }
                    input {
                        r#type: "text",
                        style: "flex: 1; border: none; outline: none; background: transparent; \
                                font-family: var(--sla-font-ui); font-size: 14px; color: var(--sla-ink);",
                        placeholder: "Search your library",
                        value: {|| query.get()},
                        oninput: move |value: String| query.set(value),
                    }
                }

                div {
                    style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD};")},

                    for row in rows(songs, query) {
                        // The `for` body re-runs as its own closure, so it takes
                        // owned copies of everything it draws — the same rule
                        // `setlist_picker`'s list is written to.
                        let id = row.id;
                        div {
                            key: id,
                            onclick: move || {
                                trouble.set(None);
                                if let Err(why) = file_into(nav, songs, id, None) {
                                    trouble.set(Some(why));
                                }
                            },
                            style: "display: flex; align-items: center; gap: 11px; padding: 12px 0; \
                                    border-bottom: 1px solid var(--sla-hairline-soft);",
                            div { style: "flex: 1; min-width: 0;",
                                div {
                                    style: {format!("{T_ROW_TITLE} overflow: hidden; \
                                                     text-overflow: ellipsis; white-space: nowrap;")},
                                    {row.title.clone()}
                                }
                                div { style: {format!("{T_META_SMALL} margin-top: 2px;")}, {row.sub.clone()} }
                            }
                            span { style: "color: var(--sla-muted); display: flex; flex-shrink: 0;",
                                {icon(__scope, TablerIcon::ChevronRight, 17)}
                            }
                        }
                    }

                    // Nothing matched. A closure rather than a plain string for
                    // the reason `setlist_picker`'s empty note is one: the `if`
                    // re-runs when the list empties, but the text inside an
                    // already-rendered body does not.
                    if rows(songs, query).is_empty() {
                        div {
                            style: {format!("{T_META} padding: 22px 0;")},
                            {move || empty_note(&query.get())}
                        }
                    }

                    div { style: "height: 12px;" }
                }

                // The bottom row, and for a link nobody has a song for yet it
                // is the likeliest answer on the screen. It names what the song
                // will be called rather than promising a form: there is no form
                // — the song is minted with a name taken from the share and can
                // be renamed in the library like any other.
                div {
                    style: {format!("padding: 10px {SCREEN_PAD} 24px; flex-shrink: 0; \
                                     border-top: 1px solid var(--sla-hairline);")},
                    div {
                        onclick: move || {
                            trouble.set(None);
                            if let Err(why) = mint_and_file(nav, songs) {
                                trouble.set(Some(why));
                            }
                        },
                        style: {NEW_SONG_ROW},
                        span { style: "display: flex;", {icon(__scope, TablerIcon::Plus, 17)} }
                        span { style: "flex: 1; min-width: 0;", "New song from this…" }
                    }
                    div {
                        style: {format!("{T_META_SMALL} margin-top: 8px;")},
                        {move || minted_note(nav)}
                    }
                }
            }
        }
    }
}

/// One drawn row. Computed outside `rsx!` so the loop body has only owned
/// values to hand around — `setlist_picker::Row`'s reason, and the same shape.
#[derive(Clone, PartialEq)]
struct Row {
    id: SongId,
    title: String,
    sub: String,
}

/// The book, searched. [`picker_songs`] and not a fourth filtering rule of this
/// screen's own: it already does query, chip and A–Z sort, and the sort is the
/// one a picker wants (see its doc comment — a list used with a song already in
/// mind). The chip is fixed at `All` because this screen has no chip row; a
/// share is an interruption and one field is as much as it should ask for.
fn rows(songs: SongsStore, query: Signal<String>) -> Vec<Row> {
    picker_songs(
        songs.songs.get(),
        &query.get(),
        SongFilter::All,
        None,
        Day::today(),
    )
    .into_iter()
    .map(|song| Row {
        id: song.id,
        // `Artist · key`, the sub-line the setlist picker draws, for the same
        // reason it draws it: it is what tells two songs with the same title
        // apart, and nothing more, because this list has no room for more.
        sub: match &song.key {
            Some(key) => format!("{} · {key}", song.artist),
            None => song.artist.clone(),
        },
        title: song.title.clone(),
    })
    .collect()
}

/// What the picker says when the search empties the list. The book itself is
/// never empty here — a share into a fresh install lands on this screen with no
/// songs at all, and that is the state the `+ New song` row below is for.
fn empty_note(query: &str) -> String {
    let query = query.trim();
    if query.is_empty() {
        "Your book has no songs in it yet. Start one below.".to_string()
    } else {
        format!("Nothing in your book matches \u{201c}{query}\u{201d}.")
    }
}

/// The shared thing's own line, or nothing while the screen is on its way out.
fn headline(nav: NavStore) -> String {
    nav.pending_share
        .get()
        .map(|item| item.headline())
        .unwrap_or_default()
}

/// The line under it: what keeping this will actually get you.
/// [`SharedItem::destination`]'s doc comment is where the wording is argued,
/// including the version of it that went to the phone and read badly.
fn destination(nav: NavStore) -> String {
    match nav.pending_share.get() {
        Some(item) => item.destination().to_string(),
        None => String::new(),
    }
}

/// What the new song would be called, said out loud before it is minted.
fn minted_note(nav: NavStore) -> String {
    match nav.pending_share.get() {
        Some(item) => format!("Names it \u{201c}{}\u{201d}. You can change that later.", song_name_for(&item)),
        None => String::new(),
    }
}

/// Why the share could not be saved, in the words of whatever refused it.
/// Empty for a share that is fine, which is every share this line is not drawn
/// for.
fn reason(nav: NavStore) -> String {
    nav.pending_share
        .get()
        .and_then(|item| item.trouble().map(str::to_string))
        .unwrap_or_default()
}

/// Whether there is anything here to file. Read reactively, so the two `if`
/// blocks it gates re-run when the share is cleared on the way out.
fn savable(nav: NavStore) -> bool {
    nav.pending_share.get().is_some_and(|item| item.savable())
}

/// Mint a song named after the share, then file the share into it.
///
/// The name is [`song_name_for`]'s and it is passed onward as the *placeholder*
/// — see [`crate::share::rename_to_captured_title`] for what the capture screen
/// does with it when the fetch comes back knowing the page's real title, and
/// for the three cases where it declines to.
fn mint_and_file(nav: NavStore, songs: SongsStore) -> Result<(), String> {
    let Some(item) = nav.pending_share.get() else {
        return Ok(());
    };
    let name = song_name_for(&item);
    // `add` hands back an id that matches nothing when the write did not land
    // (see its own comment), and filing a chart onto a song that does not exist
    // would leave a chart nothing lists. Better to say so and keep the share.
    let song = songs.add(name.clone(), "");
    if songs.get(song).is_none() {
        return Err("Your library would not take a new song. Nothing was saved.".to_string());
    }
    file_into(nav, songs, song, Some(name))
}

/// File the pending share onto `song`, and leave this screen.
///
/// `placeholder` is `Some` only when this very call minted the song a moment
/// ago; it is `None` for a song the user picked out of the list, which is what
/// stops a capture renaming somebody's existing `Landslide` to whatever the
/// page it fetched happened to call itself.
///
/// The share is cleared before navigating in every arm that succeeds, and only
/// then: a failed import leaves the screen exactly as it was found, with the
/// share still in hand and the complaint above the list, so the user can pick a
/// different song rather than start again in the app they came from.
fn file_into(
    nav: NavStore,
    songs: SongsStore,
    song: SongId,
    placeholder: Option<String>,
) -> Result<(), String> {
    let Some(item) = nav.pending_share.get() else {
        return Ok(());
    };
    match item {
        // The capture screen does the rest, address field already filled in.
        // Nothing is written here at all — a link is not an attachment until a
        // page has actually come down — which is why this arm cannot fail.
        SharedItem::Link(url) => {
            nav.capture_handoff.set(Some(SharedCapture { url, placeholder }));
            nav.pending_share.set(None);
            nav.go(Route::CaptureWebpage { song });
            Ok(())
        }
        // The same row the typed editor writes, written without opening it.
        // `chart_editor::chart_title` and `writable` rather than a second
        // opinion of this screen's own: a chart shared in and a chart typed in
        // are the same row in the same list, and two title rules would show as
        // two lengths in one column.
        SharedItem::Chart(body) => {
            if super::chart_editor::writable(&body).is_none() {
                return Err("There are no words in that to save.".to_string());
            }
            let attachment = Attachment {
                id: 0,
                kind: AttachmentKind::Text,
                title: super::chart_editor::chart_title(&body),
                // Bytes and not blocks: a typed chart lives in its database row
                // rather than in a directory, so this is what it costs the
                // library — `chart_editor::save`'s own note.
                bytes_on_disk: body.len() as u64,
                page_count: None,
                source_url: None,
                captured_at: None,
                body: Some(body),
                rechecked_at: None,
            };
            if songs.attach(song, attachment).is_none() {
                return Err("Your library would not take that chart. Nothing was saved.".to_string());
            }
            nav.pending_share.set(None);
            nav.go(Route::SongDetail(song));
            Ok(())
        }
        // Byte for byte what `song_detail`'s `Pick a PDF` row does with a
        // picked file, and by design: a PDF that arrived through the share
        // sheet and a PDF that arrived through the system picker are the same
        // file, and `pdf::import` is the only thing in this app that judges
        // one.
        SharedItem::Pdf { name, bytes } => {
            // The `Arc` is very nearly always shared here — reading the share
            // out of the signal above cloned it, and the signal still holds the
            // other one — so this copies the file rather than taking it. That
            // is the price of the rule below, and it is the right way round:
            // the share is cleared only once `import` has said yes, so a PDF
            // this song refused is still in hand for the next song the user
            // taps. `try_unwrap` first anyway, because the cheap case costs a
            // refcount check and the expensive one is what would happen
            // regardless.
            let bytes = std::sync::Arc::try_unwrap(bytes).unwrap_or_else(|held| (*held).clone());
            match crate::pdf::import(songs, song, PickedFile { name, bytes }) {
                Ok(_) => {
                    nav.pending_share.set(None);
                    nav.go(Route::SongDetail(song));
                    Ok(())
                }
                Err(e) => Err(e.message()),
            }
        }
        // Unreachable: the list and the `+ New song` row are both inside an
        // `if savable(nav)`. Answered rather than `unreachable!()`, because the
        // cost of being wrong about that is a panic on a phone against a
        // sentence nobody was going to read anyway.
        SharedItem::Unreadable(why) => Err(why),
    }
}

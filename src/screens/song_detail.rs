//! Song detail — HI-FI.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::menu::{AttachmentMenuItems, FULL_WIDTH_TARGET, MENU_SURFACE, SongMenuItems};
use crate::model::{
    Attachment, AttachmentId, AttachmentKind, Song, SongId, fmt_bytes, fmt_duration,
};
use crate::picker::Picked;
use crate::store::{AttachmentsStore, NavStore, Route, SetlistsStore, SongsStore};
use crate::theme::{SCREEN_PAD, T_BODY, T_CHART, T_DETAIL_TITLE, T_META, T_META_SMALL};
use crate::ui::{AttachmentThumb, ConfidenceDots, IconButton, MetaChip, icon};

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

    let Some(song) = songs.get(id) else {
        return rsx! {
            div { style: {format!("padding: {SCREEN_PAD}; {T_META}")}, "This song is gone." }
        };
    };

    let chips = metadata_chips(&song);
    let set_count = setlists.containing(id).len();

    // Facts that follow the confidence word, each only when it exists.
    let status_tail = {
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
    };

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

                div { style: {format!("{T_DETAIL_TITLE}")}, {song.title.clone()} }
                div { style: {format!("{T_BODY} color: var(--sla-muted); margin-top: 5px;")}, {song.artist.clone()} }

                // Filled fields only — never an empty slot or a placeholder dash.
                div { style: "display: flex; flex-wrap: wrap; gap: 7px; margin-top: 13px;",
                    for chip in chips.clone() {
                        MetaChip { key: {chip.0.clone()}, label: {chip.0.clone()}, is_key: {chip.1} }
                    }
                }

                // Status line: confidence, dots, then the facts that follow from it.
                div {
                    style: "display: flex; align-items: center; gap: 8px; margin-top: 16px; \
                            padding-top: 13px; border-top: 1px solid var(--sla-hairline);",
                    span {
                        style: "font-weight: 600; font-size: 13px; color: var(--sla-ink-2);",
                        {song.confidence.map(|c| c.label()).unwrap_or("Unrated")}
                    }
                    ConfidenceDots { confidence: {song.confidence} }
                    span { style: {format!("{T_META}")}, {status_tail.clone()} }
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
                                        for (index, line, note) in preview_of_primary(songs, attachments, id) {
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
                                for (line_index, line, note) in preview_of_row(songs, attachments, id, index, expanded) {
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
) -> Vec<(usize, String, bool)> {
    match songs.get(id).and_then(|song| song.primary()) {
        Some(primary) => preview(attachments, primary),
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
) -> Vec<(usize, String, bool)> {
    let Some(attachment) = other_id(songs, attachments, id, index) else {
        return Vec::new();
    };
    if !expanded.get().contains(&attachment) {
        return Vec::new();
    }
    preview(attachments, attachment)
}

/// What one chart shows: `(index, text, is a note)`.
///
/// A note is the muted sentence that stands in when there is nothing to render
/// — which is most of Phase D's job still to come, and the reason this replaced
/// six grey bars. A skeleton says "loading"; three of the six lines here would
/// have been a lie about a PDF that has never been rasterised.
///
/// This is the one place in the app that reads an attachment body, and it costs
/// one object read per redraw of one card. The performance budget's rule is
/// about *rows* — the library builds three hundred of them and must not touch a
/// body for any — and the row list still cannot: `AttachmentsStore::items`
/// never carries one.
fn preview(attachments: AttachmentsStore, id: AttachmentId) -> Vec<(usize, String, bool)> {
    let Some(attachment) = attachments.get(id) else {
        return Vec::new();
    };
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

    if let Some(note) = note(&attachment, lines.len(), all.len()) {
        lines.push((lines.len(), note, true));
    }
    lines
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
            // D4 rasterises a PDF's pages to PNGs beside it on import; until
            // then the app has the file and no way to look inside it.
            AttachmentKind::Pdf => "No page preview yet.",
            // E2 writes `page.html` into the directory and extracts the text
            // beside it. A capture made before that lands has the page and no
            // extract, and rendering the page itself is E5.
            AttachmentKind::CapturedPage => "Saved on this device. No preview yet.",
        }
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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

        assert!(preview_of_row(songs, attachments, id, 0, expanded).is_empty());

        toggle_expanded(songs, attachments, id, 0, expanded);

        let lines = preview_of_row(songs, attachments, id, 0, expanded);
        assert_eq!(lines[0].1, "Blue jean baby");
        assert_eq!(
            songs.get(id).unwrap().primary_attachment,
            primary_before,
            "expanding is view state and writes nothing"
        );
        assert!(other_rows(songs, attachments, id, expanded)[0].2, "the chevron flips");

        // And it closes again.
        toggle_expanded(songs, attachments, id, 0, expanded);
        assert!(preview_of_row(songs, attachments, id, 0, expanded).is_empty());
    }

    #[test]
    fn a_typed_chart_renders_its_own_text() {
        let (songs, attachments, id) = library(vec![text(Some("G       D
Carolina"))]);
        let lines = preview_of_primary(songs, attachments, id);
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
        let lines = preview_of_primary(songs, attachments, id);
        assert_eq!(lines.len(), PREVIEW_LINES + 1);
        assert_eq!(lines[PREVIEW_LINES], (PREVIEW_LINES, "+12 more lines".into(), true));
    }

    #[test]
    fn a_blank_line_between_verses_survives_but_a_leading_one_does_not() {
        let (songs, attachments, id) = library(vec![text(Some("

verse one

verse two"))]);
        let lines = preview_of_primary(songs, attachments, id);
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
            (AttachmentKind::CapturedPage, "Saved on this device. No preview yet."),
            (AttachmentKind::Text, "Nothing typed yet."),
        ] {
            let (songs, attachments, id) = library(vec![of_kind(kind)]);
            let lines = preview_of_primary(songs, attachments, id);
            assert_eq!(lines.len(), 1, "one note, and no skeleton bars");
            assert_eq!(lines[0].1, expected);
            assert!(lines[0].2, "and it is styled as a note, not as a chart");
        }
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
}

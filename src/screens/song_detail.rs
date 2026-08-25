//! Song detail — HI-FI.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::model::{Attachment, Song, SongId, fmt_duration};
use crate::store::{AttachmentsStore, NavStore, SetlistsStore, SongsStore};
use crate::theme::{SCREEN_PAD, T_BODY, T_DETAIL_TITLE, T_META, T_META_SMALL};
use crate::ui::{ConfidenceDots, IconButton, MetaChip, icon};

#[component]
pub fn SongDetail(id: Option<SongId>) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();
    let setlists = use_store::<SetlistsStore>();
    let attachments = use_store::<AttachmentsStore>();

    let id = id.unwrap_or_default();

    let Some(song) = songs.get(id) else {
        return rsx! {
            div { style: {format!("padding: {SCREEN_PAD}; {T_META}")}, "This song is gone." }
        };
    };

    let chips = metadata_chips(&song);
    let all = attachments.many(&song.attachments);
    let primary = song
        .primary_attachment
        .and_then(|pid| all.iter().find(|a| a.id == pid).cloned())
        .or_else(|| all.first().cloned());
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
                IconButton { glyph: TablerIcon::Pencil, size: 17, onclick: move || {} }
                IconButton { glyph: TablerIcon::DotsVertical, size: 17, onclick: move || {} }
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

                // Primary attachment card.
                {primary_card(__scope, primary.clone())}

                // Other attachments, collapsed. Expanding one inlines it; it
                // does not become primary.
                for att in other_attachments(attachments, songs, id) {
                    div {
                        key: {att.id},
                        style: "display: flex; align-items: center; gap: 8px; padding: 12px 0; \
                                border-bottom: 1px solid var(--sla-hairline-soft);",
                        span { style: "font-weight: 500; font-size: 15px; flex: 1;",
                            {format!("{} · {}", att.title, att.kind.descriptor())}
                        }
                        span { style: "color: var(--sla-muted); display: flex;",
                            {icon(__scope, TablerIcon::ChevronDown, 17)}
                        }
                    }
                }

                div {
                    style: "color: var(--sla-accent); font-weight: 600; font-size: 15px; padding: 14px 0 22px;",
                    "+ Add attachment"
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

/// The card the primary attachment gets: filename, page count, a preview of
/// the content, and the tap target for the full-screen viewer. Empty when the
/// song has no attachment yet.
#[component]
fn primary_card(att: Option<Attachment>) -> NodeHandle {
    let Some(att) = att else {
        return rsx! { div {} };
    };
    let subtitle = match att.page_count {
        Some(n) => format!("primary · {n} pages"),
        None => "primary".to_string(),
    };

    rsx! {
        div {
            style: "background: var(--sla-card); border-radius: 16px; padding: 16px 18px; \
                    margin-top: 14px; box-shadow: var(--sla-card-shadow);",
            div { style: "display: flex; align-items: center; gap: 8px;",
                div { style: "flex: 1; min-width: 0;",
                    div { style: "font-weight: 600; font-size: 15px;", {att.title.clone()} }
                    div { style: {format!("{T_META_SMALL} margin-top: 2px;")}, {subtitle.clone()} }
                }
                span { style: "color: var(--sla-accent); display: flex;",
                    {icon(__scope, TablerIcon::Maximize, 18)}
                }
            }

            // Content preview. A real attachment renders its own body here;
            // skeleton bars stand in until the viewer exists.
            div { style: "margin-top: 14px; display: flex; flex-direction: column; gap: 8px;",
                for i in 0u8..6 {
                    div {
                        key: i,
                        style: {
                            let w = 100 - (i as u32 % 3) * 14;
                            format!("height: 8px; border-radius: 4px; width: {w}%; background: var(--sla-skeleton);")
                        },
                    }
                }
            }

            div { style: {format!("{T_META_SMALL} margin-top: 14px;")}, "Tap to open full screen" }
        }
    }
}

/// Everything except the primary attachment, recomputed from the stores so
/// the `for` loop captures only `Copy` values.
fn other_attachments(
    attachments: AttachmentsStore,
    songs: SongsStore,
    id: SongId,
) -> Vec<Attachment> {
    let Some(song) = songs.get(id) else {
        return Vec::new();
    };
    let primary = song
        .primary_attachment
        .or_else(|| song.attachments.first().copied());
    attachments
        .many(&song.attachments)
        .into_iter()
        .filter(|a| Some(a.id) != primary)
        .collect()
}

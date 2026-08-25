//! Shared pieces used across screens: icons, chips, confidence dots, rows.

use rinch::prelude::*;
use rinch_tabler_icons::{TablerIcon, TablerIconOptions, TablerIconStyle, render_tabler_icon_with_options};

use crate::model::{AttachmentKind, Confidence, Song};
use crate::theme::{T_CHIP, T_LABEL_CAPS, T_META, T_META_SMALL, T_ROW_TITLE};

/// Tabler icon at an explicit size. Stroke 1.8–1.9 for outline icons, per the
/// handoff; 24px glyphs inside the FAB, 21px in nav, 16–19px in buttons.
pub fn icon(scope: &mut RenderScope, glyph: TablerIcon, size: u32) -> NodeHandle {
    render_tabler_icon_with_options(
        scope,
        glyph,
        TablerIconOptions {
            style: TablerIconStyle::Outline,
            class: None,
            size: Some(size),
            stroke_width: Some(1.85),
        },
    )
}

/// A 40×40 circular icon button on `fill`. Nothing below 44 gets a touch
/// target this small — the 40 box sits inside a padded row.
#[component]
pub fn IconButton(glyph: Option<TablerIcon>, size: Option<u32>, onclick: Option<Callback>) -> NodeHandle {
    // The macro builds props with `..Default::default()`, so prop types that
    // have no Default — icons, callbacks, numbers — arrive as Options.
    let glyph = glyph.unwrap_or(TablerIcon::Point);
    let size = size.unwrap_or(18);
    rsx! {
        div {
            onclick: move || { if let Some(cb) = &onclick { cb.invoke() } },
            style: "width: 40px; height: 40px; border-radius: 999px; background: var(--sla-fill); \
                    display: flex; align-items: center; justify-content: center; \
                    color: var(--sla-ink-2); flex-shrink: 0;",
            {icon(__scope, glyph, size)}
        }
    }
}

/// A filter/sort chip. `active` uses ink-on-paper; `selected` is the softer
/// fill state used for the current sort field.
#[component]
pub fn Chip(label: String, active: bool, selected: bool, onclick: Option<Callback>) -> NodeHandle {
    let colors = if active {
        "background: var(--sla-ink); color: var(--sla-paper);"
    } else if selected {
        "background: var(--sla-fill); color: var(--sla-ink-2);"
    } else {
        "background: var(--sla-fill); color: var(--sla-muted);"
    };

    rsx! {
        div {
            onclick: move || { if let Some(cb) = &onclick { cb.invoke() } },
            style: {format!("{T_CHIP} {colors} border-radius: 999px; padding: 6px 12px; white-space: nowrap;")},
            {label.clone()}
        }
    }
}

/// A metadata chip on song detail. The key chip is the only tinted one.
#[component]
pub fn MetaChip(label: String, is_key: bool) -> NodeHandle {
    let colors = if is_key {
        "background: var(--sla-accent-tint); color: var(--sla-accent-on-tint);"
    } else {
        "background: var(--sla-fill); color: var(--sla-ink-2);"
    };

    rsx! {
        div {
            style: {format!("{T_CHIP} {colors} border-radius: 8px; padding: 6px 11px;")},
            {label.clone()}
        }
    }
}

/// Three 6px dots: filled accent for solid, dimmed accent for anything
/// shakier, hairline for the rest.
#[component]
pub fn ConfidenceDots(confidence: Option<Confidence>) -> NodeHandle {
    let filled = confidence.map(|c| c.dots()).unwrap_or(0);
    let color = confidence
        .map(|c| c.dot_color())
        .unwrap_or("var(--sla-hairline)");

    rsx! {
        div { style: "display: flex; gap: 4px; align-items: center; flex-shrink: 0;",
            for i in 0u8..3 {
                div {
                    key: i,
                    style: {
                        let fill = if i < filled { color } else { "var(--sla-hairline)" };
                        format!("width: 6px; height: 6px; border-radius: 999px; background: {fill};")
                    },
                }
            }
        }
    }
}

/// The 38×38 attachment thumb. A song with no attachment gets a dashed hole,
/// not a label.
#[component]
pub fn AttachmentThumb(kind: Option<AttachmentKind>) -> NodeHandle {
    rsx! {
        div {
            style: {
                match kind {
                    Some(_) => "width: 38px; height: 38px; border-radius: 10px; background: var(--sla-fill); \
                                display: flex; align-items: center; justify-content: center; flex-shrink: 0;"
                        .to_string(),
                    None => "width: 38px; height: 38px; border-radius: 10px; background: var(--sla-fill-2); \
                             border: 1px dashed #DDD3C4; flex-shrink: 0;"
                        .to_string(),
                }
            },
            if let Some(k) = kind {
                span {
                    style: "font-size: 9px; font-weight: 600; letter-spacing: 0.06em; \
                            text-transform: uppercase; color: var(--sla-muted);",
                    {k.badge()}
                }
            }
        }
    }
}

/// A group header: label, count, then a hairline filling the rest of the row.
/// The first group is accent-coloured; later ones are muted.
#[component]
pub fn GroupHeader(
    label: String,
    count: Option<usize>,
    is_first: bool,
    collapsed: bool,
    onclick: Option<Callback>,
) -> NodeHandle {
    let count = count.unwrap_or(0);
    let label_color = if is_first {
        "var(--sla-accent)"
    } else {
        "var(--sla-muted)"
    };

    rsx! {
        div {
            onclick: move || { if let Some(cb) = &onclick { cb.invoke() } },
            style: "display: flex; align-items: center; gap: 8px; padding: 16px 0 8px;",
            span {
                style: {format!("{T_LABEL_CAPS} color: {label_color};")},
                {label.clone()}
            }
            span {
                style: {format!("{T_META_SMALL}")},
                {format!("{count}")}
            }
            if collapsed {
                span { style: {format!("{T_META_SMALL}")}, "collapsed" }
            }
            div { style: "flex: 1; height: 1px; background: var(--sla-hairline);" }
        }
    }
}

/// A library row. Comfortable is thumb + title + meta + dots; compact is one
/// line, title left and `Artist · key` right — roughly double the rows per
/// screen for a 300-song library.
#[component]
pub fn SongRow(
    song: Song,
    kind: Option<AttachmentKind>,
    compact: bool,
    onclick: Option<Callback>,
) -> NodeHandle {
    // One line, title left and `Artist · key` right — roughly double the rows
    // per screen for a 300-song library.
    let compact_tail = {
        let mut tail = song.artist.clone();
        if let Some(k) = &song.key {
            tail.push_str(" · ");
            tail.push_str(k);
        }
        tail
    };
    let meta_line = if song.key.is_none() && song.tempo.is_none() {
        song.meta_line_played()
    } else {
        song.meta_line()
    };

    if compact {
        return rsx! {
            div {
                onclick: move || { if let Some(cb) = &onclick { cb.invoke() } },
                style: "display: flex; align-items: baseline; gap: 10px; padding: 5px 0; \
                        border-bottom: 1px solid var(--sla-hairline-soft);",
                span {
                    style: {format!("{T_ROW_TITLE} font-size: 16px; flex: 1; \
                                     overflow: hidden; text-overflow: ellipsis; white-space: nowrap;")},
                    {song.title.clone()}
                }
                span { style: {format!("{T_META_SMALL}")}, {compact_tail.clone()} }
            }
        };
    }

    rsx! {
        div {
            onclick: move || { if let Some(cb) = &onclick { cb.invoke() } },
            style: "display: flex; align-items: center; gap: 13px; padding: 11px 0; \
                    border-bottom: 1px solid var(--sla-hairline-soft);",
            AttachmentThumb { kind: kind }
            div { style: "flex: 1; min-width: 0;",
                div {
                    style: {format!("{T_ROW_TITLE} overflow: hidden; text-overflow: ellipsis; white-space: nowrap;")},
                    {song.title.clone()}
                }
                div { style: {format!("{T_META} margin-top: 2px;")}, {meta_line.clone()} }
            }
            ConfidenceDots { confidence: song.confidence }
        }
    }
}

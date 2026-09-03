//! Shared pieces used across screens: icons, chips, confidence dots, rows.

use rinch::prelude::*;
use rinch_tabler_icons::{TablerIcon, TablerIconOptions, TablerIconStyle, render_tabler_icon_with_options};

use crate::model::{AttachmentKind, Confidence, Song};
use crate::theme::{SCREEN_PAD, T_CHIP, T_LABEL_CAPS, T_META, T_META_SMALL, T_ROW_TITLE};

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
/// fill state used for the current sort field. `glyph` is an optional
/// trailing icon — e.g. the sort direction arrow — drawn as a path so it
/// reads as part of the label rather than a second element.
#[component]
pub fn Chip(
    label: String,
    glyph: Option<TablerIcon>,
    active: bool,
    selected: bool,
    onclick: Option<Callback>,
) -> NodeHandle {
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
            style: {format!("{T_CHIP} {colors} border-radius: 999px; padding: 6px 12px; \
                             white-space: nowrap; display: flex; align-items: center; gap: 4px;")},
            {label.clone()}
            if let Some(g) = glyph {
                {icon(__scope, g, 14)}
            }
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

/// The group header label's font-size for a density.
///
/// The handoff commits to a literal number for the compact case — "group
/// headers shrink to 14px" (`design_handoff_setlistarray/README.md:154`) — and
/// that number is lifted straight from wireframe `2b`, which draws a 16px
/// comfortable header shrinking to 14 in Patrick Hand. This app's actual hi-fi
/// header was never 16px, though: it is [`T_LABEL_CAPS`], a small-caps,
/// letter-spaced label already sitting at 12px, because label-caps is a
/// different typographic move than a plain 16px word and the hi-fi pass chose
/// it over the wireframe's literal size the same way it chose real tokens over
/// every other raw pixel value the wireframe drew. Applying the handoff's 14
/// here verbatim would make the compact header *larger* than the comfortable
/// one — the opposite of "shrink," and a header that visibly grows when you
/// turn compact rows on is a worse bug than a card that quotes a number this
/// function does not use. So this keeps the wireframe's *direction* — compact
/// strictly smaller than comfortable — over its digit: comfortable stays at
/// `T_LABEL_CAPS`'s own 12, and compact drops one step to 11, still legible at
/// this weight and tracking.
pub fn group_header_font_size(compact: bool) -> u32 {
    if compact { 11 } else { 12 }
}

/// A group header: label, count, then a hairline filling the rest of the row.
/// The first group is accent-coloured; later ones are muted. `compact` shrinks
/// the label — see [`group_header_font_size`] for why it lands on 11px rather
/// than the handoff's literal 14.
///
/// A plain function taking `scope: &mut RenderScope` rather than a PascalCase
/// `#[component]`, and that is a deliberate downgrade made for card H4's
/// alphabet scrubber, not a style preference. `#[component]` turns a
/// PascalCase function into a struct + `Component::render` pair invoked only
/// through `Name { props }` DSL syntax, and every call written that way —
/// reactive props especially — is spliced straight into its parent by the
/// macro's own codegen (`rinch-macros/src/dom_codegen/component_codegen.rs`,
/// `generate_reactive_component_stmt`) with no `NodeHandle` ever handed back
/// to the code that wrote it. That was fine while nothing needed to remember
/// which header belonged to which letter; it stopped being fine the moment
/// the scrubber needed exactly that. A plain function called as an ordinary
/// Rust expression — the same shape `settings.rs`'s `section`, `reading_row`
/// and `link_row` already use — returns its `NodeHandle` like any other
/// function return value, so `library.rs` can keep a clone of it in the map
/// the rail's taps read. `GroupHeader` had exactly one call site, so
/// converting it in place cost nothing; a component with call sites that
/// still needed the DSL form would keep both, the way `MetaChip` and this one
/// used to coexist.
pub fn group_header(
    scope: &mut RenderScope,
    label: String,
    count: usize,
    is_first: bool,
    collapsed: bool,
    compact: bool,
    onclick: impl Fn() + 'static,
) -> NodeHandle {
    let label_color = if is_first {
        "var(--sla-accent)"
    } else {
        "var(--sla-muted)"
    };
    let label_size = group_header_font_size(compact);
    let __scope = scope;

    rsx! {
        div {
            onclick: move || onclick(),
            style: "display: flex; align-items: center; gap: 8px; padding: 16px 0 8px;",
            span {
                style: {format!("{T_LABEL_CAPS} color: {label_color}; font-size: {label_size}px;")},
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

// ---------------------------------------------------------------------------
// Bottom sheets (`2e`, `2c`)
// ---------------------------------------------------------------------------
//
// Hand-rolled rather than Rinch's `Drawer`. `Drawer`'s bottom sizes are five
// fixed pixel buckets with no percentage option and no `style`/`class`/radius
// prop to override, so "~57% height, 18px top corners" is unsayable; it paints
// `--rinch-color-body` and its own header, not `var(--sla-*)`; its slide is
// 300ms `ease` where the handoff asks for 200–250ms Android ease-out; and it
// flips `display: none` on the root in the same frame as the transform class,
// so the slide never actually runs. `Modal` is a centred dialog on fixed pixel
// widths — a different shape entirely.
//
// The panel stays mounted and translated off-screen instead of being unmounted,
// because a node that appears already at its resting place has nothing to
// animate from. The closed offset is in pixels, not `100%`, because rinch's
// transition engine drops percentage translations when it interpolates a
// transform (`rinch-dom/src/transition/apply.rs`) — a percentage slide snaps.

/// A standard Android ease-out (the deceleration curve), at the fast end of
/// the handoff's 200–250ms.
const SHEET_EASE: &str = "220ms cubic-bezier(0, 0, 0.2, 1)";

/// Far enough below the window that the tallest sheet is fully clear of it.
const SHEET_PARKED: &str = "700px";

/// The full-bleed layer a sheet lives in. Transparent to taps while closed so
/// the screen underneath stays live.
pub fn sheet_root_style(open: bool) -> String {
    let taps = if open { "auto" } else { "none" };
    format!(
        "position: absolute; left: 0; top: 0; right: 0; bottom: 0; z-index: 40; \
         pointer-events: {taps};"
    )
}

/// The scrim over the screen behind. Fades with the slide.
pub fn sheet_scrim_style(open: bool) -> String {
    let opacity = if open { "1" } else { "0" };
    format!(
        "position: absolute; left: 0; top: 0; right: 0; bottom: 0; \
         background: rgba(28, 25, 23, 0.38); opacity: {opacity}; \
         transition: opacity {SHEET_EASE};"
    )
}

/// The sheet itself: `height_pct` of the window, 18px top corners, parked
/// below the fold until it is opened.
pub fn sheet_panel_style(open: bool, height_pct: u32) -> String {
    let y = if open { "0px" } else { SHEET_PARKED };
    format!(
        "position: absolute; left: 0; right: 0; bottom: 0; height: {height_pct}%; \
         background: var(--sla-paper); border-radius: 18px 18px 0 0; \
         border-top: 1px solid var(--sla-hairline); \
         display: flex; flex-direction: column; min-height: 0; \
         box-shadow: 0 -8px 28px -12px rgba(28, 25, 23, 0.35); \
         transform: translateY({y}); transition: transform {SHEET_EASE};"
    )
}

// ---------------------------------------------------------------------------
// Group collapse (card J1)
// ---------------------------------------------------------------------------
//
// The same transition engine the sheets use has a second gap that matters
// here and never did there: `rinch-dom/src/transition/diff.rs`'s
// `diff_dimension` only emits a `PropertyChange` for `height` when *both*
// the old and the new computed value are already a concrete pixel `Length`
// — `(DimensionValue::Length(a), DimensionValue::Length(b))` is the only arm
// that matches. `Auto` on either side (an unconstrained, "just flow"
// height, which is what every element has until something says otherwise)
// falls through to the wildcard and is skipped, so a height that starts or
// ends at `auto` does not animate — it snaps, same as the sheets' percentage
// translate did before `SHEET_PARKED` moved to pixels. `max-height` is not
// in the picture at all: it is not one of the `TransitionProperty` variants
// `types.rs` defines, so there is no fallback to reach for there either.
//
// So a group's rows can only animate shut and open between two pixel
// numbers it already has on record — never from `auto`. `crate::screens::
// library`'s `group_rows` measures that number the ordinary way: it reads
// `NodeHandle::scroll_height()` on the rows wrapper, which is a live query
// against whatever layout last resolved for that node, not a snapshot taken
// when the handle was built — so it is accurate however long ago that
// layout happened, including "the last time this exact group was open,
// several toggles ago." `library.rs`'s own comment on its `group_heights`
// map has the one consequence that leaves unsolved: the very first close of
// a group that has never been measured has nothing to interpolate the
// closing edge away from, so that one transition snaps; every open and
// close after it, once a height is on record, animates.
pub fn group_rows_style(collapsed: bool, height_px: f32) -> String {
    let target = if collapsed { 0.0 } else { height_px };
    format!("overflow: hidden; height: {target}px; transition: height {SHEET_EASE};")
}

/// The grab handle every sheet wears.
#[component]
pub fn SheetHandle() -> NodeHandle {
    rsx! {
        div {
            style: "display: flex; justify-content: center; padding: 9px 0 5px; flex-shrink: 0;",
            div { style: "width: 44px; height: 4px; border-radius: 999px; background: var(--sla-hairline);" }
        }
    }
}

/// The footer both sheets share: a muted note on the left, one accent button
/// on the right.
#[component]
pub fn SheetFooter(note: String, action: String, enabled: bool, onclick: Option<Callback>) -> NodeHandle {
    let button = if enabled {
        "background: var(--sla-accent); color: var(--sla-on-accent);"
    } else {
        "background: var(--sla-fill); color: var(--sla-muted);"
    };

    rsx! {
        div {
            style: {format!("display: flex; align-items: center; gap: 12px; flex-shrink: 0; \
                             padding: 12px {SCREEN_PAD} 24px; border-top: 1px solid var(--sla-hairline);")},
            span { style: {format!("{T_META} flex: 1;")}, {note.clone()} }
            div {
                onclick: move || { if let Some(cb) = &onclick { cb.invoke() } },
                style: {format!("{button} border-radius: 999px; padding: 13px 24px; \
                                 font-weight: 600; font-size: 16px; white-space: nowrap;")},
                {action.clone()}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Card H4: the handoff's own words say group headers "shrink" in compact
    /// density, and this is the one number that check has to hold — not the
    /// handoff's literal 14, which [`group_header_font_size`]'s doc comment
    /// explains is a wireframe pixel value measured against a comfortable
    /// header this app never built. What must stay true regardless of which
    /// digit either side ends up on is the direction: compact is smaller than
    /// comfortable, full stop.
    #[test]
    fn group_header_font_size_is_smaller_in_compact_than_comfortable() {
        let comfortable = group_header_font_size(false);
        let compact = group_header_font_size(true);
        assert!(
            compact < comfortable,
            "compact header ({compact}px) must be smaller than comfortable ({comfortable}px)"
        );
        assert_eq!(comfortable, 12, "comfortable header stays at T_LABEL_CAPS's own size");
        assert_eq!(compact, 11);
    }

    // ── J1: the collapse wrapper ────────────────────────────────────────

    /// A collapsed group is zero-height and clipped, and an open one stands
    /// at whatever the last measurement said. Both carry the transition, and
    /// both must: a wrapper that only declares it in one state has nothing to
    /// interpolate *from* on the way back, which is the whole shape of the
    /// `Auto`-on-either-side gap `group_rows_style`'s own comment describes.
    #[test]
    fn a_collapsed_group_is_zero_height_and_an_open_one_is_its_measured_height() {
        let open = group_rows_style(false, 412.0);
        assert!(open.contains("height: 412px"), "{open}");
        assert!(open.contains("transition: height"), "{open}");

        let shut = group_rows_style(true, 412.0);
        assert!(shut.contains("height: 0px"), "{shut}");
        assert!(shut.contains("transition: height"), "{shut}");
    }

    /// The measurement is ignored while collapsed rather than negated or
    /// carried through — a collapsed group is 0px whatever it last measured,
    /// so a stale or absent measurement can never leave rows visible in a
    /// group the user shut.
    #[test]
    fn a_collapsed_group_is_zero_height_whatever_it_last_measured() {
        for measured in [0.0, 1.0, 412.0, 99_999.0] {
            let shut = group_rows_style(true, measured);
            assert!(shut.contains("height: 0px"), "measured {measured}: {shut}");
        }
    }

    /// It clips. Without `overflow: hidden` the rows inside a 0px box are
    /// still painted, so the "collapsed" group would animate to no height and
    /// go on showing its contents over whatever followed it.
    #[test]
    fn the_collapse_wrapper_clips_what_it_is_shrinking() {
        assert!(group_rows_style(true, 412.0).contains("overflow: hidden"));
        assert!(group_rows_style(false, 412.0).contains("overflow: hidden"));
    }
}

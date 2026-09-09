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
///
/// ## The label is a `span` with its own `color`, and card K8 is why
///
/// It was a bare text node under the chip's own `div` until this card, taking
/// `color` by inheritance the way the whole file does. That is F3's fault
/// exactly — *"a bare text node under a restyled parent does not re-resolve an
/// inherited `color`"*, which `screens::settings`'s header states as a rule and
/// which every control on that screen already obeys — and it was invisible here
/// for a year for one reason: nothing could change `--sla-ink` while a chip was
/// on screen. The only way to flip the theme was the switch in Settings, and
/// leaving Settings remounts the library, which rebuilds the chip from nothing.
///
/// K8 made the theme flip underneath whatever is on screen: the system goes
/// dark at sunset and the library repaints where it stands. Measured on the
/// moto g stylus 5G on 2026-09-09, `adb shell cmd uimode night yes` with the
/// library open, then the same screen reached by a cold launch, differ in
/// exactly three places — the labels of "Group: Confidence", "Confidence ↓" and
/// "Filter", and nothing else in the whole capture. The pill behind the active
/// one had repainted to the dark theme's near-white `ink`; the word on it had
/// not, so it was near-white on near-white and simply gone.
///
/// The glyph needs no such wrapper and did not move in that diff: an icon is an
/// element with a style of its own, and it is the *element* that re-resolves.
/// So `color` stays on the outer `div` for it, and the one thing that was a
/// bare text node stops being one.
#[component]
pub fn Chip(
    label: String,
    glyph: Option<TablerIcon>,
    active: bool,
    selected: bool,
    onclick: Option<Callback>,
) -> NodeHandle {
    let (background, ink) = if active {
        ("var(--sla-ink)", "var(--sla-paper)")
    } else if selected {
        ("var(--sla-fill)", "var(--sla-ink-2)")
    } else {
        ("var(--sla-fill)", "var(--sla-muted)")
    };

    rsx! {
        div {
            onclick: move || { if let Some(cb) = &onclick { cb.invoke() } },
            style: {format!("{T_CHIP} background: {background}; color: {ink}; \
                             border-radius: 999px; padding: 6px 12px; \
                             white-space: nowrap; display: flex; align-items: center; gap: 4px;")},
            span { style: {format!("color: {ink};")}, {label.clone()} }
            if let Some(g) = glyph {
                {icon(__scope, g, 14)}
            }
        }
    }
}

/// A metadata chip on song detail. The key chip is the only tinted one.
///
/// The label is a `span` carrying its own `color` for the reason [`Chip`]'s
/// doc comment gives at length: a bare text node does not re-resolve an
/// inherited colour when the token underneath it changes, which nothing could
/// make happen mid-screen until card K8 let the system flip the theme where
/// the app stands. Same construct, same fault, fixed the same way rather than
/// waiting to be found a second time on a different screen.
#[component]
pub fn MetaChip(label: String, is_key: bool) -> NodeHandle {
    let (background, ink) = if is_key {
        ("var(--sla-accent-tint)", "var(--sla-accent-on-tint)")
    } else {
        ("var(--sla-fill)", "var(--sla-ink-2)")
    };

    rsx! {
        div {
            style: {format!("{T_CHIP} background: {background}; color: {ink}; \
                             border-radius: 8px; padding: 6px 11px;")},
            span { style: {format!("color: {ink};")}, {label.clone()} }
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
/// `collapsed_fn` is a closure, not a plain `bool`, and that is card K51's
/// fix, not a style choice. Every call site used to hand this a `bool` read
/// once, outside any reactive closure, while the `for` loop in [`Library`]
/// built the header — so the `if collapsed { … }` below auto-wrapped by the
/// rsx macro (`generate_if_block`, `move || { #condition }`) closed over a
/// *frozen* value rather than a live signal read. Writing
/// `LibraryViewStore::collapsed` through `toggle_collapsed` therefore
/// invalidated nothing this header subscribed to: the badge, and the
/// group's row height beside it, sat exactly as they were until some
/// unrelated change (adding a song, re-sorting) rebuilt the whole group from
/// scratch and read the signal fresh. A tap looked like it did nothing.
/// Taking a closure and calling it *inside* the `if` — so the condition
/// itself performs the `.get()` — makes the auto-wrapped closure a genuine
/// subscription, and `show_dom` updates just this badge when the signal
/// changes, with no help from anything else on screen.
pub fn group_header(
    scope: &mut RenderScope,
    label: String,
    count: usize,
    is_first: bool,
    collapsed_fn: impl Fn() -> bool + 'static,
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
            if collapsed_fn() {
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
// Group collapse (card J1, removed by card K51)
// ---------------------------------------------------------------------------
//
// This block used to hold `group_rows_style`, which animated a library
// group's rows shut and open by transitioning a wrapper's `height` between
// two measured pixel values — `rinch-dom/src/transition/diff.rs`'s
// `diff_dimension` only emits a `PropertyChange` for `height` when *both*
// the old and new computed value are already a concrete pixel `Length`
// (`Auto` on either side, which is what every element starts at, falls
// through to the wildcard and is skipped), so `crate::screens::library`
// measured each group's open height with `NodeHandle::scroll_height()` the
// first time it was seen and interpolated between that and zero on every
// toggle after. That much worked, and animated correctly on the device.
//
// What card J1 could not see from a diff, and what verifying K51's actual
// fix — that tapping a header repaints at all — turned up on the device, is
// that the transition engine only half-implements a `height` change:
// `rinch-dom/src/transition/apply.rs` updates the node's `computed_style`
// for paint every frame of the animation, but it never pushes the
// interpolated value into Taffy and never marks the node's layout dirty.
// Paint and layout only agree by accident, at the two ends of a transition
// that both happen to be values Taffy already had on record from some
// earlier real layout pass — which is exactly true the first time a group
// closes (Taffy laid it out open at its natural height a moment ago, and
// "0px, clipped" needs no new layout to look right) and exactly false the
// first time that same group reopens: the animation walks `computed_style`
// back up from 0 toward the remembered height for *paint*, but the node's
// actual layout box, as far as Taffy is concerned, never left 0. The rows
// were correctly back in the DOM, correctly re-mounted, and permanently
// invisible, because the box holding them had a resolved height of zero
// that nothing in this animation ever asked Taffy to change.
//
// That is not a bug in how this screen drove the engine; it is a hole in
// the engine itself, on the layout side rather than the paint side of the
// two `../rinch-fixes` (`8526ce6`) already fixed there — a zero-sized box
// painting children regardless of `overflow`, and a degenerate clip treated
// as no clip, both found by exercising this same animation and both worth
// keeping upstream whether or not this app still animates anything. This
// one has no fix yet, upstream or local, so card K51 took the animation
// back out rather than ship a group that reopens looking broken. See
// [`crate::screens::library::group_entry`]'s doc comment for where the
// mount/unmount code that replaced it lives, and this file's git history
// for `group_rows_style` itself if the shape of the pixel math is ever
// worth reading again.
//
// The two sheets below are still the only things in this app that animate,
// and they get to keep doing it for a reason this bug makes concrete rather
// than aesthetic: a sheet's panel stays mounted, translated off-screen,
// for exactly as long as it exists — `sheet_panel_style` never asks
// anything to unmount and remount across the transition, so there is never
// a moment where paint and layout are allowed to describe two different
// boxes. A collapsing library group cannot make that same promise and
// still be a list that scrolls, which is the whole reason mounting and
// unmounting it outright, with no animation, is correct here and would not
// be a downgrade even if the engine grew a fix for this tomorrow.

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

    // ── J1: the collapse wrapper (removed by K51) ───────────────────────
    //
    // Three tests lived here, on `group_rows_style`: that a collapsed group
    // was zero-height and an open one stood at its last measured height,
    // that a stale measurement couldn't leave a collapsed group's rows
    // visible, and that the wrapper clipped what it was shrinking. All
    // three were correct descriptions of that function right up until the
    // device showed the function's whole approach was wrong — see the
    // "Group collapse" comment block above, where `group_rows_style` used
    // to live, for what verifying K51 actually found. A test asserting
    // behaviour of code that no longer exists is worse than no test: it
    // would not even compile, and fixing that by re-deriving three
    // assertions about pixel-height strings the app no longer produces
    // would only dress up dead reasoning as live coverage. Collapse itself
    // is exercised by mounting, not by a style string — there is nothing
    // left in this file for a unit test to check without a window to tap
    // in, which is exactly `crate::screens::library`'s own note on why the
    // density-chip test above reads source text instead of pixels.
}

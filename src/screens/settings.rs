//! Settings — WIREFRAME (`1q`), card H1, styled with the hi-fi token set.
//!
//! Grouped rows and no account section, because there is no account: the last
//! line on the screen is the whole product promise and it is checked, verbatim,
//! against the handoff by a test at the bottom of this file.
//!
//! Reached from the gear in *both* tab headers and from nowhere else — there is
//! no nav item for it, which the handoff is explicit about and
//! `crate::bottom_nav` has said in a comment since the day it was written. That
//! is the fact the back affordance here has to survive: ← cannot assume Songs.
//! It does not — [`NavStore::back`] lands on the *current tab's* root, and the
//! gear does not change the tab, so opening Settings from Setlists and pressing
//! ← returns to Setlists. Nothing on this screen needs to know which door it
//! came in by.
//!
//! ## What is built, and what is deliberately not
//!
//! `1q` draws thirteen rows. Several of them belong to cards that do not exist
//! yet, and a row that looks tappable and does nothing is worse than a row that
//! is not there — so the ones without an engine behind them are absent, and
//! absent *on the record* rather than by oversight. The screen's shape is a
//! decision, and this is where it is written down.
//!
//! **Built, because the state exists, persists and takes effect today:**
//!
//! | row | where the state lives |
//! | --- | --- |
//! | Attachments on device | `AttachmentsStore::total_bytes` |
//! | Saved webpages | a count over `AttachmentsStore::items` |
//! | Re-check saved pages | `SettingsStore::recheck_saved_pages` |
//! | Library sort | `LibraryViewStore::sort_field` / `sort_dir` |
//! | Library density | `LibraryViewStore::density` |
//! | Keep screen awake while playing | `SettingsStore::keep_awake` |
//! | Accent | `SettingsStore::accent` |
//! | Performance mode theme | `SettingsStore::performance_theme` |
//! | Dark mode | `SettingsStore::dark_mode` |
//!
//! **Left to the card that owns it:**
//!
//! * **Export library (.zip)** — card I1. There is nothing to export *to* yet.
//! * **Import from file** — card I2, which has a real decision in front of it
//!   (replace or merge) that a row here cannot make on its behalf.
//! * **Last export** — card I3. It is a date produced by I1; until I1 exists
//!   the row can only ever read "never", which is not a fact about the user's
//!   library but a fact about this app's progress.
//! * With all three gone the **Backup** heading has nothing under it, so the
//!   heading is gone too. An empty section is a promise of its own.
//! * **Default tuning** — **no card owns this, and one should.** There is no
//!   `default_tuning` anywhere in the app: `Preferences` has no field for it,
//!   `schema.rhype` has no line for it, and `song_form` prefills nothing. It is
//!   also not merely plumbing — `Song::tuning` is free text, so the row needs a
//!   decision about whether it offers a list (of what? the tunings already in
//!   the book? a fixed six?) or a text field, and that decision belongs to a
//!   card rather than to the last twenty lines of this one.
//! * The **`›` chevrons** on the two storage rows. `1q` draws both as gateways
//!   to a breakdown screen; no such screen exists and no card describes one, so
//!   the two rows are read-only here. The number is the whole of what they can
//!   honestly offer, and it is offered without pretending to lead anywhere.
//! * **"Dark mode follows system"** — the handoff asks for it and it cannot be
//!   built: nothing in this app or in Rinch can read the system's light/dark
//!   preference. `rinch-theme` has a `dark_mode` flag an app *sets*; there is no
//!   API that reports what Android's night mode is set to, and no `#[cfg]` here
//!   would help because the value does not exist to be read. So the row is a
//!   plain on/off switch over `SettingsStore::dark_mode`, which is what that
//!   signal has always been. This is the same shape as
//!   `AccentChoice::FromSystem`, whose own TODO records the same missing
//!   platform call, and it should land in the same card that lands that one.
//!
//! Two of these were checked rather than assumed, because the card asked and
//! the answers went opposite ways. **Library sort** is real: `LibraryViewStore`
//! has held `sort_field` and `sort_dir`, persisted, since the sort sheet (`2c`)
//! was built — so the row states them and opens that sheet, and there are not
//! two ways to change one setting. **Library density** is real too, and this
//! screen is the first thing that can reach it: `Density` is persisted, and
//! `library.rs` has been reading it to pick a compact row since it was written,
//! but `LibraryViewStore::toggle_density` had no caller at all. Card H4 is
//! *"compact density everywhere it applies, plus the alphabet scrubber rail"* —
//! the everywhere-else and the rail are still H4's; the switch that turns it on
//! is here, because the handoff says in as many words that density is "set in
//! Settings → Library density".
//!
//! ## The accent row is four swatches and not a door
//!
//! Card H2 owns the accent picker. The judgement this card had to make is what
//! goes here in the meantime, and the answer is: the control itself, inline.
//!
//! `SettingsStore::accent` already resolves through `AccentChoice::resolve`,
//! already persists, and already repaints the entire app the moment it changes —
//! `crate::app`'s root style closure reads `accent_resolved()`, so a tap here is
//! a live theme change with nothing left to wire. `theme::ACCENTS` holds exactly
//! four values. Four swatches fit on one row. A row that instead opened a screen
//! H2 has not built would be the dead row this card exists to avoid, and a row
//! that only *printed* the accent name would be strictly less than what the
//! state can already do.
//!
//! What H2 still owns is everything the four swatches cannot say: the names, the
//! previews, and above all `AccentChoice::FromSystem` — the wallpaper extraction
//! that resolves to Rust today because Rinch exposes no platform call for it.
//! `FromSystem` is deliberately **not** offered as a fifth swatch here: choosing
//! it would silently mean Rust, which is a control that lies. The row's right
//! edge names the colour actually on screen (`derive::accent_note`), so a fresh
//! install reads "Rust" and is telling the truth about the pixels.
//!
//! ## Every reactive control carries its own colour
//!
//! Card F3 found this on the phone on 2026-09-01: a toggle whose background and
//! text colour both lived on one reactive style repainted the pill and left the
//! label the colour it had been, because a bare text node under a restyled
//! parent does not re-resolve an inherited `color`. Every switch, chip and
//! swatch below therefore declares `color` in a closure of its own, on the
//! element that carries the ink, and never inherits it from a parent that is
//! itself changing. The switches and the choice chips are the two shapes on this
//! screen that change colour under a finger, and both obey it.
//!
//! `onclick` and nothing else — see `src/gesture_reachability.rs`. The whole row
//! is the tap target for a switch, rather than the 42px pill: this is a phone,
//! and the pill is a quarter the width of the thing it is drawn beside.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{
    accent_note, attachments_note, density_label, library_sort_note, on_off,
    performance_theme_label, saved_pages_note,
};
use crate::model::AttachmentKind;
use crate::store::{
    AccentChoice, AttachmentsStore, Density, LibraryViewStore, NavStore, PerformanceTheme,
    SettingsStore,
};
use crate::theme::{
    ACCENTS, SCREEN_PAD, T_BODY, T_CHIP, T_META, T_META_SMALL, T_SCREEN_TITLE, T_SECTION_CAPS,
};
use crate::ui::{IconButton, icon};

/// The last line on the screen, and the reason the screen has no account
/// section. Exact, and a `const` rather than a literal in the markup so the test
/// at the bottom of this file can hold it against the handoff word for word.
pub const PROMISE: &str = "SetListArray · works with no connection. Nothing is uploaded anywhere.";

/// One row: label on the left, whatever states or changes it on the right.
///
/// `hairline-soft` and not `hairline`, matching the library's own rows — a list
/// of settings is a list, and the heavier rule is what this app uses to separate
/// a bar from content rather than one row from the next.
const ROW: &str = "display: flex; align-items: center; gap: 12px; padding: 12px 0; \
    min-height: 44px; border-bottom: 1px solid var(--sla-hairline-soft);";

#[component]
pub fn Settings() -> NodeHandle {
    let nav = use_store::<NavStore>();
    let settings = use_store::<SettingsStore>();
    let view = use_store::<LibraryViewStore>();
    let attachments = use_store::<AttachmentsStore>();

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",

            // The same back row every secondary screen in this app wears, at the
            // same 18px inset. ← and nothing else: there is no action on this
            // screen that belongs in a top bar.
            div { style: "padding: 2px 18px 8px; display: flex; align-items: center;",
                IconButton { glyph: TablerIcon::ChevronLeft, size: 19, onclick: move || nav.back() }
            }

            div { style: {format!("flex: 1; min-height: 0; overflow-y: auto; padding: 0 {SCREEN_PAD} 20px;")},

                div { style: {format!("{T_SCREEN_TITLE}")}, "Settings" }

                {section(__scope, "Storage")}

                {reading_row(__scope, "Attachments on device", move || {
                    attachments_note(attachments.total_bytes())
                })}
                {reading_row(__scope, "Saved webpages", move || {
                    saved_pages_note(saved_pages(attachments))
                })}
                // The switch persists today; card E6 is the re-fetch-and-diff
                // engine that will read it. That order is deliberate and is the
                // bet card F3 made with the keep-awake toggle and won: the
                // preference is a real, stored, tested value either way, and E6
                // arrives needing a signal to read rather than a screen to
                // redesign. Said out loud here so it is a plan and not a lie.
                {switch_row(__scope, "Re-check saved pages",
                    move || settings.recheck_saved_pages.get(),
                    move || settings.set_recheck_saved_pages(!settings.recheck_saved_pages.get()))}

                {section(__scope, "Defaults")}

                // Opens the sort sheet (`2c`) rather than growing a second way
                // to set the same two signals. The sheet is mounted in
                // `crate::app` as a sibling of the route, above everything, so
                // it slides over this screen exactly as it does over the
                // library — which is the whole reason the sheets live out there.
                {link_row(__scope, "Library sort",
                    move || library_sort_note(view.sort_field.get(), view.sort_dir.get()),
                    move || nav.sort_sheet_open.set(true))}

                {choice_row(__scope, "Library density",
                    [
                        (Density::Comfortable, density_label(Density::Comfortable)),
                        (Density::Compact, density_label(Density::Compact)),
                    ],
                    move || view.density.get(),
                    // Through `toggle_density`, which is the store's own
                    // persisting path, rather than writing the signal here and
                    // leaving the Preferences row behind.
                    move |wanted| if view.density.get() != wanted { view.toggle_density() })}

                {switch_row(__scope, "Keep screen awake while playing",
                    move || settings.keep_awake.get(),
                    move || settings.set_keep_awake(!settings.keep_awake.get()))}

                {accent_row(__scope, settings)}

                {choice_row(__scope, "Performance mode theme",
                    [
                        (PerformanceTheme::FollowApp, performance_theme_label(PerformanceTheme::FollowApp)),
                        (PerformanceTheme::AlwaysDark, performance_theme_label(PerformanceTheme::AlwaysDark)),
                    ],
                    move || settings.performance_theme.get(),
                    move |wanted| settings.set_performance_theme(wanted))}

                {switch_row(__scope, "Dark mode",
                    move || settings.dark_mode.get(),
                    move || settings.toggle_dark())}
            }

            // Outside the scroller, not inside it.
            //
            // `1q` pins this line to the bottom of the content column with
            // `margin-top: auto`, which is right up until the column is taller
            // than the screen — and then the promise scrolls away and the screen
            // ends on "Dark mode". Out here it is the last thing on the screen
            // whatever the list above it does, which is what a promise is for.
            div {
                style: {format!("flex-shrink: 0; padding: 13px {SCREEN_PAD} 16px; \
                                 border-top: 1px solid var(--sla-hairline); {T_META}")},
                {PROMISE}
            }
        }
    }
}

/// How many attachments are saved webpages.
///
/// Derived on read from the list already in memory rather than counted into a
/// field: the startup scan loads every attachment's *metadata* (never its body —
/// see `AttachmentsStore`'s header), so the kind of each one is already here,
/// and a stored count is a number that can disagree with the list it counts.
fn saved_pages(attachments: AttachmentsStore) -> usize {
    attachments
        .items
        .get()
        .iter()
        .filter(|a| a.kind == AttachmentKind::CapturedPage)
        .count()
}

/// A group heading. `section-caps` is the handoff's own token for this.
fn section(scope: &mut RenderScope, label: &'static str) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            style: {format!("{T_SECTION_CAPS} color: var(--sla-muted); padding: 26px 0 2px;")},
            {label}
        }
    }
}

/// A row that states something and cannot be changed from here.
fn reading_row(
    scope: &mut RenderScope,
    label: &'static str,
    value: impl Fn() -> String + Copy + 'static,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div { style: {ROW},
            span { style: {format!("{T_BODY} flex: 1;")}, {label} }
            span { style: {format!("{T_META_SMALL}")}, {move || value()} }
        }
    }
}

/// A row that states something and opens the place it is changed.
fn link_row(
    scope: &mut RenderScope,
    label: &'static str,
    value: impl Fn() -> String + Copy + 'static,
    tap: impl Fn() + 'static,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            onclick: move || tap(),
            style: {ROW},
            span { style: {format!("{T_BODY} flex: 1;")}, {label} }
            span { style: {format!("{T_META_SMALL}")}, {move || value()} }
            span {
                style: "display: flex; align-items: center; color: var(--sla-muted);",
                {icon(__scope, TablerIcon::ChevronRight, 15)}
            }
        }
    }
}

/// A row carrying a switch. The whole row is the target; see the module header.
fn switch_row(
    scope: &mut RenderScope,
    label: &'static str,
    on: impl Fn() -> bool + Copy + 'static,
    tap: impl Fn() + 'static,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            onclick: move || tap(),
            style: {ROW},
            span { style: {format!("{T_BODY} flex: 1;")}, {label} }
            // The word, beside the shape. Its own closure, so it repaints with
            // the pill rather than a frame behind it — see the header.
            span { style: {move || format!("{T_META_SMALL} color: {};", if on() {
                "var(--sla-accent)"
            } else {
                "var(--sla-muted)"
            })}, {move || on_off(on()).to_string()} }
            {switch(__scope, on)}
        }
    }
}

/// The pill itself: accent track and paper knob when on, fill track and muted
/// knob when off, the knob sliding by `justify-content` rather than a transform
/// so there is no percentage translation for rinch's transition engine to drop.
fn switch(scope: &mut RenderScope, on: impl Fn() -> bool + Copy + 'static) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div {
            style: {move || format!(
                "width: 42px; height: 24px; border-radius: 999px; flex-shrink: 0; \
                 display: flex; align-items: center; padding: 0 3px; \
                 justify-content: {}; background: {};",
                if on() { "flex-end" } else { "flex-start" },
                if on() { "var(--sla-accent)" } else { "var(--sla-fill)" },
            )},
            div {
                style: {move || format!(
                    "width: 18px; height: 18px; border-radius: 999px; background: {};",
                    if on() { "var(--sla-on-accent)" } else { "var(--sla-muted)" },
                )},
            }
        }
    }
}

/// A row whose value is one of a short closed list, drawn as chips.
///
/// Generic over the value, because the two rows that want it hold different
/// enums and the alternative is the same fifteen lines twice. Both lists happen
/// to be two long; nothing here assumes that.
fn choice_row<T: Copy + PartialEq + 'static>(
    scope: &mut RenderScope,
    label: &'static str,
    options: [(T, &'static str); 2],
    current: impl Fn() -> T + Copy + 'static,
    choose: impl Fn(T) + Copy + 'static,
) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div { style: {ROW},
            span { style: {format!("{T_BODY} flex: 1;")}, {label} }
            div { style: "display: flex; gap: 6px; flex-shrink: 0;",
                for option in options {
                    let value = option.0;
                    let text = option.1;
                    let chosen = move || current() == value;
                    div {
                        key: {text},
                        onclick: move || choose(value),
                        style: {move || format!(
                            "{T_CHIP} border-radius: 999px; padding: 6px 11px; white-space: nowrap; \
                             background: {};",
                            if chosen() { "var(--sla-accent-tint)" } else { "var(--sla-fill)" },
                        )},
                        // Its own colour, for the F3 reason in the header.
                        span {
                            style: {move || format!("color: {};", if chosen() {
                                "var(--sla-accent-on-tint)"
                            } else {
                                "var(--sla-muted)"
                            })},
                            {text}
                        }
                    }
                }
            }
        }
    }
}

/// The accent row: the name of the colour on screen, then the four swatches.
///
/// Each swatch is a 24px circle inside a 40px box, because 24px is the size the
/// dot wants to be and 40px is the size a finger needs; the same trade
/// `ui::IconButton` makes for the same reason. The hexes come out of
/// `theme::ACCENTS` — the token table itself — rather than being written here,
/// which is the rule "nothing downstream hard-codes a hex" actually asks for.
/// Four colours cannot each be a CSS variable when the point of the row is to
/// show all four at once.
fn accent_row(scope: &mut RenderScope, settings: SettingsStore) -> NodeHandle {
    let __scope = scope;
    rsx! {
        div { style: {ROW},
            span { style: {format!("{T_BODY} flex: 1;")}, "Accent" }
            span { style: {format!("{T_META_SMALL}")}, {move || accent_note(settings.accent.get()).to_string()} }
            div { style: "display: flex; flex-shrink: 0; margin-right: -8px;",
                for index in 0..ACCENTS.len() {
                    let accent = ACCENTS[index];
                    div {
                        key: index,
                        onclick: move || settings.set_accent(AccentChoice::Named(index)),
                        style: "width: 40px; height: 40px; display: flex; \
                                align-items: center; justify-content: center;",
                        div {
                            style: {move || {
                                let fill = if settings.dark_mode.get() {
                                    accent.base_dark
                                } else {
                                    accent.base
                                };
                                // The ring is drawn as two shadows rather than a
                                // border so it sits *outside* the circle and the
                                // four swatches stay the same size whichever one
                                // is chosen — a border would move the other three
                                // by a pixel every time the accent changed.
                                let ring = if settings.accent_resolved() == accent {
                                    "box-shadow: 0 0 0 2px var(--sla-paper), 0 0 0 4px var(--sla-ink);"
                                } else {
                                    ""
                                };
                                format!("width: 24px; height: 24px; border-radius: 999px; \
                                         background: {fill}; {ring}")
                            }},
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The footer line is the app's central promise, and the handoff is where
    /// it is authored. Held against that file rather than against a copy of
    /// itself, so a reworded footer fails here instead of quietly becoming a
    /// different promise than the one the design makes.
    #[test]
    fn the_footer_is_the_promise_the_handoff_authored() {
        let handoff = include_str!("../../design_handoff_setlistarray/README.md");
        assert!(
            handoff.contains(PROMISE),
            "the footer line no longer matches the handoff: {PROMISE}"
        );
    }

    /// It is also the only sentence on the screen that mentions the network, and
    /// what it says about it is "no". A guard on the two words that carry that.
    #[test]
    fn the_promise_still_promises_no_upload() {
        assert!(PROMISE.contains("no connection"));
        assert!(PROMISE.contains("Nothing is uploaded anywhere."));
    }
}

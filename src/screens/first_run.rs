//! First run — WIREFRAME (`1r`), card H3, styled with the hi-fi token set.
//!
//! ## When this shows, and the flag that does not exist
//!
//! `crate::app`'s `Route::Library` arm renders this in place of
//! [`super::Library`] whenever [`crate::derive::first_run_active`] says the
//! book is empty — no field anywhere records "has this device ever seen a
//! song", because the question this screen answers is not "is this a fresh
//! install" but "does the book have anything in it right now". Those are the
//! same question the first time and different questions every time after: a
//! library somebody has just emptied by deleting the last song is back to
//! having exactly one job, and a persisted flag would have already been
//! spent on the install and would leave that second visit staring at the
//! library's old "nothing here" text instead. `first_run_active`'s own doc
//! comment carries the rest of this; the short version is that recomputing
//! from the live count is what makes the screen reachable again for free.
//!
//! ## It replaces a dead answer, not a live one
//!
//! `library.rs` used to render "Nothing here yet. Add the first song you
//! know how to play." for exactly this state, and that branch is gone now —
//! its own comment says why and points here. Two screens answering "the book
//! is empty" was one too many; the *filtered*-to-nothing state G3 added
//! (a book with songs in it that a filter has narrowed to none) is a
//! different question with a different answer and library.rs still answers
//! it.
//!
//! ## The bottom nav stays up, dimmed, and it still works
//!
//! `1r`'s own drawing has the nav at `opacity: .4` and nothing else — dimming
//! is about emphasis, this screen has one job and the nav is not it, not
//! about disabling the thing. `crate::bottom_nav` reads `first_run_active`
//! itself for the opacity and changes nothing about `onclick`, because an
//! empty library with a dead nav would be a user with no way to reach
//! Settings at all: the gear lives only in the Library and Setlists tab
//! headers, this screen draws neither, and the Setlists tab's own header
//! keeps its gear regardless of how many songs are in the book. Tapping
//! `Setlists` from here is therefore the way to Settings for as long as the
//! book stays empty, and it has to keep working.
//!
//! ## What `1r` draws that this file does not
//!
//! * **The illustration slot.** No artwork exists for it — the wireframe
//!   itself draws a dashed placeholder rather than a picture — so this file
//!   reserves the same square of space and puts nothing in it. That is a
//!   decision, not a gap: the layout is right with the box empty, and
//!   whoever adds the art later only has to fill it in, not build the slot.
//!
//! ## "or import a backup file" — card I2's line
//!
//! `1r` draws this as a secondary line under Add song, and until card I2
//! existed there was nothing behind it — a control that does nothing is the
//! one thing this codebase keeps refusing to ship (`screens::settings`'s
//! header, on `AccentChoice::FromSystem`, is the canonical statement of it).
//! It now opens the same flow the Settings → Backup row does —
//! `crate::screens::import_flow`, shared rather than reimplemented, down to
//! the same "say replace, before it happens" confirmation strip that module's
//! own header argues for. There is nothing about being on an empty library
//! that makes that confirmation less necessary: somebody who has already
//! typed in a couple of songs before deciding to import instead is exactly
//! who "this replaces everything on this device" is for, and special-casing
//! "the book was empty when I opened this screen" would be a second, narrower
//! rule sitting next to the one the flow already enforces everywhere else.
//!
//! Once an import lands, this screen does not have to notice on its own: the
//! same reactive count that swaps `FirstRun` back out for `Library` the
//! instant `songs.songs` gains its first row (see the module header below)
//! fires just as well for a book an import filled as for one a person typed
//! into, because both are the same signal changing the same way.
//!
//! ## Add song writes one field
//!
//! `1r` asks one question, so this screen collects one answer: a title and
//! nothing else. `Song::artist` is not `Option<String>` — the model was
//! never given a way to say "no artist" — but the ordinary add-song form
//! (`screens::song_form`) already treats the field as optional in practice,
//! leaving it as `String::new()` when nobody types one, and this screen does
//! the same thing rather than inventing a second required field `1r` never
//! asked for. An empty artist is therefore not a new state; it is the state
//! every song already has before anyone fills the field in.
//!
//! Submitting an empty or whitespace-only title is *refused*, not
//! prevented by a disabled button: [`add`] mirrors `song_form.rs`'s own
//! `save`, checking the trimmed title itself and doing nothing when it is
//! blank, with the button's own colour dimming to match rather than the
//! button losing its click handler. The two screens read the same rule the
//! same way on purpose — a control that is sometimes clickable and sometimes
//! not is a second interaction to learn, and this app already decided against
//! that once.
//!
//! Once a song exists, this screen does not navigate anywhere. `Route::Library`
//! stays put; `first_run_active` re-reads `SongsStore::songs` the instant the
//! store's signal changes and comes back `false`, so the very same `if` in
//! `crate::app` that put this screen up swaps it back out for `Library` on
//! its own. A `nav.go` here would be a second, redundant way of saying the
//! same thing the reactive count already says for free.

use rinch::prelude::*;

use super::import_flow::{self, ImportStatus};
use crate::model::Song;
use crate::store::{AttachmentsStore, LibraryViewStore, SettingsStore, SetlistsStore, SongsStore, Storage};
use crate::theme::{SCREEN_PAD, T_META};

/// Verbatim from the handoff (`design_handoff_setlistarray/README.md`, §13,
/// "First run — WIREFRAME (`1r`)"). A `const` rather than a literal in the
/// markup so the test at the bottom of this file can hold it against that
/// file word for word, the same guard `screens::settings::PROMISE` keeps on
/// its own footer.
const QUESTION: &str = "What's a song you know how to play?";
const SUBLINE: &str = "Add it now. Charts, keys and setlists can come later — or never.";
const FOOTER: &str = "Everything stays on this phone. No account, no signal needed.";

#[component]
pub fn FirstRun() -> NodeHandle {
    let songs = use_store::<SongsStore>();
    let setlists = use_store::<SetlistsStore>();
    let attachments = use_store::<AttachmentsStore>();
    let view = use_store::<LibraryViewStore>();
    let settings = use_store::<SettingsStore>();
    let storage = use_store::<Storage>();
    let title = Signal::new(String::new());
    // Set by an Add song tap that had nothing to add; cleared the moment a
    // title appears, the same lifetime `song_form.rs`'s `missing_title` has.
    let refused = Signal::new(false);

    // Card I2's half of this screen — see the module header. Component-local
    // for the same reason `crate::screens::settings` keeps its own copies of
    // these: neither outlives the screen that is open when either changes.
    let confirming_import = Signal::new(false);
    let import_status = Signal::new(Option::<ImportStatus>::None);

    let add = move || {
        let trimmed = title.get().trim().to_string();
        if trimmed.is_empty() {
            refused.set(true);
            return;
        }
        let mut song = Song::default();
        song.title = trimmed;
        // A failed write (`Storage` has already recorded why) leaves the
        // field exactly as typed rather than clearing it into the void —
        // `song_form.rs`'s `save` makes the same bet for the same reason.
        // A successful one needs no cleanup at all: the book is no longer
        // empty, `first_run_active` flips, and `crate::app` swaps this
        // screen out for `Library` without anyone here asking it to.
        songs.create(song);
    };

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",
            div {
                style: {format!(
                    "flex: 1; min-height: 0; overflow-y: auto; \
                     padding: 34px {SCREEN_PAD} 20px; display: flex; flex-direction: column; \
                     align-items: center; text-align: center; gap: 18px;"
                )},

                div {
                    style: "font-family: var(--sla-font-display); font-weight: 500; \
                            font-size: 26px; color: var(--sla-ink);",
                    "SetListArray"
                }

                // The illustration slot. No artwork exists for it — see the
                // module header — so this reserves `1r`'s 150×150 box and
                // draws nothing inside it rather than a placeholder icon that
                // would itself need replacing later.
                div {
                    style: "width: 150px; height: 150px; border-radius: 16px; \
                            background: var(--sla-fill); flex-shrink: 0;",
                }

                div {
                    style: "font-family: var(--sla-font-display); font-weight: 500; \
                            font-size: 21px; line-height: 1.3; color: var(--sla-ink);",
                    {QUESTION}
                }
                div { style: {format!("{T_META} font-size: 14px;")}, {SUBLINE} }

                div {
                    style: "align-self: stretch; margin-top: 6px;",
                    input {
                        r#type: "text",
                        style: {move || format!(
                            "width: 100%; border: none; border-bottom: 2px solid {}; \
                             outline: none; background: transparent; border-radius: 0; \
                             padding: 6px 0 11px; font-family: var(--sla-font-display); \
                             font-weight: 500; font-size: 19px; color: var(--sla-ink);",
                            if refused.get() { "var(--sla-danger)" } else { "var(--sla-ink)" },
                        )},
                        placeholder: "What's it called?",
                        value: {move || title.get()},
                        oninput: move |value: String| {
                            if !value.trim().is_empty() {
                                refused.set(false);
                            }
                            title.set(value);
                        },
                    }
                }

                div {
                    onclick: add,
                    // Off/on colours from the same pair every unselected/
                    // selected chip on this app uses (`fill`/`muted` versus
                    // `accent`/`on-accent`) rather than `--sla-accent-dim`,
                    // which is a different token with a different meaning —
                    // Confidence's desaturated dot, not a generic disabled
                    // state. This is the visual half of "refused, not
                    // disabled": the tap handler always runs (see `add`
                    // above), and an empty title makes it a no-op that looks
                    // like the no-op it is.
                    style: {move || {
                        let empty = title.get().trim().is_empty();
                        format!(
                            "align-self: stretch; border-radius: 22px; padding: 13px; \
                             font-family: var(--sla-font-ui); font-weight: 600; font-size: 16px; \
                             background: {}; color: {};",
                            if empty { "var(--sla-fill)" } else { "var(--sla-accent)" },
                            if empty { "var(--sla-muted)" } else { "var(--sla-on-accent)" },
                        )
                    }},
                    "Add song"
                }

                // `1r`'s secondary line — card I2's import, shared with
                // Settings' Backup row through `crate::screens::import_flow`.
                // See the module header for why the same replace-warning
                // strip applies here even though the book is, by definition,
                // empty on this screen.
                div {
                    onclick: move || confirming_import.set(true),
                    style: "font-weight: 600; font-size: 14px; color: var(--sla-muted); \
                            padding: 6px 10px;",
                    "or import a backup file"
                }
                {import_flow::confirm_strip(__scope, confirming_import, move || {
                    import_flow::start(
                        storage, songs, setlists, attachments, view, settings, import_status,
                    );
                })}
                div {
                    style: {move || format!(
                        "{T_META} color: {};",
                        import_flow::status_color(import_status.get()),
                    )},
                    {move || import_flow::status_note(import_status.get())}
                }

                div {
                    style: {format!("{T_META} margin-top: auto; padding-top: 20px;")},
                    {FOOTER}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The handoff is where this screen's three lines are authored. Held
    /// against that file rather than a copy of itself, the same guard
    /// `screens::settings::PROMISE` keeps, so a reworded question, sub-line
    /// or footer fails a test here instead of quietly drifting from the
    /// design.
    #[test]
    fn first_runs_strings_are_the_handoffs_own_words() {
        let handoff = include_str!("../../design_handoff_setlistarray/README.md");
        assert!(handoff.contains(QUESTION), "question drifted from the handoff: {QUESTION}");
        assert!(handoff.contains(SUBLINE), "sub-line drifted from the handoff: {SUBLINE}");
        assert!(handoff.contains(FOOTER), "footer drifted from the handoff: {FOOTER}");
    }
}

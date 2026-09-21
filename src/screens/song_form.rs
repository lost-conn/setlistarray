//! Add / edit song — WIREFRAME (`1j`), styled with the hi-fi token set.
//!
//! One screen for both jobs, because they are the same job: the difference
//! between adding and editing is whether the fields start empty, and whether
//! Save inserts or updates. `Route::AddSong` opens it blank;
//! `Route::EditSong(id)` opens it over an existing song.
//!
//! ## The thing this screen is for
//!
//! Someone stops mid-practice, remembers a song they can play, and wants it in
//! the book before the thought goes. Title, Save, done — that is the whole
//! path, and everything else on this screen is arranged not to be in it. Only
//! Title and Artist are visible; the eight optional fields live behind a
//! disclosure that starts closed and stays closed unless asked for.
//!
//! ## Nothing here writes except Save
//!
//! Every store mutation on this screen happens in one place — `save`, below.
//! The fields edit signals that live and die with the screen, so leaving by ✕
//! (or by any other route change) cannot half-write an edit: there is no code
//! path that could. That is the whole of how "Cancel discards" is implemented,
//! and keeping it that way is worth more than any confirmation dialog.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::derive::{
    artist_count_note, artist_suggestions, format_tags, parse_count, parse_duration, parse_tags,
    ArtistSuggestion,
};
use crate::model::{Confidence, Song, SongId, fmt_duration};
use crate::store::{DefaultTuning, NavStore, Route, SettingsStore, SongsStore};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_ROW_TITLE};
use crate::ui::{Chip, IconButton, icon};

/// Verbatim from the card, and from the handoff before it. This screen is
/// where the app's one promise gets made in words.
const PRIVACY_NOTE: &str = "Saved on this device. Nothing leaves the phone.";

/// Verbatim from the handoff's sub-line for the closed disclosure. It names
/// every field behind it, in the order they appear there, so that nobody has
/// to open it to find out whether what they want is inside.
const OPTIONAL_FIELDS: &str =
    "key · tempo · tuning · capo · duration · tags · confidence · notes — all optional";

/// The two visible fields wear the wireframe's heavy rule; a field being asked
/// for is drawn in ink, a field that has just been refused in danger.
fn underline_style(bad: bool) -> String {
    let line = if bad {
        "var(--sla-danger)"
    } else {
        "var(--sla-ink)"
    };
    format!(
        // The bottom padding is deliberately larger than the top: a rule this
        // close to a serif face otherwise cuts the descenders off the words
        // being typed on it.
        "width: 100%; border: none; border-bottom: 2px solid {line}; outline: none; \
         background: transparent; border-radius: 0; padding: 6px 0 11px; margin-top: 2px; \
         font-family: var(--sla-font-display); font-weight: 500; font-size: 19px; \
         color: var(--sla-ink);"
    )
}

/// The optional fields are quieter than the two above them: one hairline row
/// each, label left, value right.
const DETAIL_ROW: &str = "display: flex; align-items: center; gap: 12px; padding: 9px 0; \
    border-bottom: 1px solid var(--sla-hairline-soft);";
const DETAIL_LABEL: &str = "font-size: 13px; color: var(--sla-muted); width: 82px; flex-shrink: 0;";
const DETAIL_INPUT: &str = "flex: 1; min-width: 0; border: none; outline: none; \
    background: transparent; font-family: var(--sla-font-ui); font-size: 15px; \
    color: var(--sla-ink);";

/// Every field the form edits, as text.
///
/// Text, not `Option<u16>` and friends, because a half-typed number is a real
/// state the user is allowed to be in — `parse_count` and `parse_duration`
/// turn what is on screen into what gets stored, at Save, and only then. The
/// whole struct is `Copy`, so the helpers below can be called from inside the
/// reactive closures `rsx!` generates.
#[derive(Clone, Copy)]
struct Draft {
    title: Signal<String>,
    artist: Signal<String>,
    key: Signal<String>,
    tempo: Signal<String>,
    tuning: Signal<String>,
    capo: Signal<String>,
    duration: Signal<String>,
    tags: Signal<String>,
    confidence: Signal<Option<Confidence>>,
    notes: Signal<String>,
}

impl Draft {
    fn blank() -> Self {
        Self {
            title: Signal::new(String::new()),
            artist: Signal::new(String::new()),
            key: Signal::new(String::new()),
            tempo: Signal::new(String::new()),
            tuning: Signal::new(String::new()),
            capo: Signal::new(String::new()),
            duration: Signal::new(String::new()),
            tags: Signal::new(String::new()),
            confidence: Signal::new(None),
            notes: Signal::new(String::new()),
        }
    }

    /// A song, pre-filled. Every field the form can edit arrives filled in —
    /// an edit that silently dropped the tempo because the form never loaded
    /// it would be worse than no edit screen at all.
    fn of(song: &Song) -> Self {
        let draft = Self::blank();
        draft.title.set(song.title.clone());
        draft.artist.set(song.artist.clone());
        draft.key.set(song.key.clone().unwrap_or_default());
        draft.tempo.set(text_or_blank(song.tempo));
        draft.tuning.set(song.tuning.clone().unwrap_or_default());
        draft.capo.set(text_or_blank(song.capo));
        draft
            .duration
            .set(song.duration.map(fmt_duration).unwrap_or_default());
        draft.tags.set(format_tags(&song.tags));
        draft.confidence.set(song.confidence);
        draft.notes.set(song.notes.clone().unwrap_or_default());
        draft
    }

    /// The draft onto a song. Only the fields this form owns are touched:
    /// attachments, play history and the added-at time belong to the song and
    /// survive an edit untouched.
    ///
    /// A field emptied here is cleared rather than left alone — deleting a key
    /// you got wrong has to actually delete it. `repo::save_song` writes the
    /// absent ones as `Null` for the same reason.
    fn apply(self, song: &mut Song) {
        song.title = self.title.get().trim().to_string();
        song.artist = self.artist.get().trim().to_string();
        song.key = some_text(self.key.get());
        song.tempo = parse_count(&self.tempo.get()).and_then(|n| u16::try_from(n).ok());
        song.tuning = some_text(self.tuning.get());
        song.capo = parse_count(&self.capo.get()).and_then(|n| u8::try_from(n).ok());
        song.duration = parse_duration(&self.duration.get());
        song.tags = parse_tags(&self.tags.get());
        song.confidence = self.confidence.get();
        song.notes = some_text(self.notes.get());
    }
}

/// The draft a **brand-new** song starts from: blank, except the Tuning field,
/// which arrives carrying today's default (card H5) — visible the moment
/// "More details" is opened, and editable there like any other field before
/// Save ever runs.
///
/// This is a free function taking the preference as a plain value, rather
/// than a second constructor on `Draft` that reaches into a store itself, for
/// one reason worth being explicit about: it is the *only* thing in this file
/// that reads `SettingsStore::default_tuning`, and keeping the read at the
/// call site (below, in the `None` arm and nowhere else) is what makes it
/// structurally impossible for the edit path to pick it up by accident. There
/// is no `Draft::of` that could grow this call later without someone
/// deliberately adding it there — see the module header's "Nothing here
/// writes except Save" for why that kind of impossibility, rather than a
/// runtime check, is how this screen keeps its promises.
fn blank_draft(default_tuning: DefaultTuning) -> Draft {
    let draft = Draft::blank();
    draft.tuning.set(default_tuning.label().to_string());
    draft
}

/// A number back into the text the field edits, or nothing at all. An unset
/// field is empty, never `0` — the handoff's rule that an unfilled field is
/// simply absent holds inside the form as well as outside it.
fn text_or_blank(value: Option<impl std::fmt::Display>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

/// Trimmed text, or `None` when there is nothing left of it.
fn some_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Solid · Rusty · Learning · Unrated, in the order the dots fill — the same
/// order and the same words as the overflow menu's confidence rows.
fn confidence_choices() -> [(&'static str, Option<Confidence>); 4] {
    [
        ("Solid", Some(Confidence::Solid)),
        ("Rusty", Some(Confidence::Rusty)),
        ("Learning", Some(Confidence::Learning)),
        ("Unrated", None),
    ]
}

#[component]
pub fn SongForm(editing: Option<SongId>) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();
    let settings = use_store::<SettingsStore>();

    // Components run once, so this reads the song being edited exactly once —
    // at mount. From here on the signals are the truth on screen and the store
    // is not consulted again until Save.
    let existing = editing.and_then(|id| songs.get(id));
    if editing.is_some() && existing.is_none() {
        return rsx! {
            div { style: {format!("padding: {SCREEN_PAD}; {T_META}")}, "This song is gone." }
        };
    }

    let editing_id = existing.as_ref().map(|s| s.id);
    // `Route::EditSong` always takes the `Some` arm here, which reads the
    // song's own stored tuning (or leaves it blank, if that is what the song
    // has) and never `blank_draft` — so the default-tuning preference has no
    // path into an edit, deliberately, not by the two happening to agree
    // today. See `blank_draft`'s own doc comment.
    let draft = match &existing {
        Some(song) => Draft::of(song),
        None => blank_draft(settings.default_tuning.get()),
    };

    let more = Signal::new(false);
    // The `use "<typed>"` escape hatch: the suggestion list is a help, and one
    // tap has to be enough to be rid of it. See `suggestions` below for how
    // long it stays gone.
    let artist_dismissed = Signal::new(false);
    // Set by a Save that had nothing to save. Cleared the moment a title
    // appears, so the complaint never outlives the problem.
    let missing_title = Signal::new(false);

    let heading = if editing_id.is_some() {
        "Edit song"
    } else {
        "New song"
    };

    // Leaving without saving. An edit goes back to the song it was about; a
    // new song has nowhere of its own to return to yet.
    let cancel = move || match editing_id {
        Some(id) => nav.go(Route::SongDetail(id)),
        None => nav.back(),
    };

    // The phone's Back key is this screen's ✕, handed the closure itself rather
    // than a second copy of the two-door rule above it. `nav.back()` alone
    // would be wrong on the edit door: it lands on the tab root, so backing out
    // of an edit would drop you in the library instead of on the song you were
    // editing, and the two would drift apart the first time either was changed.
    nav.register_back(cancel);

    // The only place on this screen that writes anything.
    let save = move || {
        if draft.title.get().trim().is_empty() {
            // Nothing is written and nothing is dismissed: the form stays put,
            // the Title rule turns red and says what it wants. A song without
            // a title is not a song, and a Save that quietly did nothing would
            // read as a Save that quietly lost the song.
            missing_title.set(true);
            return;
        }
        match editing_id {
            Some(id) => {
                songs.edit(id, |song| draft.apply(song));
                nav.go(Route::SongDetail(id));
            }
            None => {
                let mut song = Song::default();
                draft.apply(&mut song);
                // A failed write leaves the form exactly as it was, with the
                // typing still in it — `Storage` has already recorded the
                // fault, and losing the words on top of that helps nobody.
                if songs.create(song).is_some() {
                    nav.back();
                }
            }
        }
    };

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",

            // ✕ · heading · Save, per `1j`. Save is the accent word on the
            // screen and the only one; it dims while there is nothing to save.
            div {
                style: "padding: 2px 14px 8px; display: flex; align-items: center; gap: 8px; \
                        border-bottom: 1px solid var(--sla-hairline);",
                IconButton { glyph: TablerIcon::X, size: 18, onclick: cancel }
                div { style: {format!("{T_ROW_TITLE} font-size: 19px; flex: 1;")}, {heading} }
                div {
                    onclick: save,
                    style: {move || {
                        let color = if draft.title.get().trim().is_empty() {
                            "var(--sla-muted)"
                        } else {
                            "var(--sla-accent)"
                        };
                        format!("color: {color}; font-weight: 600; font-size: 16px; padding: 10px 8px;")
                    }},
                    "Save"
                }
            }

            div {
                style: {format!("flex: 1; min-height: 0; overflow-y: auto; \
                                 padding: 16px {SCREEN_PAD} 24px; display: flex; flex-direction: column;")},

                // Title — the one field a song cannot do without.
                div {
                    div { style: {format!("{T_META_SMALL}")}, "Title" }
                    input {
                        r#type: "text",
                        style: {move || underline_style(missing_title.get())},
                        placeholder: "What's it called?",
                        value: {|| draft.title.get()},
                        oninput: move |value: String| {
                            if !value.trim().is_empty() {
                                missing_title.set(false);
                            }
                            draft.title.set(value);
                        },
                    }
                    if missing_title.get() {
                        div {
                            style: {format!("{T_META_SMALL} color: var(--sla-danger); margin-top: 6px;")},
                            "A song needs a title. Everything else can wait."
                        }
                    }
                }

                // Artist — optional, but the field that most wants help, since
                // the same handful of names come round again and again.
                div { style: "margin-top: 20px;",
                    div { style: {format!("{T_META_SMALL}")}, "Artist" }
                    input {
                        r#type: "text",
                        style: {underline_style(false)},
                        placeholder: "Who plays it?",
                        value: {|| draft.artist.get()},
                        oninput: move |value: String| {
                            // Clearing the field starts the question again, so
                            // the list is allowed back.
                            if value.trim().is_empty() {
                                artist_dismissed.set(false);
                            }
                            draft.artist.set(value);
                        },
                    }

                    // The suggestion box, and the escape hatch at the foot of
                    // it. Both `if` and `for` re-run as their own closures, so
                    // each recomputes from the `Copy` store and signals rather
                    // than capturing a list from around it.
                    if !suggestions(songs, draft, artist_dismissed).is_empty() {
                        div {
                            style: "margin-top: 6px; border: 1px solid var(--sla-hairline); \
                                    border-radius: 10px; overflow: hidden; background: var(--sla-card);",

                            for suggestion in suggestions(songs, draft, artist_dismissed) {
                                let name = suggestion.name.clone();
                                let note = artist_count_note(suggestion.songs);

                                div {
                                    key: {suggestion.name.clone()},
                                    // Tapping a suggestion adopts the book's
                                    // spelling — which is the point of showing
                                    // it. Nothing else rewrites the field.
                                    onclick: {
                                        let name = name.clone();
                                        move || draft.artist.set(name.clone())
                                    },
                                    style: "display: flex; align-items: baseline; gap: 7px; \
                                            padding: 10px 13px; \
                                            border-bottom: 1px solid var(--sla-hairline-soft);",
                                    span {
                                        style: "font-size: 15px; color: var(--sla-ink); \
                                                overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                                        {name.clone()}
                                    }
                                    span { style: {format!("{T_META_SMALL}")}, {note.clone()} }
                                }
                            }

                            // `use "<typed>"`. It changes nothing about the
                            // song — what was typed is already in the field —
                            // it only says "stop offering", which is exactly
                            // what someone adding an artist the book has never
                            // heard of needs it to mean.
                            div {
                                onclick: move || artist_dismissed.set(true),
                                style: {format!("{T_META} padding: 10px 13px;")},
                                {move || format!("use \"{}\"", draft.artist.get().trim())}
                            }
                        }
                    }
                }

                // Everything optional, behind one tap. Closed on arrival, and
                // the sub-line says what is inside so that opening it is never
                // a guess.
                div {
                    style: "margin-top: 26px; padding-top: 14px; border-top: 1px solid var(--sla-hairline);",

                    div {
                        onclick: move || more.update(|open| *open = !*open),
                        style: "display: flex; align-items: center; gap: 8px;",
                        span { style: {format!("{T_ROW_TITLE} font-size: 17px; flex: 1;")}, "More details" }
                        span { style: "color: var(--sla-muted); display: flex;",
                            if more.get() {
                                {icon(__scope, TablerIcon::ChevronUp, 18)}
                            } else {
                                {icon(__scope, TablerIcon::ChevronDown, 18)}
                            }
                        }
                    }

                    if more.get() {
                        div { style: "margin-top: 4px;",
                            {detail_row(__scope, "Key", "G", draft.key)}
                            {detail_row(__scope, "Tempo", "96", draft.tempo)}
                            {detail_row(__scope, "Tuning", "Standard", draft.tuning)}
                            {detail_row(__scope, "Capo", "2", draft.capo)}
                            {detail_row(__scope, "Duration", "3:44", draft.duration)}
                            {detail_row(__scope, "Tags", "campfire, open mic", draft.tags)}

                            // Confidence is the one optional field with a
                            // fixed set of answers, so it gets the chips the
                            // rest of the app already uses rather than a
                            // seventh text box.
                            div {
                                style: {format!("{DETAIL_ROW} flex-wrap: wrap; padding: 12px 0;")},
                                span { style: {DETAIL_LABEL}, "Confidence" }
                                for (label, value) in confidence_choices() {
                                    // Both bindings are named with their types on purpose.
                                    // Every use of them below sits inside a closure — `key:`
                                    // and `label:` reach `label` through `ToString`, `active:`
                                    // and `onclick:` capture `value` — and a closure body is
                                    // not checked until its captures' types are known, so
                                    // nothing in the loop ever pins the item and the element
                                    // type of the iterable never reaches the pattern. The
                                    // sibling loop in `filter_sheet.rs` needs none of this
                                    // because its `active:` is a plain call whose parameter
                                    // type does the pinning eagerly.
                                    let label: &'static str = label;
                                    let value: Option<Confidence> = value;
                                    Chip {
                                        key: {label},
                                        label: {label.to_string()},
                                        active: {move || draft.confidence.get() == value},
                                        selected: false,
                                        onclick: move || draft.confidence.set(value),
                                    }
                                }
                            }

                            div { style: "padding: 12px 0 4px;",
                                div { style: {DETAIL_LABEL}, "Notes" }
                                textarea {
                                    rows: "4",
                                    style: "width: 100%; margin-top: 6px; padding: 11px 13px; \
                                            border: 1px solid var(--sla-hairline); border-radius: 12px; \
                                            outline: none; background: var(--sla-card); \
                                            font-family: var(--sla-font-ui); font-size: 15px; \
                                            color: var(--sla-ink);",
                                    placeholder: "Anything worth remembering next time.",
                                    value: {|| draft.notes.get()},
                                    oninput: move |value: String| draft.notes.set(value),
                                }
                            }
                        }
                    } else {
                        div { style: {format!("{T_META_SMALL} margin-top: 8px;")}, {OPTIONAL_FIELDS} }
                    }
                }

                // `1j` puts an Attachments section here — Pick a PDF · Save a
                // webpage offline · Type lyrics / chords. It is not built: all
                // three are Phase D cards, and a song being added does not
                // exist yet to hang a chart on. Three rows that looked live and
                // did nothing would be worse than the gap, so the gap is left
                // and this comment marks where they go.

                // The scroll container's own bottom padding stops at the last
                // box rather than after it, so the note carries its own — the
                // promise being cut off mid-descender is not the impression to
                // leave on the screen that makes it.
                div {
                    style: {format!("{T_META_SMALL} margin-top: auto; padding: 28px 0 10px;")},
                    {PRIVACY_NOTE}
                }
            }
        }
    }
}

/// One optional field: label, then a text box that spends most of its life
/// empty. Takes only `Copy` values so it can be called from inside the
/// disclosure's reactive `if`.
#[component]
fn detail_row(label: &str, placeholder: &str, value: Signal<String>) -> NodeHandle {
    let label = label.to_string();
    let placeholder = placeholder.to_string();

    rsx! {
        div { style: {DETAIL_ROW},
            span { style: {DETAIL_LABEL}, {label.clone()} }
            input {
                r#type: "text",
                style: {DETAIL_INPUT},
                placeholder: {placeholder.clone()},
                value: {move || value.get()},
                oninput: move |typed: String| value.set(typed),
            }
        }
    }
}

/// The artists to offer under the field, or none at all.
///
/// `artist_suggestions` already withholds the list for an empty field and for
/// a name typed out in full; this adds the one state that belongs to the
/// screen rather than to the data — the escape hatch having been tapped.
///
/// A dismissal lasts until the field is emptied, not until the next keystroke.
/// Someone who has just said "no, this is a new artist" is about to type the
/// rest of the name, and having the list reappear on every letter of it would
/// make the escape hatch an escape from nothing.
///
/// Takes only `Copy` arguments so it can be called from inside a reactive
/// closure.
fn suggestions(
    songs: SongsStore,
    draft: Draft,
    dismissed: Signal<bool>,
) -> Vec<ArtistSuggestion> {
    if dismissed.get() {
        return Vec::new();
    }
    artist_suggestions(&songs.songs.get(), &draft.artist.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SongsStore;

    /// Card H5's prefill, at the one function that performs it. Everything
    /// else on a brand-new draft stays exactly as blank as `Draft::blank`
    /// leaves it — the prefill touches Tuning and nothing beside it.
    #[test]
    fn a_new_songs_draft_is_prefilled_with_the_default_tuning() {
        let draft = blank_draft(DefaultTuning::HalfStepDown);
        assert_eq!(draft.tuning.get(), "Half-step down");
        assert!(draft.title.get().is_empty());
        assert!(draft.artist.get().is_empty());
    }

    /// The edit path (`Draft::of`) never calls `blank_draft`, so a song saved
    /// with no tuning of its own opens for editing with Tuning still blank —
    /// whatever `SettingsStore::default_tuning` happens to be set to. This is
    /// the "prefill, not implied meaning" half of the card: a preference is
    /// free to change what the *next* song starts on without being allowed an
    /// opinion about a song that already chose to leave the field empty.
    #[test]
    fn editing_a_song_with_no_tuning_of_its_own_never_gets_the_default_prefill() {
        let song = Song::new(1, "Reuben's Train", "Trad.");
        assert_eq!(song.tuning, None);
        let draft = Draft::of(&song);
        assert_eq!(draft.tuning.get(), "");
    }

    /// The other half of "test both paths": a song that *does* have a tuning
    /// keeps it on edit, untouched by whatever the default is today.
    #[test]
    fn editing_a_song_with_its_own_tuning_keeps_it_regardless_of_the_default() {
        let mut song = Song::new(1, "Copperhead Road", "Steve Earle");
        song.tuning = Some("Drop D".to_string());
        let draft = Draft::of(&song);
        assert_eq!(draft.tuning.get(), "Drop D");
    }

    /// The property the card's own reasoning demands: changing the preference
    /// after a song exists must not rewrite what that song means. This walks
    /// the real path — `blank_draft`, `Draft::apply`, `SongsStore::create` —
    /// the same three calls `save`'s `None` arm makes, for two songs created
    /// on either side of a preference change, and reads both back from the
    /// store afterwards.
    #[test]
    fn a_song_created_before_the_default_tuning_changed_still_reads_back_with_the_tuning_it_was_created_with() {
        let songs = SongsStore::new(Vec::new());

        let first_draft = blank_draft(DefaultTuning::Standard);
        first_draft.title.set("Wagon Wheel".to_string());
        let mut first_song = Song::default();
        first_draft.apply(&mut first_song);
        let first_id = songs.create(first_song).expect("first song is created");

        // The preference changes after the first song already exists.
        let second_draft = blank_draft(DefaultTuning::DropD);
        second_draft.title.set("Angel from Montgomery".to_string());
        let mut second_song = Song::default();
        second_draft.apply(&mut second_song);
        let second_id = songs.create(second_song).expect("second song is created");

        assert_eq!(
            songs.get(first_id).unwrap().tuning.as_deref(),
            Some("Standard"),
            "changing the preference must not rewrite a song created under the old one"
        );
        assert_eq!(songs.get(second_id).unwrap().tuning.as_deref(), Some("Drop D"));
    }
}

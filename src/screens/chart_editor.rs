//! Typed lyrics / chords — card D2. **No wireframe exists for this screen.**
//!
//! `1j` names it as one row of the Add-song form (`TXT · Type lyrics / chords ·
//! ›`) and stops there; `1k` is the *viewer*, and the hi-fi draws only the
//! collapsed `lyrics · typed` row this screen fills in. So rather than invent a
//! screen the handoff never drew, this is the minimal reading of the card —
//! full-screen `✕ / title / Save`, one field, writing a `Text` attachment —
//! wearing the chrome `song_form.rs` (`1j`) already established, because that
//! is the closest sibling and the only header/footer pattern the app has for a
//! full-screen form.
//!
//! ## The four decisions the card asked for
//!
//! **The attachment is created by Save, never by opening the screen.** Every
//! other reading loses: `SongsStore::attach` mints a row *and* a directory, so
//! create-on-open would litter a directory per abandoned edit, and — because
//! the first chart a song gets becomes its primary one (`Song::attach`) — an
//! empty ghost chart would take the card on song detail away from a real one.
//! It also keeps the rule `song_form.rs` states in its own header: nothing on a
//! form screen writes except Save.
//!
//! **`✕` asks, but only when there is something to lose.** `song_form` discards
//! silently and says so is worth more than a dialog, and for two short fields
//! it is right. This screen is different in kind: what is in it is a verse
//! somebody typed out, there is no undo anywhere in this app, and `✕` sits one
//! finger-width from the text. So leaving with the text exactly as it was
//! found — including leaving an untouched empty screen — goes straight back;
//! leaving with changed text raises one inline strip, and nothing else on the
//! screen moves.
//!
//! **Save refuses an empty body rather than writing one.** A chart with no
//! lines is not a chart: it would take the primary card, render as "Nothing
//! typed yet." and have to be removed by hand. Refusing is also what makes
//! "Save deletes it" unnecessary — emptying an existing chart and saving is a
//! *removal*, and this app already has one, on the long-press menu, where a
//! destructive action can be recognised as one. The refusal says so.
//!
//! **An existing typed chart opens in this same screen.** The card asks for an
//! "editor", D1 gave every chart a stable id, and the alternative — type once,
//! never correct a chord — is not a text editor. `Route::TypeChart` carries an
//! `Option<AttachmentId>`: `None` is a new chart, `Some` is that one, and the
//! way in is the chart's own long-press menu (`menu::AttachmentMenuItems`).
//!
//! ## What the framework cannot do yet, and what this screen does about it
//!
//! A `<textarea>` **cannot scroll to its caret** on this build: whatever falls
//! past its `rows` height is clipped at the border and unreachable. So the
//! field never has a fixed height — [`field_rows`] drives `rows` from the
//! value, the box grows as it is typed into, and the *screen* scrolls. That is
//! approximate for soft-wrapped lines, so the estimate deliberately runs high:
//! a narrow column count and two spare rows, because over-guessing costs blank
//! paper and under-guessing swallows a line.
//!
//! Tapping to place the caret used to land a line off, and that one is fixed
//! rather than worked around: this field is an inline-block the surrounding
//! text flow positions, so the anonymous block wrapping it carries the padding
//! of the column above (22px across, 14px down). Paint and hit testing both
//! added that back; the caret arithmetic summed the parent chain and did not,
//! so a tap was measured against a box 14px above the painted one — 0.89 of a
//! 15.71px line, and so a line. Fixed upstream in joeleaver/rinch#310.
//!
//! The mono face below used to be a desktop-only promise — `font-family:
//! monospace` resolved to nothing on Android and the chords floated over the
//! wrong syllables. The app now ships DejaVu Sans Mono and declares it as
//! `monospace` (`crate::FONTS`), so the CSS here, which was always written for
//! the face it wanted, finally gets it.

use rinch::prelude::*;
use rinch_tabler_icons::TablerIcon;

use crate::model::{Attachment, AttachmentId, AttachmentKind, SongId};
use crate::store::{AttachmentsStore, NavStore, Route, SongsStore};
use crate::theme::{SCREEN_PAD, T_META, T_META_SMALL, T_ROW_TITLE};
use crate::ui::IconButton;

/// The name a typed chart wears in the collapsed row under the card, when its
/// text gives nothing better to call it. The hi-fi's own row reads
/// `lyrics · typed`, and `AttachmentKind::descriptor` supplies the second half.
const FALLBACK_TITLE: &str = "lyrics";

/// How much of a first line is worth keeping as a name.
const TITLE_LIMIT: usize = 40;

/// Roughly how many monospaced characters fit across the field on the 393px
/// phone the designs assume: ~349px of content width at 13.5px, where a
/// monospace advance is about 0.6em. Deliberately rounded *down* — see
/// [`field_rows`] for why the estimate is allowed to be generous and not
/// allowed to be mean.
const COLUMNS: usize = 38;

/// The field never gets smaller than this, so an empty screen still reads as
/// somewhere to write a song rather than as a search box.
const MIN_ROWS: usize = 12;

/// Blank lines kept below the last one, so there is always somewhere for the
/// next line to appear before the box has to grow.
const SPARE_ROWS: usize = 2;

/// Room under the field for the soft keyboard.
///
/// Rinch exposes no IME inset on Android, so a focused field can end up behind
/// the keyboard with nothing telling the app it happened. This is the whole
/// mitigation: enough scrollable emptiness below the field that the screen can
/// always be scrolled far enough to bring the line being typed above the
/// keyboard by hand. It is not a fix and it is not pretending to be one.
const KEYBOARD_ROOM: &str = "320px";

/// The editor's own face. `T_CHART` renders a saved chart in the card; this is
/// the same intent for the field it is typed into — the chord names have to sit
/// over the syllables they land on, and in a proportional face that alignment
/// is noise. Slightly larger than the card's 12.5px, because this one is being
/// typed into rather than glanced at.
const FIELD: &str = "width: 100%; margin-top: 10px; padding: 14px 15px; \
    border: 1px solid var(--sla-hairline); border-radius: 14px; outline: none; \
    background: var(--sla-card); font-family: var(--sla-font-mono); \
    font-size: 13.5px; line-height: 1.5; color: var(--sla-ink);";

/// How many rows of the field a body needs to be visible in full.
///
/// The framework gap this exists for is in the module header. In one line: a
/// `<textarea>` clips at its `rows` height and cannot scroll, so the height has
/// to be right or the text is gone.
///
/// Soft wrapping is what makes this an estimate — the field wraps at whatever
/// width it actually got, in whatever face actually resolved, and neither is
/// visible from here. The bias is deliberate: guessing too many rows leaves
/// blank paper at the bottom of a screen that scrolls anyway, and guessing too
/// few eats what somebody typed.
///
/// A trailing newline counts as the empty line it is — that is where the caret
/// sits after pressing Enter, and it needs a row.
pub fn visual_rows(text: &str, columns: usize) -> usize {
    let columns = columns.max(1);
    text.split('\n')
        .map(|line| line.chars().count().div_ceil(columns).max(1))
        .sum()
}

/// The `rows` the field is actually given: what the text needs, room to keep
/// typing, and never less than a screenful.
pub fn field_rows(text: &str) -> usize {
    (visual_rows(text, COLUMNS) + SPARE_ROWS).max(MIN_ROWS)
}

/// What to call a typed chart, from the text of it.
///
/// The title shows in exactly one place — the collapsed row under the primary
/// card, as `<title> · typed` — and a constant would make two typed charts on
/// one song read identically, which is the state somebody keeping a lyric sheet
/// and a chord sheet is most likely to be in. So the first line that has
/// anything on it names the chart, which for a chart is usually its chords or
/// its opening words, and an empty text falls back to the hi-fi's own word.
///
/// It is re-derived on every Save rather than fixed at creation: nothing else
/// in the app can set an attachment title, so a name held over from an earlier
/// first line could only ever be stale.
pub fn chart_title(body: &str) -> String {
    let first = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if first.is_empty() {
        return FALLBACK_TITLE.to_string();
    }
    // A chord line is mostly spaces; collapsing them keeps the row from
    // reading as a gap with two letters at either end of it.
    let collapsed = first.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= TITLE_LIMIT {
        return collapsed;
    }
    let cut: String = collapsed.chars().take(TITLE_LIMIT).collect();
    format!("{}…", cut.trim_end())
}

/// Whether there is anything here worth writing. Whitespace is not: a chart of
/// four spaces and a newline is an empty chart that would take the primary
/// card.
pub fn writable(text: &str) -> Option<&str> {
    (!text.trim().is_empty()).then_some(text)
}

#[component]
pub fn ChartEditor(song: Option<SongId>, chart: Option<AttachmentId>) -> NodeHandle {
    let nav = use_store::<NavStore>();
    let songs = use_store::<SongsStore>();
    let attachments = use_store::<AttachmentsStore>();

    let song = song.unwrap_or_default();

    // Components run once, so the body is read from the store exactly once, at
    // mount, and the signal is the truth on screen from then until Save. This
    // is the one body read this screen makes — `AttachmentsStore::body` is an
    // object read, and doing it inside a reactive closure would repeat it on
    // every keystroke.
    let existing = chart.and_then(|id| attachments.get(id).map(|a| (id, a)));
    if songs.get(song).is_none() {
        return rsx! {
            div { style: {format!("padding: {SCREEN_PAD}; {T_META}")}, "This song is gone." }
        };
    }
    if chart.is_some() && existing.is_none() {
        return rsx! {
            div { style: {format!("padding: {SCREEN_PAD}; {T_META}")}, "This chart is gone." }
        };
    }

    let editing = existing.as_ref().map(|(id, _)| *id);
    let opened_with = editing
        .and_then(|id| attachments.body(id))
        .unwrap_or_default();

    let text = Signal::new(opened_with.clone());
    // The text as it was found, kept in a signal rather than a `String` so the
    // reactive closures below — which can only capture `Copy` values — can ask
    // whether anything has changed.
    let original = Signal::new(opened_with);
    // Raised by ✕ over changed text, and by nothing else.
    let confirming = Signal::new(false);
    // Set by a Save that had nothing to write. Cleared the moment a line
    // appears, so the complaint never outlives the problem.
    let blank = Signal::new(false);

    let leave = move || nav.go(Route::SongDetail(song));

    let close = move || {
        if text.get() == original.get() {
            leave();
        } else {
            confirming.set(true);
        }
    };

    // The only place on this screen that writes anything.
    let save = move || {
        let body = text.get();
        if writable(&body).is_none() {
            blank.set(true);
            return;
        }
        let title = chart_title(&body);
        // Bytes, not blocks: a typed chart's text lives in its database row
        // rather than in its attachment directory, so this is what it costs
        // the library, which is what the Storage screen (H1) is counting.
        let bytes = body.len() as u64;

        let written = match editing {
            Some(id) => attachments.update(id, |a| {
                a.title = title;
                a.bytes_on_disk = bytes;
                a.body = Some(body.clone());
            }),
            None => songs
                .attach(
                    song,
                    Attachment {
                        id: 0,
                        kind: AttachmentKind::Text,
                        title,
                        bytes_on_disk: bytes,
                        page_count: None,
                        source_url: None,
                        captured_at: None,
                        body: Some(body.clone()),
                    },
                )
                .is_some(),
        };

        // A failed write leaves the screen exactly as it was, with the typing
        // still in it — `Storage` has already recorded the fault, and losing
        // the words on top of that helps nobody. Same rule as `song_form`.
        if written {
            leave();
        }
    };

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",

            // ✕ · heading · Save — `1j`'s header, because this screen is the
            // row on `1j` that opened it.
            div {
                style: "padding: 2px 14px 8px; display: flex; align-items: center; gap: 8px; \
                        border-bottom: 1px solid var(--sla-hairline);",
                IconButton { glyph: TablerIcon::X, size: 18, onclick: close }
                div { style: {format!("{T_ROW_TITLE} font-size: 19px; flex: 1;")}, "Lyrics / chords" }
                div {
                    onclick: save,
                    style: {move || {
                        let color = if writable(&text.get()).is_some() {
                            "var(--sla-accent)"
                        } else {
                            "var(--sla-muted)"
                        };
                        format!("color: {color}; font-weight: 600; font-size: 16px; padding: 10px 8px;")
                    }},
                    "Save"
                }
            }

            // The one question this screen asks. An inline strip rather than a
            // dialog: there is no modal anywhere in this app, and the text it
            // is about has to stay visible behind the choice.
            if confirming.get() {
                div {
                    style: "padding: 12px 14px; background: var(--sla-fill); \
                            border-bottom: 1px solid var(--sla-hairline); \
                            display: flex; align-items: center; gap: 10px; flex-wrap: wrap;",
                    span { style: {format!("{T_META} flex: 1; min-width: 140px;")},
                        "Leave without saving? This typing is not stored yet."
                    }
                    div {
                        onclick: move || confirming.set(false),
                        style: "font-weight: 600; font-size: 15px; color: var(--sla-ink-2); padding: 8px 10px;",
                        "Keep typing"
                    }
                    div {
                        onclick: move || { confirming.set(false); leave(); },
                        style: "font-weight: 600; font-size: 15px; color: var(--sla-danger); padding: 8px 10px;",
                        "Discard"
                    }
                }
            }

            div {
                style: {format!("flex: 1; min-height: 0; overflow-y: auto; \
                                 padding: 14px {SCREEN_PAD} 0;")},

                div { style: {format!("{T_META_SMALL}")},
                    "Chords over the words. The spacing is kept exactly as you type it."
                }

                textarea {
                    // Driven by the value, because a `<textarea>` cannot scroll
                    // to its own caret on this build and anything past `rows`
                    // is clipped and unreachable. The screen scrolls instead.
                    rows: {move || field_rows(&text.get())},
                    // Deliberately no `data-onsubmit`: with one, Enter submits
                    // and only Shift+Enter breaks the line. This field is
                    // nothing but lines.
                    style: {FIELD},
                    placeholder: "G                 D\nCarolina in my mind",
                    value: {move || text.get()},
                    oninput: move |value: String| {
                        if !value.trim().is_empty() {
                            blank.set(false);
                        }
                        text.set(value);
                    },
                }

                if blank.get() {
                    // Both of these name the control in words rather than
                    // drawing it. The second used to read "tap \u{2715} to leave",
                    // with the character itself sitting in the sentence — the
                    // same fault K13 cleared out of the chips, where a glyph no
                    // bundled face carries tofus on Android. K13's rule was
                    // "use a Tabler glyph for an icon, spell the word for an
                    // arrow", and prose pointing *at* an icon is a third case
                    // neither half covers: the icon is real (`IconButton` with
                    // `TablerIcon::X`, drawn as a path, above), it is the
                    // sentence that cannot carry a picture. So the sentence
                    // says what the control does, the way its sibling one
                    // branch up already did. Card K21.
                    div {
                        style: {format!("{T_META_SMALL} color: var(--sla-danger); margin-top: 8px;")},
                        {if editing.is_some() {
                            "Nothing typed. To delete this chart, long-press it on the song and choose Remove attachment."
                        } else {
                            "Nothing typed yet. Write a line, or close the editor to leave."
                        }}
                    }
                }

                // The keyboard's share of the screen — see `KEYBOARD_ROOM`.
                div { style: {format!("height: {KEYBOARD_ROOM}; flex-shrink: 0;")} }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── the auto-grow (the no-scroll-to-caret stopgap) ──────────────────────

    #[test]
    fn an_empty_field_is_one_row_and_a_trailing_newline_is_two() {
        assert_eq!(visual_rows("", COLUMNS), 1);
        assert_eq!(visual_rows("Carolina", COLUMNS), 1);
        // The caret is on the empty line after Enter, and it needs a row.
        assert_eq!(visual_rows("Carolina\n", COLUMNS), 2);
        assert_eq!(visual_rows("\n\n", COLUMNS), 3);
    }

    #[test]
    fn every_line_counts_including_the_blank_ones_between_verses() {
        assert_eq!(visual_rows("verse one\n\nverse two", COLUMNS), 3);
    }

    #[test]
    fn a_line_too_long_for_the_field_counts_as_the_rows_it_wraps_onto() {
        assert_eq!(visual_rows(&"x".repeat(10), 10), 1);
        assert_eq!(visual_rows(&"x".repeat(11), 10), 2);
        assert_eq!(visual_rows(&"x".repeat(30), 10), 3);
        // ...and a wrapped line does not steal the row of the one after it.
        assert_eq!(visual_rows(&format!("{}\nshort", "x".repeat(25)), 10), 4);
    }

    #[test]
    fn the_row_count_is_measured_in_characters_not_bytes() {
        // Four accented characters, eight bytes. A field two of them wide
        // wraps them onto two rows, not four.
        assert_eq!(visual_rows("éééé", 2), 2);
    }

    /// The whole point of the estimate: it may be generous, it may never be
    /// mean. A field one row short of its text swallows a line with no way to
    /// scroll to it.
    #[test]
    fn the_field_is_never_smaller_than_the_text_in_it() {
        let long: String = (0..40).map(|n| format!("line {n}\n")).collect();
        for body in ["", "one line", "a\nb\nc", &long, &"y".repeat(400)] {
            assert!(
                field_rows(body) >= visual_rows(body, COLUMNS),
                "{body:?} would clip"
            );
        }
        assert_eq!(field_rows(""), MIN_ROWS, "and an empty one is a screenful");
        assert_eq!(field_rows(&long), 40 + 1 + SPARE_ROWS);
    }

    // ── the empty-body decision ─────────────────────────────────────────────

    #[test]
    fn whitespace_is_not_a_chart() {
        assert!(writable("").is_none());
        assert!(writable("   ").is_none());
        assert!(writable("\n\n \t\n").is_none());
        assert_eq!(writable("\nG  D\n"), Some("\nG  D\n"), "and is kept verbatim");
    }

    // ── the name a chart wears ──────────────────────────────────────────────

    #[test]
    fn a_chart_is_named_after_its_first_line_of_anything() {
        assert_eq!(chart_title("Capo 3\nG   D"), "Capo 3");
        assert_eq!(chart_title("\n\n  Verse one  \nmore"), "Verse one");
        // A chord line's alignment is meaningless once it is a title.
        assert_eq!(chart_title("G       D      Em"), "G D Em");
    }

    #[test]
    fn a_chart_with_no_text_falls_back_to_the_word_the_hifi_uses() {
        assert_eq!(chart_title(""), FALLBACK_TITLE);
        assert_eq!(chart_title("   \n\n"), FALLBACK_TITLE);
    }

    #[test]
    fn a_long_first_line_is_cut_rather_than_filling_the_row() {
        let title = chart_title(&"la ".repeat(40));
        assert!(title.chars().count() <= TITLE_LIMIT + 1, "{title}");
        assert!(title.ends_with('…'));
        assert!(!title.contains(" …"), "no space left dangling before the ellipsis");
    }
}

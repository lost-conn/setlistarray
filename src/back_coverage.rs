//! A guard for the fault this card was written to remove, aimed at the next
//! screen rather than at the ones fixed today.
//!
//! Until now the Android Back key did nothing anywhere. Rinch's Android shell
//! maps `AK::Back` to `KeyCode::Escape` and answers every key with
//! `InputStatus::Handled`, so the OS never got its own default handling and
//! this app never acted on it: Back on song detail left you on song detail.
//! The fix is a per-screen registered back action ([`NavStore::register_back`])
//! that the one keyboard interceptor in `crate::app` runs, and the whole reason
//! it is *registered by the screen* rather than decided by a `match` on the
//! route is that Back and the on-screen ← must be the same closure and so
//! cannot drift apart.
//!
//! But a mechanism a screen has to opt into is a mechanism a screen can forget,
//! and forgetting is invisible on a laptop. A screen that never calls
//! `register_back` gets the "nothing registered" answer, which on this app's
//! rules means *exit the app* — so the failure mode is not a Back that does
//! nothing, it is a Back that closes the book from three screens deep. Nothing
//! about writing that screen, reviewing it, or running it on the desktop would
//! say so; only a phone would, and only if somebody thought to press Back on
//! that particular screen.
//!
//! So this is card K15's rule (anything found on hardware becomes a `cargo
//! test` that fails on a laptop) applied to a key a laptop does have: every
//! screen module that draws something a person would press to go back must also
//! register what going back means.
//!
//! ## What counts as "draws something a person would press to go back"
//!
//! Three glyphs, and they are the three this app's chrome actually uses for it:
//!
//! * [`TablerIcon::ChevronLeft`] — the ← every secondary screen wears at the
//!   same 18px inset (`song_detail`, `setlist_detail`, `settings`, `search`).
//! * [`TablerIcon::ArrowLeft`] — the same job in the two full-screen dark-chrome
//!   screens, where the heavier arrow reads better against a chart
//!   (`attachment_viewer`, `capture`).
//! * [`TablerIcon::X`] — the ✕ the form-shaped screens wear instead, because
//!   what they do is abandon rather than go back (`song_form`, `chart_editor`,
//!   and performance mode's own close button).
//!
//! ## What this scan can and cannot see
//!
//! It is deliberately coarse: it asks *per module*, not per glyph. That is not
//! laziness, it is the honest limit of reading source text, and it is worth
//! spelling out because the same three glyph names are used in this tree for
//! things that are emphatically not back controls:
//!
//! * `performance.rs` draws `ChevronLeft` as the previous-song chevron over the
//!   chart, and `attachment_viewer.rs` draws it as the previous-page button in
//!   its bottom bar. Neither goes back anywhere.
//! * `search.rs` draws `X` as the clear-the-query button inside the field, which
//!   deliberately does *not* leave the screen.
//! * `setlists.rs` draws `X` as the cancel button on a setlist card's inline
//!   rename field.
//!
//! A scan over names cannot tell those from a header ←. What it can say is that
//! a module containing any of them and containing no registration at all is
//! either a dead end or a false alarm, and that both of those are worth a human
//! deciding on once, in writing. The false alarms live in [`NO_BACK_ACTION`]
//! with their reasons, the same way `crate::gesture_reachability`'s unreachable
//! handlers do, and a stale exemption fails just as loudly as a missing
//! registration — see the second test.
//!
//! Three more things it cannot see, stated so that nobody reads a green bar as
//! more than it is:
//!
//! * **Whether the registered action matches the control.** A screen could
//!   register `nav.back()` and wire its ← to something else entirely and this
//!   would pass. The defence against that is not a scan, it is that every screen
//!   here hands `register_back` the *same closure* its control already carries
//!   (`cancel`, `close`, `back`) — see each screen's note.
//! * **Whether the screen is reachable, or the control is tappable.** Card K22's
//!   fault was a handler that ran nowhere because a `position: fixed` backdrop
//!   sat over it, and card K23's was a sheet that opened with no frame clock to
//!   animate it. Neither is a name a scan can read.
//! * **Whether Back arrives at all.** That is the shell's business, and the only
//!   proof of it is a device. See this card's report.
//!
//! It scans `src/screens/` and nothing else, because that is where every screen
//! in this app lives — the two roots included — and a back control drawn
//! anywhere else would be a control belonging to no screen.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::gesture_reachability::mask;

/// The glyph names this app uses for a control whose job is "leave this
/// screen". A module mentioning any of them in code is a module this guard has
/// an opinion about.
const BACK_GLYPHS: &[&str] = &[
    "TablerIcon::ChevronLeft",
    "TablerIcon::ArrowLeft",
    "TablerIcon::X",
];

/// What a module has to contain to have opted in.
const REGISTRATION: &str = "register_back(";

/// Screen modules that draw one of [`BACK_GLYPHS`] and register nothing, with
/// the reason each is right to. Anything not on this list has to register.
///
/// Both entries here are the same shape and it is the shape to be suspicious
/// of: a tab root. The roots are where Back stops meaning "go back" and starts
/// meaning "leave the app", which is Android's own default at the root of a
/// task, so registering an action on one would be actively wrong rather than
/// merely unnecessary — it would make the app impossible to leave with the key
/// the platform expects to leave it with.
const NO_BACK_ACTION: &[(&str, &str)] = &[(
    "setlists.rs",
    "a tab root — Back leaves the app here, as Android's own default does. \
     The `TablerIcon::X` it draws is the cancel button on a setlist card's \
     inline rename field, and `NavStore::press_back` closes that field itself \
     (`Sheet::RenameSetlist`) before it ever gets as far as asking this screen.",
)];

/// The screen modules, by file name, that mention a back glyph in code —
/// comments and string literals masked out first, so prose about `TablerIcon::X`
/// does not count as drawing one.
fn modules_drawing_a_back_control() -> BTreeSet<String> {
    screen_modules()
        .into_iter()
        .filter(|path| {
            let masked = mask(&read(path));
            BACK_GLYPHS.iter().any(|glyph| masked.contains(glyph))
        })
        .map(file_name)
        .collect()
}

/// The screen modules that call [`NavStore::register_back`](crate::store::NavStore::register_back),
/// by file name. Masked for the same reason: this file's own module doc names
/// the function repeatedly and is not a registration.
fn modules_registering_a_back_action() -> BTreeSet<String> {
    screen_modules()
        .into_iter()
        .filter(|path| mask(&read(path)).contains(REGISTRATION))
        .map(file_name)
        .collect()
}

fn screen_modules() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/screens");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the crate's own screens directory")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .filter(|path| path.file_name().is_some_and(|n| n != "mod.rs"))
        .collect();
    files.sort();
    files
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("a source file this crate compiles")
}

fn file_name(path: PathBuf) -> String {
    path.file_name()
        .expect("a file, not a directory")
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guard: a screen that draws a way back has to say what back means.
    ///
    /// The failure message names the module and says both ways out, because
    /// both are legitimate answers and which one is right is a design question
    /// rather than a mechanical one.
    #[test]
    fn every_screen_that_draws_a_back_control_registers_a_back_action() {
        let exempt: BTreeSet<&str> = NO_BACK_ACTION.iter().map(|(name, _)| *name).collect();
        let registered = modules_registering_a_back_action();

        let missing: Vec<String> = modules_drawing_a_back_control()
            .into_iter()
            .filter(|name| !registered.contains(name) && !exempt.contains(name.as_str()))
            .map(|name| {
                format!(
                    "src/screens/{name} draws a back control and never calls \
                     `nav.register_back(...)`, so the Android Back key on it would fall \
                     through to `NothingLeftToDoButExit` and close the app from inside a \
                     screen. Hand `register_back` the same closure the control's `onclick` \
                     already carries — or, if this really is a place Back should leave the \
                     app, add a row to NO_BACK_ACTION in src/back_coverage.rs saying why."
                )
            })
            .collect();

        assert!(
            missing.is_empty(),
            "{} screen(s) can become a dead end on a phone:\n{}",
            missing.len(),
            missing.join("\n")
        );
    }

    /// The floor under the guard: an exemption that has stopped being true has
    /// to fail, not quietly keep a screen off the list.
    ///
    /// Two ways it can rot. The module may have grown a `register_back` call,
    /// in which case the row is now a lie about what the file does; or it may
    /// have stopped drawing a back glyph at all, in which case the row is
    /// exempting a module the guard was never going to look at. Both mean the
    /// reasoning in `NO_BACK_ACTION` no longer matches the tree, and the fix
    /// for either is to delete the row.
    #[test]
    fn no_exemption_outlives_the_reason_it_was_written_for() {
        let drawing = modules_drawing_a_back_control();
        let registered = modules_registering_a_back_action();

        for (name, reason) in NO_BACK_ACTION {
            assert!(
                drawing.contains(*name),
                "src/screens/{name} is exempted from the back-action guard \
                 (\"{reason}\") but no longer draws a back control at all — \
                 delete the row rather than leaving a rule about a file that has \
                 moved on"
            );
            assert!(
                !registered.contains(*name),
                "src/screens/{name} is exempted from the back-action guard \
                 (\"{reason}\") and yet registers one — the exemption is now a \
                 false statement about the file; delete the row"
            );
        }
    }

    /// That the mask is doing its job, pinned on the one place in this tree
    /// where it currently makes a difference.
    ///
    /// `chart_editor.rs` draws exactly one back control — the ✕ in its header —
    /// and also *talks about* `TablerIcon::X` in a comment further down, next to
    /// the sentence K13 rewrote to name the glyph in words rather than print it.
    /// Unmasked, that file has two mentions. Nothing about the module-level
    /// question above can tell the difference, so this is where the masking is
    /// checked instead: if `mask` ever regresses, the count below goes to two
    /// and this test says so before a future scan that *does* count sites
    /// inherits the fault silently.
    #[test]
    fn prose_about_a_glyph_is_not_a_glyph() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/screens/chart_editor.rs");
        let raw = read(&path);
        let masked = mask(&raw);

        let count = |text: &str| text.matches("TablerIcon::X").count();

        assert_eq!(
            count(&raw),
            2,
            "chart_editor.rs no longer has one drawn ✕ and one written about — \
             this test's premise has moved, so re-read the file and re-pin it"
        );
        assert_eq!(
            count(&masked),
            1,
            "masking stopped removing comments: chart_editor.rs's prose about \
             `TablerIcon::X` is being counted as a drawn control"
        );
    }
}

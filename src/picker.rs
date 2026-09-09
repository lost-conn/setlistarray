//! Choosing a file the app does not own, and handing one back — card K4's
//! seam. D3 built the pick half; this file's other half, `save`, is K4's own
//! work and is documented in full further down, under "The save half".
//!
//! Two platforms, two entirely different mechanisms. On the desktop `rfd`
//! (through `rinch::dialogs`, behind rinch's `file-dialogs` feature) puts up a
//! modal dialog, blocks, and hands back a [`PathBuf`](std::path::PathBuf). On
//! Android `rinch_android::file_picker::pick_file` fires an
//! `ACTION_OPEN_DOCUMENT` intent at another app entirely, this process is
//! backgrounded, and the answer arrives frames later as a `content://` URI
//! through `drain_activity_results` on the main loop.
//!
//! ## Why a trait, when `platform.rs` is two `#[cfg]` functions
//!
//! [`crate::platform`] is the precedent for a platform seam here and this file
//! keeps its shape — a plain [`pick`] function at the top, `#[cfg]` picking one
//! of two implementations underneath, and nothing above this module knowing
//! which platform it is on. What it adds is a trait, for one reason:
//! `safe_area()` *returns a number* and a test can simply call it, whereas a
//! picker *does something* — it opens a dialog, or hands control to another
//! app — and a test can do neither. So the seam has to be substitutable, which
//! means a trait and a third implementation ([`test_support::Canned`]) that
//! answers without a dialog. The two real ones are still chosen by `#[cfg]`,
//! exactly as `safe_area` is.
//!
//! ## Why the answer comes back through a callback
//!
//! Because Android cannot give it any other way. `pick_file` registers a
//! `FnOnce` against a request code; the SAF activity runs; `onActivityResult`
//! queues the result from the JNI thread; and rinch's Android main loop drains
//! the queue and runs the callback — the next frame at the earliest, and after
//! a resume from the background in practice. There is no value to return.
//!
//! The desktop *could* return one, and an API that did would be nicer to read
//! and impossible to implement twice. So the callback is the shape, and the
//! desktop calls it before [`pick`] returns while Android calls it much later.
//! **A caller has to be correct under both**, which in practice means: assume
//! nothing about what is still on screen. The one thing that makes that safe
//! is that a Rinch `Signal` whose owning scope has been disposed — the user
//! navigated away while the picker was up — is a warn-once no-op on write, not
//! a panic, so a late callback writing to a screen that is gone is harmless.
//!
//! ## Why the currency is bytes and not a path
//!
//! Android hands back `content://com.android.providers…/document/…`. That is
//! not a path, there is no file behind it this process can open, and the only
//! thing `rinch-android` offers is `read_content_uri`, which reads the whole
//! thing into a `Vec<u8>` through the Java `ContentResolver`. A seam whose
//! currency were paths would have nothing to put in one on a phone. So the
//! intersection of the two platforms is *bytes plus a name*, and that is what
//! [`PickedFile`] carries.
//!
//! ## Three things the Android side cannot do, found by reading it
//!
//! **It cannot filter by type.** `RinchActivity.openFilePicker` hard-codes
//! `setType("*/*")` and takes no MIME argument, so [`PickRequest::extensions`]
//! is honoured by the desktop dialog and ignored by the phone. Anything that
//! picks a file must therefore validate the bytes it is given rather than
//! trusting the picker to have filtered — which is why `crate::pdf::import`
//! checks for the PDF header itself and does not look at the file name.
//!
//! **It cannot report a size before reading.** A `content://` URI exposes
//! nothing without a `ContentResolver` query, and no such query is exposed. So
//! the desktop refuses an oversized file from its metadata without opening it,
//! and Android can only refuse after the whole thing is already in memory.
//! Worse, `read_content_uri` holds it three times over on the way — a Java
//! `byte[]`, then a `Vec<i8>`, then the `Vec<u8>` it maps into — so the peak is
//! three times [`PickRequest::max_bytes`] and that ceiling is set with a phone
//! heap in mind rather than a desktop one.
//!
//! **It cannot get the file's name.** The display name of a document lives in
//! `OpenableColumns.DISPLAY_NAME` and needs a `ContentResolver` query that
//! `rinch-android` does not have. All this app is given is the URI, so
//! [`name_from_content_uri`] recovers a name from it where the provider
//! happened to put one there and gives up honestly where it did not.
//!
//! ## The desktop dialog does not live on the desktop's display
//!
//! `rfd` here is the `xdg-portal` backend — `Cargo.lock` has `ashpd` in it and
//! no `gtk3` — so `pick_file` draws nothing. It asks
//! `org.freedesktop.portal.Desktop` over the session bus, and the chooser is
//! drawn by `xdg-desktop-portal-gtk`, which this app did not start and whose
//! environment says `DISPLAY=:0`. `scripts/with-display.sh` moves the app onto
//! a private Xvfb and cannot move that. `PICK_OVERRIDE` is what closes the
//! gap, and its comment is where the argument is.
//!
//! ## The save half
//!
//! K4 pairs picking with saving — the backup export in Phase I (I1, I2) is the
//! first caller, and does not exist yet; this seam is built ahead of it. The
//! desktop half falls straight out: `rinch::dialogs::save_file`, then write
//! the bytes to the path it returns.
//!
//! The Android half needed a capability `rinch-android` did not have.
//! `rinch_android::file_picker::save_file` fires `ACTION_CREATE_DOCUMENT` and
//! hands back the `content://` URI of a document it created *empty* — there
//! was no `write_content_uri` beside `read_content_uri` to put bytes into it.
//! The only `openOutputStream` call in `RinchActivity.java` was buried inside
//! `shareImage`, hard-coded to a MediaStore JPEG it had just inserted itself.
//! `write_content_uri` closes that gap in `rinch-fixes`: the same
//! `openOutputStream` idiom, generalized to any URI a document provider hands
//! back rather than one this app made, and — unlike `shareImage`, which has
//! nothing downstream waiting on a share's outcome — it reports success back
//! rather than swallowing the exception, because a failed save is a failure
//! [`Saved::Failed`] has to be able to show.
//!
//! The shape mirrors the pick half exactly, for the same reasons: a
//! [`SaveRequest`] in, a callback out, because Android cannot answer a save
//! any more synchronously than it can answer a pick — `ACTION_CREATE_DOCUMENT`
//! backgrounds this process and returns through `onActivityResult` just like
//! `ACTION_OPEN_DOCUMENT` does. And the currency is bytes plus a name again,
//! for the same reason: a `content://` URI has no path behind it to write
//! through directly, on either side of the trip.
//!
//! ### What the Android save path still cannot do
//!
//! **It cannot filter or fix the type either.** `RinchActivity.saveFilePicker`
//! hard-codes `setType("*/*")`, exactly as `openFilePicker` does, so
//! [`SaveRequest::extensions`] is decoration on the desktop dialog and inert
//! on the phone. The consequence cuts the other way from the pick side,
//! though: on pick, an ignored filter means the app must validate bytes it is
//! handed; on save, `*/*` means no document provider will append an extension
//! to the suggested name on the app's behalf, so [`SaveRequest::file_name`]
//! has to already carry whatever extension the caller wants — there is no
//! second chance to add one.
//!
//! **It cannot say what the file actually got named.** SAF documents are free
//! to not honour `EXTRA_TITLE` verbatim — a provider can and does append `(1)`
//! to avoid clobbering an existing document — and there is no query back to
//! learn the name that won, only the same opaque `content://` URI the pick
//! side gets. [`name_from_content_uri`] could be pointed at it, but a save
//! confirmation that read "saved as `1000000451`" would be worse than one that
//! just names the file the caller *asked* to save it as, so [`save`] does not
//! try. A caller wanting to tell the user what happened has to use the name it
//! sent, not one read back.
//!
//! **It gives no distinct signal for "you just overwrote something."**
//! `ACTION_CREATE_DOCUMENT` is a create, and if the user steers it onto an
//! existing document, the provider overwrites it as part of granting the
//! write — with whatever confirmation UI *that provider* chooses to show, or
//! none. `write_content_uri` cannot tell a fresh file from a clobbered one and
//! does not pretend to; [`Saved::Done`] means bytes reached the URI, nothing
//! more.
//!
//! One thing it does *not* inherit from the pick side: the size problem.
//! Reading held the file three times over — a Java `byte[]`, a `Vec<i8>`, the
//! `Vec<u8>` it mapped into. Writing is cheaper, because the direction avoids
//! the `i8`/`u8` reinterpretation entirely: `byte_array_from_slice` copies the
//! caller's `&[u8]` straight into a JNI `byte[]`, and `openOutputStream` writes
//! that same array. Peak cost is the original `Vec<u8>` plus one JNI copy of
//! it, not three — still worth keeping in mind for a backup zip that could run
//! into the megabytes, but a real improvement over the read side rather than
//! the same tax paid twice.

/// What the picker is being asked for. `Copy`, so it can cross into the
/// `'static` callback the Android picker holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickRequest {
    /// The dialog's title, on the platform that has a dialog.
    pub title: &'static str,
    /// What to call the file type in the desktop dialog's filter row.
    pub kind: &'static str,
    /// Extensions the desktop dialog filters to. **Ignored on Android** — see
    /// the module header.
    pub extensions: &'static [&'static str],
    /// The most this app will read. See the module header for why the phone's
    /// peak is three times this.
    pub max_bytes: u64,
}

/// A file the user chose, in the only form both platforms can supply it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickedFile {
    /// What to call it, if anything could be recovered. `None` on Android
    /// whenever the provider's URI carries no name — see
    /// [`name_from_content_uri`].
    pub name: Option<String>,
    pub bytes: Vec<u8>,
}

/// What came back. Cancelling is not a failure and does not get an error
/// message: the user closed a dialog, which is a thing they are allowed to do,
/// and a screen that complained about it would be complaining about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picked {
    Chose(PickedFile),
    Cancelled,
    /// Something went wrong, in words that can be shown. Reaching the bytes is
    /// the fallible part — a file that vanished between the dialog and the
    /// read, a URI whose provider refused, a file too large to hold.
    Failed(String),
}

/// What the save picker is being asked for. Not `Copy` like [`PickRequest`] —
/// `file_name` is a suggestion built from the thing being saved (a set's
/// title, a backup's timestamp) and cannot live in a `&'static str` — but it
/// still has to cross into the `'static` callback the Android picker holds,
/// so it is `Clone` and the platforms clone what they need before the dialog
/// or the intent goes up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveRequest {
    /// The dialog's title, on the platform that has a dialog.
    pub title: &'static str,
    /// What to call the file type in the desktop dialog's filter row.
    pub kind: &'static str,
    /// Extensions the desktop dialog filters to, **and the extension this
    /// name had better already carry**, because Android will not add one —
    /// see the module header's "What the Android save path still cannot do".
    pub extensions: &'static [&'static str],
    /// The name to suggest. What the file is actually saved as can differ —
    /// see the module header — so a caller must not treat this as a promise.
    pub file_name: String,
}

/// What came back from a save. There is no payload on success: the bytes
/// were the caller's to begin with, so all a save can tell them is whether
/// they landed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Saved {
    Done,
    /// Same rule as [`Picked::Cancelled`]: the user closing a dialog is not a
    /// failure and gets no message.
    Cancelled,
    /// Something went wrong, in words that can be shown. See the module
    /// header for what a phone can and cannot report about why.
    Failed(String),
}

/// The seam. See the module header for why it is a trait and why the answer
/// arrives through a callback rather than a return value.
pub trait FilePicker {
    /// Ask for a file. `done` runs exactly once — before this returns on the
    /// desktop, some frames later on Android.
    fn pick(&self, request: PickRequest, done: Box<dyn FnOnce(Picked)>);

    /// Ask to save `bytes` somewhere. Same timing rule as `pick`: `done` runs
    /// exactly once, synchronously on the desktop and frames later on
    /// Android.
    fn save(&self, request: SaveRequest, bytes: Vec<u8>, done: Box<dyn FnOnce(Saved)>);
}

/// Ask the platform's picker for a file.
///
/// The one function anything outside this module calls, and the reason no
/// screen contains a `#[cfg]`.
pub fn pick(request: PickRequest, done: impl FnOnce(Picked) + 'static) {
    platform_picker().pick(request, Box::new(done));
}

/// Ask the platform's picker to save `bytes` somewhere.
///
/// The save-half counterpart to [`pick`], and for the same reason the only
/// function outside this module that should ever call `save` on a
/// `FilePicker`.
pub fn save(request: SaveRequest, bytes: Vec<u8>, done: impl FnOnce(Saved) + 'static) {
    platform_picker().save(request, bytes, Box::new(done));
}

#[cfg(target_os = "android")]
fn platform_picker() -> impl FilePicker {
    AndroidPicker
}

#[cfg(not(target_os = "android"))]
fn platform_picker() -> impl FilePicker {
    DesktopPicker
}

/// The sentence a file too large to hold gets. Both implementations refuse at
/// different moments — the desktop from the file's metadata, Android only once
/// the bytes are already in hand — but they refuse with the same words, because
/// what the user did wrong is the same either way.
///
/// `pub(crate)` since the share target (`crate::share`), which is a third
/// moment again: a shared `EXTRA_STREAM` is read the instant it arrives, before
/// any screen exists to complain to, and refusing it there with a *fourth*
/// wording for the same ceiling would mean the same 32 MB file explaining
/// itself differently depending on which door it came in by.
pub(crate) fn too_large(bytes: u64, max_bytes: u64) -> String {
    format!(
        "That file is {}. This app will not open one over {}.",
        crate::model::fmt_bytes(bytes),
        crate::model::fmt_bytes(max_bytes)
    )
}

// ── The desktop: a modal dialog on this thread ──────────────────────────────

/// `rfd`, through `rinch::dialogs`.
///
/// The dialog is **blocking**, and it blocks the thread that draws the app —
/// there is one, and it is this one. So the window behind the dialog does not
/// repaint while it is open. That is the same freeze any modal `rfd` call has
/// and it is accepted here rather than fixed: `rfd`'s async dialog would need
/// an executor and a wake path into the frame loop, for a window nobody is
/// looking at while a file dialog is in front of it.
#[cfg(not(target_os = "android"))]
pub struct DesktopPicker;

#[cfg(not(target_os = "android"))]
impl FilePicker for DesktopPicker {
    fn pick(&self, request: PickRequest, done: Box<dyn FnOnce(Picked)>) {
        // Before the dialog, because on this machine the dialog cannot be put
        // anywhere safe. See `PICK_OVERRIDE`.
        if let Some(answer) = overridden(std::env::var_os(PICK_OVERRIDE), request.max_bytes) {
            done(answer);
            return;
        }
        let chosen = rinch::dialogs::open_file()
            .set_title(request.title)
            .add_filter(request.kind, request.extensions)
            .pick_file();
        let Some(path) = chosen else {
            done(Picked::Cancelled);
            return;
        };
        done(read_from_disk(&path, request.max_bytes));
    }

    fn save(&self, request: SaveRequest, bytes: Vec<u8>, done: Box<dyn FnOnce(Saved)>) {
        // Same reason as `pick`: this machine cannot put the real dialog
        // anywhere safe. See `SAVE_OVERRIDE`.
        if let Some(answer) = overridden_save(std::env::var_os(SAVE_OVERRIDE), &bytes) {
            done(answer);
            return;
        }
        let chosen = rinch::dialogs::save_file()
            .set_title(request.title)
            .set_file_name(&request.file_name)
            .add_filter(request.kind, request.extensions)
            .save();
        let Some(path) = chosen else {
            done(Saved::Cancelled);
            return;
        };
        done(write_to_disk(&path, &bytes));
    }
}

/// The path the dialog would have returned, for something driving the app that
/// has no hands.
///
/// This is not a convenience, it is the only way this half of the card can be
/// verified on this machine, and the reason is worth writing down because
/// `scripts/with-display.sh` looks like it already solves it and does not.
///
/// `rfd` 0.15 on Linux is compiled with its default `xdg-portal` backend —
/// there is no `gtk3` anywhere in `Cargo.lock`, only `ashpd`. So `pick_file`
/// does not open a window. It sends `OpenFile` over the session bus to
/// `org.freedesktop.portal.Desktop`, and the chooser is then drawn by
/// `xdg-desktop-portal-gtk`, **a process this app did not start and does not
/// control**. Read that process's environment on this machine and it says
/// `DISPLAY=:0`, `WAYLAND_DISPLAY=wayland-0`: the developer's session, the
/// developer's screen. Running the app under `DISPLAY=:99` moves the app and
/// not the dialog, because the app's environment was never what decided where
/// the dialog goes.
///
/// That is the missing half of the 2026-08-28 incident CLAUDE.md records. The
/// agent that opened file dialogs on somebody's desktop would have gone on
/// opening them on somebody's desktop from inside Xvfb. So the import path is
/// driven here instead: set `SLA_PICK_FILE` to a path and the desktop picker
/// answers with that file, having opened nothing.
///
/// Set and empty means the user cancelled — the one answer a path cannot spell,
/// and the one a test of "cancelling is not an error" needs.
///
/// The door is desktop-only and it can only ever choose a file the person
/// running the app could have chosen through the dialog themselves, so it takes
/// nothing away from anyone: it reads a file this process could already read,
/// and it does it with the same size ceiling the dialog's answer gets.
#[cfg(not(target_os = "android"))]
const PICK_OVERRIDE: &str = "SLA_PICK_FILE";

/// What the override asks for, or `None` when there is no override and the
/// dialog should go up.
///
/// Split out from the read of the environment so that it can be tested without
/// one: `std::env::set_var` is unsafe and process-wide, and a test that mutated
/// the environment out from under the other 280 running beside it would be
/// buying coverage of four lines with a whole test suite's determinism.
#[cfg(not(target_os = "android"))]
fn overridden(value: Option<std::ffi::OsString>, max_bytes: u64) -> Option<Picked> {
    let value = value?;
    if value.is_empty() {
        return Some(Picked::Cancelled);
    }
    Some(read_from_disk(std::path::Path::new(&value), max_bytes))
}

/// Read a file the dialog named, refusing an oversized one **before** opening
/// it. This is the half Android cannot have: a path has a size that can be
/// asked for without reading anything.
#[cfg(not(target_os = "android"))]
fn read_from_disk(path: &std::path::Path, max_bytes: u64) -> Picked {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > max_bytes => return Picked::Failed(too_large(meta.len(), max_bytes)),
        Ok(_) => {}
        Err(e) => return Picked::Failed(format!("{e}")),
    }
    match std::fs::read(path) {
        Ok(bytes) => Picked::Chose(PickedFile {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|n| !n.is_empty()),
            bytes,
        }),
        Err(e) => Picked::Failed(format!("{e}")),
    }
}

/// The path the save dialog would have returned, for the same reason
/// `PICK_OVERRIDE` exists: `rinch::dialogs::save_file()` is `rfd`'s
/// `xdg-portal` backend exactly as `open_file()` is, so it draws nothing
/// itself and asks `xdg-desktop-portal-gtk` to put a chooser on `DISPLAY=:0`
/// — the developer's screen, not this process's. See `PICK_OVERRIDE`'s
/// comment; every word of it applies here with "save" in place of "open."
///
/// Set `SLA_SAVE_FILE` to a path and the desktop saver writes there, having
/// opened nothing. Set and empty means the user cancelled the dialog before
/// choosing anywhere.
#[cfg(not(target_os = "android"))]
const SAVE_OVERRIDE: &str = "SLA_SAVE_FILE";

/// What the save override asks for, or `None` when there is no override and
/// the dialog should go up. Split out from the environment read for the same
/// determinism reason as [`overridden`].
#[cfg(not(target_os = "android"))]
fn overridden_save(value: Option<std::ffi::OsString>, bytes: &[u8]) -> Option<Saved> {
    let value = value?;
    if value.is_empty() {
        return Some(Saved::Cancelled);
    }
    Some(write_to_disk(std::path::Path::new(&value), bytes))
}

/// Write to the path the dialog named. There is nothing here to refuse in
/// advance the way `read_from_disk` refuses an oversized file — the bytes
/// already exist in memory by the time a save is asked for, so the only
/// question left is whether the write itself succeeds.
#[cfg(not(target_os = "android"))]
fn write_to_disk(path: &std::path::Path, bytes: &[u8]) -> Saved {
    match std::fs::write(path, bytes) {
        Ok(()) => Saved::Done,
        Err(e) => Saved::Failed(format!("{e}")),
    }
}

// ── Android: another app, and a URI to read afterwards ──────────────────────

/// `ACTION_OPEN_DOCUMENT` through `rinch-android`, then `ContentResolver`.
#[cfg(target_os = "android")]
pub struct AndroidPicker;

#[cfg(target_os = "android")]
impl FilePicker for AndroidPicker {
    fn pick(&self, request: PickRequest, done: Box<dyn FnOnce(Picked)>) {
        rinch_android::file_picker::pick_file(move |uri| {
            // `None` is a cancelled picker — `pick_file` maps any result code
            // that is not `RESULT_OK` to it, so a back press and a picker that
            // died arrive identically. There is nothing to tell them apart
            // with and nothing a user would do differently, so both are a
            // cancel.
            let Some(uri) = uri else {
                done(Picked::Cancelled);
                return;
            };
            // The whole file, through Java, three copies deep. See the module
            // header: this is where the size ceiling has to be enforced,
            // because by now it has already cost what it was going to cost.
            match rinch_android::file_picker::read_content_uri(&uri) {
                Err(e) => done(Picked::Failed(e)),
                Ok(bytes) if bytes.len() as u64 > request.max_bytes => {
                    done(Picked::Failed(too_large(bytes.len() as u64, request.max_bytes)))
                }
                Ok(bytes) => done(Picked::Chose(PickedFile {
                    name: name_from_content_uri(&uri),
                    bytes,
                })),
            }
        });
    }

    fn save(&self, request: SaveRequest, bytes: Vec<u8>, done: Box<dyn FnOnce(Saved)>) {
        rinch_android::file_picker::save_file(&request.file_name, move |uri| {
            // Same reasoning as the pick side: a cancelled picker and a
            // picker that died both come back as `None`, and a save has
            // nothing more useful to tell them apart with either.
            let Some(uri) = uri else {
                done(Saved::Cancelled);
                return;
            };
            match rinch_android::file_picker::write_content_uri(&uri, &bytes) {
                Ok(()) => done(Saved::Done),
                Err(e) => done(Saved::Failed(e)),
            }
        });
    }
}

// ── Recovering a name from a `content://` URI ───────────────────────────────

/// The most of a file name a SAF URI is willing to give up, or `None`.
///
/// There is no `DISPLAY_NAME` query to make (module header), so this is
/// scavenging. What the two common providers actually produce:
///
/// ```text
/// content://com.android.externalstorage.documents/document/primary%3ADownload%2Flandslide.pdf
///     → "landslide.pdf"
/// content://com.android.providers.downloads.documents/document/msf%3A1000000123
///     → None
/// ```
///
/// The first has the whole path sitting in the document id, percent-encoded;
/// the second has a database row number and nothing else. So: decode, take the
/// last path segment, then the part after the last `:` (the document id's
/// `authority:path` split), and accept it only if it still looks like a file
/// name — something with an extension on the end. A row number is not a name
/// and returning `"1000000123"` as the title of somebody's chart would be worse
/// than admitting the name is gone, which is what `None` lets
/// `crate::pdf::title_for` do.
///
/// Compiled on both platforms, deliberately: it is the one part of the Android
/// path that can be tested without a phone.
pub fn name_from_content_uri(uri: &str) -> Option<String> {
    // Query and fragment are not part of a name. Neither appears on a SAF URI
    // today, and cutting them costs two `find`s.
    let uri = uri.split(['?', '#']).next().unwrap_or(uri);
    let decoded = percent_decode(uri);
    let candidate = decoded
        .rsplit('/')
        .next()
        .unwrap_or(&decoded)
        .rsplit(':')
        .next()
        .unwrap_or(&decoded)
        .trim();
    looks_like_a_file_name(candidate).then(|| candidate.to_string())
}

/// Whether what was scavenged is worth calling a name: a stem, a dot, and a
/// short alphanumeric extension. `landslide.pdf` yes; `1000000123` no;
/// `4.2` no, because a bare number with a dot in it is a row id that happens to
/// look like a version.
fn looks_like_a_file_name(candidate: &str) -> bool {
    let Some((stem, extension)) = candidate.rsplit_once('.') else {
        return false;
    };
    if stem.is_empty() || stem.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let length = extension.chars().count();
    (1..=8).contains(&length) && extension.chars().all(|c| c.is_ascii_alphanumeric())
}

/// `%XX` back to bytes, then to text.
///
/// Hand-written rather than reached for, because `url` — which this crate
/// already depends on for the capture module — will not parse a `content://`
/// URI into path segments the way it does an `http://` one, and pulling
/// `percent-encoding` in as a direct dependency to decode one string would be a
/// dependency line longer than the function.
///
/// A malformed escape is left exactly as it was found rather than dropped: this
/// text is on its way to being a title, and a mangled name is more use to
/// somebody than a shorter mangled name.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(byte) = hex {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    // A provider that percent-encoded something that is not UTF-8 gets
    // replacement characters rather than an error: this is a display name.
    String::from_utf8_lossy(&out).into_owned()
}

// ── The third implementation, for tests ────────────────────────────────────

/// A picker that answers without a dialog.
///
/// This is the whole reason [`FilePicker`] is a trait rather than a second pair
/// of `#[cfg]` functions: the import path in `crate::pdf` has to be tested
/// against a cancel, a failure and a file, and none of those can be produced by
/// a real picker in a test.
#[cfg(test)]
pub mod test_support {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Hands back a fixed answer, and remembers what it was asked. Holds one
    /// slot for each half of [`FilePicker`] rather than one shared slot,
    /// because a test exercising `pick` has no `Saved` to give and a test
    /// exercising `save` has no `Picked` to give, and a single `Option`
    /// shared between two unrelated answer types would need an enum wrapper
    /// that existed for no reason but this struct.
    pub struct Canned {
        pick_answer: RefCell<Option<Picked>>,
        save_answer: RefCell<Option<Saved>>,
        pub asked: Rc<RefCell<Vec<PickRequest>>>,
        pub saved: Rc<RefCell<Vec<(SaveRequest, Vec<u8>)>>>,
    }

    impl Canned {
        /// A picker whose next `pick` answers with `answer`. Calling `save`
        /// on one built this way panics — build with [`Canned::new_save`]
        /// for a test that needs the other half.
        pub fn new(answer: Picked) -> Self {
            Self {
                pick_answer: RefCell::new(Some(answer)),
                save_answer: RefCell::new(None),
                asked: Rc::new(RefCell::new(Vec::new())),
                saved: Rc::new(RefCell::new(Vec::new())),
            }
        }

        /// A picker whose next `save` answers with `answer`. `pick`'s
        /// counterpart to `new`.
        pub fn new_save(answer: Saved) -> Self {
            Self {
                pick_answer: RefCell::new(None),
                save_answer: RefCell::new(Some(answer)),
                asked: Rc::new(RefCell::new(Vec::new())),
                saved: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    impl FilePicker for Canned {
        fn pick(&self, request: PickRequest, done: Box<dyn FnOnce(Picked)>) {
            self.asked.borrow_mut().push(request);
            let answer = self
                .pick_answer
                .borrow_mut()
                .take()
                .expect("a Canned picker answers a pick once, and only when built to");
            done(answer);
        }

        fn save(&self, request: SaveRequest, bytes: Vec<u8>, done: Box<dyn FnOnce(Saved)>) {
            let answer = self
                .save_answer
                .borrow_mut()
                .take()
                .expect("a Canned picker answers a save once, and only when built to");
            self.saved.borrow_mut().push((request, bytes));
            done(answer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::Canned;
    use super::*;

    const REQUEST: PickRequest = PickRequest {
        title: "Pick a PDF",
        kind: "PDF",
        extensions: &["pdf"],
        max_bytes: 1024,
    };

    // ── the seam ────────────────────────────────────────────────────────────

    #[test]
    fn the_callback_runs_once_and_carries_the_answer() {
        let picker = Canned::new(Picked::Cancelled);
        // An `Rc<RefCell<_>>` stands in for the signal a screen would write:
        // the callback is the only thing that ever learns the answer, so a
        // callback that never ran would be indistinguishable from a cancel if
        // this were not recorded.
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let recorder = std::rc::Rc::clone(&seen);
        picker.pick(
            REQUEST,
            Box::new(move |answer| recorder.borrow_mut().push(answer)),
        );

        assert_eq!(*seen.borrow(), vec![Picked::Cancelled]);
        assert_eq!(picker.asked.borrow().len(), 1, "and the picker was asked once");
        assert_eq!(picker.asked.borrow()[0], REQUEST, "with what it was given");
    }

    #[test]
    fn the_save_callback_runs_once_and_carries_the_answer_and_the_bytes_travel_with_it() {
        let picker = Canned::new_save(Saved::Done);
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let recorder = std::rc::Rc::clone(&seen);
        let request = SaveRequest {
            title: "Save a backup",
            kind: "Zip",
            extensions: &["zip"],
            file_name: "setlistarray-backup.zip".to_string(),
        };
        picker.save(
            request.clone(),
            b"a backup's bytes".to_vec(),
            Box::new(move |answer| recorder.borrow_mut().push(answer)),
        );

        assert_eq!(*seen.borrow(), vec![Saved::Done]);
        assert_eq!(picker.saved.borrow().len(), 1, "and the picker was asked once");
        assert_eq!(
            picker.saved.borrow()[0],
            (request, b"a backup's bytes".to_vec()),
            "with the request and the bytes it was given"
        );
    }

    // ── scavenging a name off a content URI ─────────────────────────────────

    #[test]
    fn a_storage_provider_puts_the_whole_path_in_the_document_id() {
        assert_eq!(
            name_from_content_uri(
                "content://com.android.externalstorage.documents/document/primary%3ADownload%2Flandslide.pdf"
            ),
            Some("landslide.pdf".to_string())
        );
    }

    #[test]
    fn a_downloads_row_number_is_not_a_name_and_says_so() {
        assert_eq!(
            name_from_content_uri(
                "content://com.android.providers.downloads.documents/document/msf%3A1000000123"
            ),
            None
        );
        assert_eq!(name_from_content_uri("content://x/document/12345"), None);
        // A row id that happens to have a dot in it is still a row id.
        assert_eq!(name_from_content_uri("content://x/document/4.2"), None);
    }

    #[test]
    fn a_name_with_spaces_and_accents_survives_the_decode() {
        assert_eq!(
            name_from_content_uri(
                "content://x/document/primary%3ADocs%2FSongs%2FCarolina%20in%20my%20M%C3%ADnd.pdf"
            ),
            Some("Carolina in my Mínd.pdf".to_string())
        );
    }

    #[test]
    fn a_query_string_is_not_part_of_the_name() {
        assert_eq!(
            name_from_content_uri("content://x/document/a%2Fchart.pdf?v=2#top"),
            Some("chart.pdf".to_string())
        );
    }

    #[test]
    fn a_broken_escape_is_kept_verbatim_rather_than_swallowing_the_name() {
        // `%zz` is not hex and `%2` runs off the end. Both stay as typed, and
        // the name is still recognisable.
        assert_eq!(percent_decode("a%zzb"), "a%zzb");
        assert_eq!(percent_decode("chart%2"), "chart%2");
        assert_eq!(percent_decode("%2Fa%2Fb"), "/a/b");
    }

    #[test]
    fn only_something_with_an_extension_counts_as_a_name() {
        assert!(looks_like_a_file_name("chart.pdf"));
        assert!(looks_like_a_file_name("set 1.PDF"));
        assert!(!looks_like_a_file_name("chart"));
        assert!(!looks_like_a_file_name(".pdf"), "an extension with no stem");
        assert!(!looks_like_a_file_name("chart."), "a dot with nothing after it");
        assert!(
            !looks_like_a_file_name("chart.averylongextension"),
            "not an extension, part of a name that ran on"
        );
    }

    // ── the desktop half, without a dialog ──────────────────────────────────
    //
    // `DesktopPicker` had no test at all before the override existed, because
    // every path through it went through a modal chooser first. These reach
    // the part after the chooser — which is the part that can actually be
    // wrong, since a dialog that returns a path has already done the only
    // thing a dialog does.

    /// A scratch file, cleaned up by the test that made it. `crate::db::scratch`
    /// is the same idea for a database directory.
    #[cfg(not(target_os = "android"))]
    fn scratch_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("sla-pick-{}-{name}", std::process::id()));
        std::fs::write(&path, bytes).expect("a scratch file");
        path
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn a_path_the_dialog_named_comes_back_as_bytes_and_a_name() {
        let path = scratch_file("chosen.pdf", b"%PDF-1.4 hello");
        let answer = overridden(Some(path.clone().into_os_string()), REQUEST.max_bytes);
        assert_eq!(
            answer,
            Some(Picked::Chose(PickedFile {
                // The whole file name, extension included: stripping it is
                // `crate::pdf::title_for`'s job and this seam does not know
                // what the bytes are for.
                name: Some(path.file_name().unwrap().to_string_lossy().into_owned()),
                bytes: b"%PDF-1.4 hello".to_vec(),
            }))
        );
        std::fs::remove_file(&path).ok();
    }

    /// The race the card names: the dialog returns a path and the file is gone
    /// by the time it is read. Rare with a chooser and ordinary with a
    /// removable drive, a sync client, or an override pointing at something
    /// that never existed — and the answer has to be a sentence rather than a
    /// panic, because the callback that receives it runs on the frame loop.
    #[cfg(not(target_os = "android"))]
    #[test]
    fn a_file_that_vanished_between_the_dialog_and_the_read_is_a_failure_not_a_panic() {
        let path = scratch_file("vanishing.pdf", b"%PDF-1.4");
        std::fs::remove_file(&path).expect("gone");

        let answer = overridden(Some(path.into_os_string()), REQUEST.max_bytes);
        let Some(Picked::Failed(why)) = answer else {
            panic!("{answer:?}");
        };
        // Whatever the OS called it, said in the OS's words — there is nothing
        // this app knows about the file that would improve on them.
        assert!(!why.is_empty(), "{why}");
    }

    /// Refused from the metadata, so a 4 GB file never becomes 4 GB of this
    /// process. The proof that it was not read is that the ceiling here is 12
    /// bytes and the file is bigger than that with nothing in it worth reading.
    #[cfg(not(target_os = "android"))]
    #[test]
    fn an_oversized_file_is_refused_without_being_opened() {
        let path = scratch_file("oversized.pdf", &vec![b'x'; 4000]);
        let answer = overridden(Some(path.clone().into_os_string()), 12);
        let Some(Picked::Failed(why)) = answer else {
            panic!("{answer:?}");
        };
        assert!(why.contains("4 KB"), "{why}");
        assert!(why.contains("12 B"), "{why}");
        std::fs::remove_file(&path).ok();
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn an_empty_override_is_a_cancel_and_no_override_is_a_dialog() {
        assert_eq!(
            overridden(Some(std::ffi::OsString::new()), REQUEST.max_bytes),
            Some(Picked::Cancelled),
            "set and empty is the only way to spell a cancel in a path"
        );
        assert_eq!(
            overridden(None, REQUEST.max_bytes),
            None,
            "and unset means the dialog goes up, which is the shipped behaviour"
        );
    }

    // ── the desktop save half, without a dialog ─────────────────────────────

    /// A path in the scratch directory that nothing has written to yet — the
    /// save side's counterpart to `scratch_file`, which needs the bytes ahead
    /// of time because reading is the thing under test there. Here, writing
    /// is the thing under test, so the file must not exist beforehand or a
    /// successful write would prove nothing that an already-correct file
    /// didn't already show.
    #[cfg(not(target_os = "android"))]
    fn scratch_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("sla-save-{}-{name}", std::process::id()))
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn a_save_writes_the_bytes_to_the_path_the_dialog_named() {
        let path = scratch_path("backup.zip");
        let answer = overridden_save(Some(path.clone().into_os_string()), b"a zip's bytes");
        assert_eq!(answer, Some(Saved::Done));
        assert_eq!(
            std::fs::read(&path).expect("the write happened"),
            b"a zip's bytes"
        );
        std::fs::remove_file(&path).ok();
    }

    /// The one failure a save can hit that a pick cannot: there is nothing to
    /// refuse in advance, so the only way to see `Saved::Failed` here is to
    /// point the write at somewhere that cannot exist — a path through a
    /// directory this test never created.
    #[cfg(not(target_os = "android"))]
    #[test]
    fn a_write_to_a_directory_that_does_not_exist_is_a_failure_not_a_panic() {
        let path = scratch_path("nowhere").join("backup.zip");
        let answer = overridden_save(Some(path.into_os_string()), b"bytes");
        let Some(Saved::Failed(why)) = answer else {
            panic!("{answer:?}");
        };
        assert!(!why.is_empty(), "{why}");
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn an_empty_save_override_is_a_cancel_and_no_override_is_a_dialog() {
        assert_eq!(
            overridden_save(Some(std::ffi::OsString::new()), b"bytes"),
            Some(Saved::Cancelled),
            "set and empty is the only way to spell a cancel in a path"
        );
        assert_eq!(
            overridden_save(None, b"bytes"),
            None,
            "and unset means the dialog goes up, which is the shipped behaviour"
        );
    }

    // ── the words a refusal wears ───────────────────────────────────────────

    #[test]
    fn a_file_too_large_is_refused_in_sizes_a_person_reads() {
        let message = too_large(41_900_000, crate::pdf::MAX_BYTES);
        assert!(message.contains("41.9 MB"), "{message}");
        assert!(message.contains("32 MB"), "{message}");
    }
}

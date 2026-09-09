//! Being shared *to* — the app as an Android share target.
//!
//! The other half of this feature lives in `rinch-android`
//! (`rinch_android::intent`, landed as its own card) and does the platform
//! work: Java stashes the launch intent in `onCreate`, hands it over once Rust
//! has registered its natives, and delivers everything after that through
//! `onNewIntent`. What arrives here is an [`IncomingIntent`] flattened to owned
//! strings, once a frame, on the main thread. This file is everything the app
//! does with one.
//!
//! ## Why the payload is read into a store instead of onto the route
//!
//! Because [`Route`](crate::store::Route) is `Copy` and every variant on it
//! carries ids and nothing else. `Route::Search`'s own doc comment argues that
//! at length for a `String` query — a route is matched in half a dozen reactive
//! closures a frame and passed by value to `derive::dark_chrome` and
//! `keep_awake::wanted`, and one `String` variant takes that away from every
//! other variant too. A shared PDF is a `Vec<u8>` that can run to 32 MB, so the
//! same argument holds several thousand times over. `Route::SaveShared` carries
//! nothing and [`NavStore::pending_share`](crate::store::NavStore) is what the
//! chooser screen actually reads, exactly as `LibraryViewStore::query` is what
//! `Route::Search` reads.
//!
//! ## Why the bytes are read the moment the share arrives
//!
//! A share's `EXTRA_STREAM` is a `content://` URI belonging to the *sending*
//! app, and what makes it readable is a grant Android attaches to the intent
//! and scopes to this activity. It is not a file path and it is not durable:
//! the sending app can revoke it, and the platform drops it when the activity
//! that received it goes away. So [`read_stream`] runs inside the intent
//! handler — before the chooser screen has even been mounted, let alone before
//! the user has decided which song this belongs to — and what travels onward is
//! bytes, not a URI to dereference later. That is the same shape
//! `picker::AndroidPicker::pick` already has for a *picked* file and for the
//! same reason; see its comment about the whole file going through Java three
//! copies deep, which is also why [`crate::pdf::PICK_REQUEST`]'s ceiling is
//! enforced here rather than at import time.
//!
//! ## What this app will and will not be offered
//!
//! `android/AndroidManifest.xml` declares `ACTION_SEND` over exactly two MIME
//! types, `text/plain` and `application/pdf`, and the manifest comment says why
//! those two and not the obvious third. Everything below is written to that
//! promise: `ACTION_SEND_MULTIPLE` is not declared and is refused here too, and
//! an `ACTION_VIEW` deep link is not something this app asked to be in the
//! chooser for.
//!
//! Text is the interesting half, because "share this text" is one action to
//! Android and two entirely different intentions to a musician. A browser
//! sharing a page sends the URL as `EXTRA_TEXT`; a notes app sharing a verse
//! sends the verse the same way. [`classify`] is where those part company —
//! a URL becomes a capture, anything else becomes a typed chart — and the
//! second case is not a fallback for the first going wrong. Declaring
//! `text/plain` puts this app in the share sheet for *every* text share on the
//! phone, so refusing a lyric share with an error would be the app complaining
//! about a chooser entry it put there itself.
//!
//! [`IncomingIntent`]: https://docs.rs/rinch-android

use crate::model::AttachmentKind;

/// `Intent.ACTION_SEND`, spelled by the crate that receives it.
///
/// Taken from `rinch-android` on the platform that has one so the two can
/// never drift; written out on the desktop, where there is no such crate, so
/// that [`incoming`]'s filter is still a thing a laptop can test. That is the
/// only difference between the platforms in this file — everything below it
/// compiles and runs on both, which is what cards K15 and K20 asked for and
/// what makes the whole of the share *decision* testable without a phone.
#[cfg(target_os = "android")]
pub use rinch_android::intent::ACTION_SEND;

/// See the Android arm above.
#[cfg(not(target_os = "android"))]
pub const ACTION_SEND: &str = "android.intent.action.SEND";

/// The longest a name this module invents is allowed to be.
///
/// The same 40 as `pdf::TITLE_LIMIT` and `chart_editor::TITLE_LIMIT`, and for
/// the reason `pdf`'s own comment gives about those two: a share can mint a
/// song *and* the chart hanging off it, so a fourth producer with a fifth
/// cut-off point would show as a ragged column beside the three that already
/// agree.
const NAME_LIMIT: usize = 40;

/// What the app was handed, once every fallible part of getting it is over.
///
/// The failure is a variant rather than a `Result` around the whole enum
/// because it has to reach the same screen: by the time a `content://` read
/// fails, Android has already launched this app in front of somebody who
/// pressed Share and is now looking at it. A silent return to the library
/// would be the app appearing to have done nothing, which is exactly the
/// complaint that makes people stop using a share target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SharedItem {
    /// Shared text that is a URL — the capture screen's job. See [`classify`].
    Link(String),
    /// Shared text that is not a URL: lyrics, chords, a verse someone typed.
    /// Becomes the `AttachmentKind::Text` attachment `chart_editor` already
    /// writes, body and all.
    Chart(String),
    /// A shared PDF, already read (see the module header on why "already").
    /// `name` is whatever [`crate::picker::name_from_content_uri`] could
    /// scavenge, which for half the providers on a phone is nothing.
    Pdf { name: Option<String>, bytes: std::sync::Arc<Vec<u8>> },
    /// The share arrived and could not be got at, in words that can be shown.
    Unreadable(String),
}

impl SharedItem {
    /// The thumb the chooser draws beside the shared thing, or `None` for a
    /// share there is nothing to save.
    ///
    /// It is the kind the attachment *would* be, which is what makes the top
    /// of the chooser look like the row it is about to become on song detail.
    pub fn kind(&self) -> Option<AttachmentKind> {
        match self {
            SharedItem::Link(_) => Some(AttachmentKind::CapturedPage),
            SharedItem::Chart(_) => Some(AttachmentKind::Text),
            SharedItem::Pdf { .. } => Some(AttachmentKind::Pdf),
            SharedItem::Unreadable(_) => None,
        }
    }

    /// The one line under the chooser's title that says what is being filed.
    ///
    /// Not [`song_name_for`]: this is the *thing*, shown as it arrived, so a
    /// URL stays a URL and a verse stays its first line. The tidied-up name is
    /// what a song minted from it would be called, which is a different
    /// question asked one row further down the screen.
    ///
    /// The failed share gets a sentence of this app's own rather than the one
    /// it is carrying, and that is a decision taken on the phone. A stream this
    /// app cannot open comes back from the framework as
    /// `readContentUri returned null (IO error or invalid URI)`, and set in the
    /// chooser's title face at 19px it is the largest thing on a screen
    /// somebody arrived at from another app's share sheet. The reason is not
    /// thrown away — [`trouble`](Self::trouble) is where it goes, in the small
    /// grey line where this app puts every other thing a developer wants and a
    /// musician does not.
    pub fn headline(&self) -> String {
        match self {
            SharedItem::Link(url) => url.clone(),
            SharedItem::Chart(body) => first_line(body).unwrap_or_default(),
            SharedItem::Pdf { name, .. } => name.clone().unwrap_or_else(|| "PDF".to_string()),
            SharedItem::Unreadable(_) => "That could not be read.".to_string(),
        }
    }

    /// The small grey line under the chooser's headline: what keeping this
    /// will actually get you.
    ///
    /// A sentence per variant rather than [`AttachmentKind::descriptor`],
    /// which was what this used first and which reads as written for a
    /// different sentence than this one — because it was. `descriptor` is the
    /// qualifier after a name in a collapsed row on song detail (`chart · pdf`,
    /// `Wonderwall · saved page`), where the noun in front of it is doing the
    /// work. Borrowed into a sentence of its own it came out on the phone as
    /// *"will be saved as a saved page"* and *"will be saved as a typed"*: one
    /// stutter and one sentence with no noun in it. Two words from two screens
    /// that happen to be about the same three kinds of thing are not the same
    /// two words.
    ///
    /// The link's line says *no signal* rather than *offline* on purpose: it is
    /// the promise the capture screen already makes in those words two taps
    /// later ("714 B on this device · will work with no signal"), and this is
    /// the screen where somebody decides whether that is worth a tap.
    pub fn destination(&self) -> &'static str {
        match self {
            SharedItem::Link(_) => "kept as a page you can read with no signal",
            SharedItem::Chart(_) => "kept as a typed chart",
            SharedItem::Pdf { .. } => "kept as a PDF",
            SharedItem::Unreadable(_) => "nothing this app can save",
        }
    }

    /// Why a share could not be saved, in whatever words the thing that
    /// refused it used. `None` for every share that is fine.
    pub fn trouble(&self) -> Option<&str> {
        match self {
            SharedItem::Unreadable(why) => Some(why),
            _ => None,
        }
    }

    /// Whether there is anything here to put in the book. `false` for exactly
    /// one variant, and the chooser draws no song list at all when it is.
    pub fn savable(&self) -> bool {
        !matches!(self, SharedItem::Unreadable(_))
    }
}

/// A link handed from the chooser to the capture screen.
///
/// Two fields and they answer two different questions. `url` is what
/// `CaptureScreen`'s address field starts out holding, which is the whole of
/// "prefill it"; `placeholder` is the name a song *minted for this share* was
/// given, and it is `None` when the user picked a song that already existed.
///
/// The second one exists because of what a link share can and cannot know at
/// the moment it happens. Minting the song needs a name immediately and the
/// only thing to hand is the URL's last path segment — `free-fallin-chords`,
/// which is close enough to file under and not what anybody would have typed.
/// The page's real `<title>` arrives a network round trip later, inside
/// `CapturedPage::title`, long after the chooser has gone. So the rename is
/// carried to the screen that will learn the answer, and
/// [`rename_to_captured_title`] is the rule it applies when it does — including
/// the refusal to touch a name the user has meanwhile changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedCapture {
    pub url: String,
    pub placeholder: Option<String>,
}

/// What an intent means to this app *before* anything fallible has happened.
///
/// Split out from the reading so that the whole of the decision — which
/// actions count, which of `EXTRA_TEXT` and `EXTRA_STREAM` wins when both are
/// there, what a `text/plain` stream is — can be exercised on a laptop, which
/// is the same reason `crate::picker::name_from_content_uri` is compiled on
/// both platforms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Incoming {
    /// `EXTRA_TEXT`, still to be told apart by [`classify`].
    Text(String),
    /// `EXTRA_STREAM`: a `content://` URI whose bytes have to be read now, and
    /// the MIME type the sender declared, which is what decides whether they
    /// are a PDF or a text file.
    Stream { uri: String, mime_type: Option<String> },
}

/// What to do with an intent, or `None` for one this app has nothing to say
/// about.
///
/// **The stream wins when both are present**, and that is not arbitrary. An
/// app sharing a file frequently attaches a caption as well — a Drive share
/// puts the document's URL in `EXTRA_TEXT` beside the `EXTRA_STREAM` holding
/// the file itself — and in every one of those pairs the file is the thing the
/// user meant to send and the text is a description of it. Taking the text
/// would file a link to a PDF the app was holding the bytes of.
///
/// **`ACTION_SEND` and nothing else.** `ACTION_SEND_MULTIPLE` is not declared
/// in the manifest, so it should never arrive; it is refused here anyway,
/// because "the manifest does not ask for it" and "the app cannot be sent it"
/// are different claims — an `am start` can send anything, and so can any app
/// holding this activity's component name.
pub fn incoming(
    action: &str,
    mime_type: Option<&str>,
    text: Option<&str>,
    stream_uris: &[String],
) -> Option<Incoming> {
    if action != ACTION_SEND {
        return None;
    }
    if let Some(uri) = stream_uris.first() {
        return Some(Incoming::Stream {
            uri: uri.clone(),
            mime_type: mime_type.map(str::to_string),
        });
    }
    let text = text.unwrap_or_default();
    if text.trim().is_empty() {
        return None;
    }
    Some(Incoming::Text(text.to_string()))
}

/// A URL becomes a capture; anything else becomes a typed chart.
///
/// The whole of the text half of this feature, and the one place the two
/// meanings of "share this text" are told apart. See [`shared_url`] for what
/// counts as a URL and — more to the point — what deliberately does not.
pub fn classify(text: &str) -> SharedItem {
    match shared_url(text) {
        Some(url) => SharedItem::Link(url.to_string()),
        None => SharedItem::Chart(text.to_string()),
    }
}

/// The shared text as a URL, or `None` because it is prose.
///
/// **The whole of the text has to be the URL.** A share reading
/// `Wonderwall (Oasis) chords\nhttps://tabs.example.com/wonderwall` — which is
/// what several apps send, the title above the link — comes back `None` and is
/// filed as a typed chart containing a URL, which is not what its sender meant.
/// That case is left alone on purpose: pulling a link out of surrounding text
/// means deciding *which* link and what to do with the words around it, and
/// every rule for that is a guess about an app this project has never seen.
/// Filing the share as text loses nothing — the text is kept whole, the link is
/// in it, and the user can capture it in two more taps — whereas a rule that
/// guessed wrong would throw the words away and capture the wrong page.
///
/// A scheme is not required, because `capture::capture` already assumes https
/// for a URL without one (its own comment says why https and not http), so
/// insisting on one here would refuse a share the capture screen would have
/// accepted from the field. What *is* required without a scheme is a path or a
/// `www.` — otherwise `Chords.pdf` typed into a notes app is a hostname with a
/// `pdf` TLD, and this app would try to download somebody's lyrics.
pub fn shared_url(text: &str) -> Option<&str> {
    let text = text.trim();
    // One whitespace-free token. Anything else is prose that contains a URL at
    // best, which is the case the doc comment above declines to guess at.
    if text.is_empty() || text.split_whitespace().count() != 1 {
        return None;
    }

    let (rest, had_scheme) = match text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
    {
        Some(rest) => (rest, true),
        None => {
            // Some other scheme — `mailto:`, `tel:`, `content://`, `file://`.
            // A scheme has no dot in it and a host always does, which is what
            // tells `example.com:8080/x` from `mailto:someone@example.com`
            // without needing to know either name.
            if let Some((before, _)) = text.split_once(':')
                && !before.contains('.')
                && !before.contains('/')
                && !before.is_empty()
            {
                return None;
            }
            (text, false)
        }
    };

    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, Some(path)),
        None => (rest, None),
    };
    // A port is not part of the name being judged below.
    let host = match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => host,
        _ => authority,
    };
    if !looks_like_host(host) {
        return None;
    }
    if !had_scheme && path.is_none() && !host.starts_with("www.") {
        return None;
    }
    Some(text)
}

/// Whether a bare authority reads as a hostname: labelled, dotted, and ending
/// in something that could be a TLD.
///
/// The last test is what earns this function its keep. Every string with a dot
/// in it looks like a host to a naive check — `2.5`, `Fig. 3`, `chorus.txt` —
/// and a TLD is the one part of a hostname with a shape: letters, at least two
/// of them. It is not a list of real TLDs, deliberately; a list would have to
/// be kept, and being wrong about a new one costs a share.
fn looks_like_host(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 || labels.iter().any(|label| label.is_empty()) {
        return false;
    }
    let last = labels[labels.len() - 1];
    last.chars().count() >= 2 && last.chars().all(|c| c.is_ascii_alphabetic())
}

/// What a song minted for this share is called to begin with.
///
/// A song has to have a name the instant `SongsStore::add` mints it, and the
/// only thing to hand at that moment is the share itself. So: the URL's last
/// path segment, the PDF's file name, the first line of the verse. None of the
/// three is what a person would have typed, and none of them has to be — the
/// name is in the library where it can be edited, and for a link it is
/// [`rename_to_captured_title`]'s job to replace it with the page's own title
/// as soon as the fetch knows one.
///
/// The fallbacks name what happened rather than what was shared, which is the
/// honest answer when the share carried no name at all: a Downloads-provider
/// URI is a database row number (see `picker::name_from_content_uri`), and
/// `1000000123` at the top of somebody's book would be worse than saying
/// "Shared PDF" and letting them rename it.
pub fn song_name_for(item: &SharedItem) -> String {
    match item {
        SharedItem::Link(url) => name_from_url(url).unwrap_or_else(|| "Shared link".to_string()),
        SharedItem::Chart(body) => first_line(body)
            .and_then(|line| tidy(&line))
            .unwrap_or_else(|| "Shared text".to_string()),
        SharedItem::Pdf { name, .. } => name
            .as_deref()
            .and_then(|name| tidy(drop_extension(name)))
            .unwrap_or_else(|| "Shared PDF".to_string()),
        // Nothing mints a song from one of these — the chooser draws no
        // `+ New song` row for a share it cannot save — but a `match` that
        // cannot be called is still a `match` somebody will read.
        SharedItem::Unreadable(_) => "Shared file".to_string(),
    }
}

/// The last path segment of a URL, made into something to call a song.
///
/// `https://tabs.example.com/tabs/free-fallin-chords.html` is
/// `free fallin chords`: query and fragment dropped, trailing slashes ignored,
/// the extension dropped the way `pdf::title_for` drops one, and the separators
/// a URL uses in place of spaces turned back into spaces. A URL with no path at
/// all falls back to the host without its `www.`, because
/// `https://hymnal.net` should file as `hymnal.net` rather than as the generic
/// sentence.
fn name_from_url(url: &str) -> Option<String> {
    // A query string and a fragment are addressing, not naming: `?print=1` on
    // the end of a chart URL says how to serve the page and would read as part
    // of the song's title.
    let trimmed = url.split(['?', '#']).next().unwrap_or(url);
    let after_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let (authority, path) = match after_scheme.split_once('/') {
        Some((authority, path)) => (authority, path),
        None => (after_scheme, ""),
    };

    let segment = path.split('/').filter(|s| !s.is_empty()).next_back();
    if let Some(segment) = segment
        && let Some(name) = tidy(drop_extension(&percent_decoded(segment)))
    {
        return Some(name);
    }
    let host = authority
        .rsplit_once(':')
        .filter(|(_, port)| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()))
        .map_or(authority, |(host, _)| host);
    tidy_keeping_dots(host.strip_prefix("www.").unwrap_or(host))
}

/// `%20` and its friends, decoded, with anything malformed left as it stands.
///
/// Path segments are percent-encoded and a chart URL really does contain them
/// — `Free%20Fallin.html` is an ordinary thing for a site to serve — so a name
/// taken from one without this reads `Free%20Fallin`, which is not a name.
/// Invalid UTF-8 comes back lossily rather than failing: half a decoded name is
/// still better than a percent sign.
fn percent_decoded(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The name without its extension, by the same rule `pdf::title_for` applies
/// and for the same reason — one to eight alphanumerics after the last dot is
/// an extension, and `Mr. Jones` keeps its full stop.
fn drop_extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, extension))
            if !stem.is_empty()
                && (1..=8).contains(&extension.chars().count())
                && extension.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            stem
        }
        _ => name,
    }
}

/// Separators to spaces, whitespace collapsed, cut at [`NAME_LIMIT`], `None`
/// when there is nothing left. The `-`/`_`/`+` substitution is what turns a
/// URL slug back into words; everything after it is `pdf::title_for`'s
/// treatment, cut point and ellipsis included.
fn tidy(raw: &str) -> Option<String> {
    tidy_keeping_dots(&raw.replace(['-', '_', '+'], " "))
}

/// [`tidy`] without the separator substitution, for the one caller that must
/// not have it: a hostname's dots and hyphens are part of the name.
fn tidy_keeping_dots(raw: &str) -> Option<String> {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    if collapsed.chars().count() <= NAME_LIMIT {
        return Some(collapsed);
    }
    let cut: String = collapsed.chars().take(NAME_LIMIT).collect();
    Some(format!("{}…", cut.trim_end()))
}

/// The first line with anything on it, trimmed.
fn first_line(body: &str) -> Option<String> {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

/// The new name for a song minted from a shared link, once the capture knows
/// the page's own title — or `None` for every reason not to rename it.
///
/// Three refusals, and each is a case that happens:
///
/// * **The song was not minted for this share.** `placeholder` is `None` when
///   the user filed the link under a song that already existed, and renaming
///   somebody's `Landslide` to whatever the page called itself is the worst
///   thing this feature could do.
/// * **The name is not the one that was minted.** The chooser mints, then the
///   capture screen sits there while a fetch runs, and in between the user can
///   go and edit the song. A name that no longer matches is a name somebody
///   chose, and it wins.
/// * **The page had no title worth having.** An empty `<title>`, or one that
///   tidies down to nothing, leaves the slug — which is at least derived from
///   something the user pressed Share on.
pub fn rename_to_captured_title(
    current: &str,
    placeholder: Option<&str>,
    captured: &str,
) -> Option<String> {
    if placeholder? != current {
        return None;
    }
    let wanted = tidy_keeping_dots(captured)?;
    (wanted != current).then_some(wanted)
}

// ── Android: the registration, and the one fallible read ────────────────────

/// Start listening for shares. Called once, from `crate::app`, on Android only.
///
/// Registering here rather than from the chooser screen is not a detail: the
/// launch intent — the share that started the app — is queued by
/// `rinch_android::intent` *before any app code runs at all*, and its drain
/// deliberately keeps intents that arrive before a handler exists rather than
/// discarding them (see that module's header). So the handler must be
/// registered by something that exists on the first frame, and the screen this
/// share is about to open does not.
#[cfg(target_os = "android")]
pub fn listen(nav: crate::store::NavStore) {
    rinch_android::intent::on_incoming_intent(move |intent| {
        let Some(incoming) = incoming(
            &intent.action,
            intent.mime_type.as_deref(),
            intent.text.as_deref(),
            &intent.stream_uris,
        ) else {
            // Not a share, or a share with nothing in it. Every app with a
            // handler is offered every intent, so this is the ordinary case
            // rather than a fault, and the app carries on wherever it was.
            return;
        };
        let item = match incoming {
            Incoming::Text(text) => classify(&text),
            Incoming::Stream { uri, mime_type } => read_stream(&uri, mime_type.as_deref()),
        };
        nav.pending_share.set(Some(item));
        nav.go(crate::store::Route::SaveShared);
    });
}

/// Read what `EXTRA_STREAM` pointed at, now, while the grant still holds.
///
/// The module header is the long version of why this is not deferred. The
/// ceiling is `pdf::PICK_REQUEST.max_bytes` and it is checked with the same
/// sentence the file picker refuses an oversized file with, because what the
/// user did is the same thing either way — and because
/// `read_content_uri` has already spent the peak heap by the time it returns,
/// so this is a refusal to *keep* the bytes rather than a refusal to read
/// them. `picker::AndroidPicker::pick` says the same about the same call.
///
/// A `text/*` stream is a real share and not an exotic one: sharing a `.txt`
/// out of a file manager sends `text/plain` with a stream and no `EXTRA_TEXT`,
/// and the manifest asks for `text/plain`, so refusing it would be this app
/// putting itself in a chooser it then walks out of. The bytes are decoded
/// lossily and go down the same road a pasted verse does.
#[cfg(target_os = "android")]
fn read_stream(uri: &str, mime_type: Option<&str>) -> SharedItem {
    let max = crate::pdf::PICK_REQUEST.max_bytes;
    let bytes = match rinch_android::file_picker::read_content_uri(uri) {
        Ok(bytes) => bytes,
        Err(e) => return SharedItem::Unreadable(e),
    };
    if bytes.len() as u64 > max {
        return SharedItem::Unreadable(crate::picker::too_large(bytes.len() as u64, max));
    }
    if mime_type.is_some_and(|mime| mime.starts_with("text/")) {
        return classify(&String::from_utf8_lossy(&bytes));
    }
    SharedItem::Pdf {
        name: crate::picker::name_from_content_uri(uri),
        bytes: std::sync::Arc::new(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pdf(name: &str) -> SharedItem {
        SharedItem::Pdf {
            name: Some(name.to_string()),
            bytes: std::sync::Arc::new(Vec::new()),
        }
    }

    /// The action filter, from both ends. `SEND_MULTIPLE` is the one worth
    /// pinning: it is not in the manifest, so it should never arrive, and an
    /// `am start` can send it anyway.
    #[test]
    fn only_an_action_send_is_a_share_this_app_answers() {
        assert_eq!(
            incoming(ACTION_SEND, Some("text/plain"), Some("hello"), &[]),
            Some(Incoming::Text("hello".to_string()))
        );
        assert_eq!(
            incoming(
                "android.intent.action.SEND_MULTIPLE",
                Some("application/pdf"),
                None,
                &["content://a".to_string(), "content://b".to_string()],
            ),
            None,
            "SEND_MULTIPLE is out of scope and is not declared in the manifest"
        );
        assert_eq!(
            incoming("android.intent.action.VIEW", None, None, &[]),
            None,
            "this app deliberately does not appear in the browser's open-with chooser"
        );
        assert_eq!(
            incoming(ACTION_SEND, Some("text/plain"), Some("   \n "), &[]),
            None,
            "a share of nothing at all is not a share"
        );
    }

    /// A file share with a caption files the file. See [`incoming`]'s own note:
    /// a Drive share sends the document's URL in `EXTRA_TEXT` beside the
    /// stream, and taking the text would file a link to a PDF already in hand.
    #[test]
    fn a_stream_beats_a_caption_when_a_share_carries_both() {
        assert_eq!(
            incoming(
                ACTION_SEND,
                Some("application/pdf"),
                Some("https://drive.example.com/file/d/123"),
                &["content://media/external/file/9".to_string()],
            ),
            Some(Incoming::Stream {
                uri: "content://media/external/file/9".to_string(),
                mime_type: Some("application/pdf".to_string()),
            })
        );
    }

    /// The decision the whole text half of this feature turns on.
    #[test]
    fn a_shared_url_becomes_a_capture_and_shared_words_become_a_chart() {
        for url in [
            "https://tabs.example.com/wonderwall",
            "http://hymnal.net/en/hymn/h/123",
            "tabs.example.com/wonderwall",
            "www.hymnal.net",
            "https://tabs.example.com:8443/wonderwall?key=G#chorus",
        ] {
            assert_eq!(
                classify(url),
                SharedItem::Link(url.to_string()),
                "{url} is a link somebody shared to be captured"
            );
        }

        for words in [
            // The case this arm exists for: declaring `text/plain` puts this
            // app in the sheet for every text share on the phone.
            "Am  C  G\nAnd if you go, I would still remember",
            // Prose with a link in it. Kept whole rather than guessed at —
            // see `shared_url`'s doc comment.
            "Wonderwall chords\nhttps://tabs.example.com/wonderwall",
            "mailto:someone@example.com",
            "content://com.android.providers.downloads.documents/document/msf%3A1",
            // A bare hostname with no path is not something anybody shares,
            // and a file name is exactly something somebody types.
            "Chords.pdf",
            "2.5",
            "Landslide",
        ] {
            assert!(
                matches!(classify(words), SharedItem::Chart(_)),
                "{words:?} is not a URL and must be kept as a typed chart"
            );
        }
    }

    /// The name a minted song starts life with, per kind of share.
    #[test]
    fn a_song_minted_for_a_share_is_named_from_the_share() {
        assert_eq!(
            song_name_for(&SharedItem::Link(
                "https://tabs.example.com/tabs/free-fallin-chords.html".to_string()
            )),
            "free fallin chords"
        );
        assert_eq!(
            song_name_for(&SharedItem::Link(
                "https://tabs.example.com/tabs/Free%20Fallin.htm?print=1".to_string()
            )),
            "Free Fallin",
            "a percent-encoded space is a space, not three characters of name"
        );
        assert_eq!(
            song_name_for(&SharedItem::Link("https://hymnal.net/".to_string())),
            "hymnal.net",
            "a URL with no path at all still says where it came from"
        );
        assert_eq!(
            song_name_for(&SharedItem::Link("https://www.hymnal.net".to_string())),
            "hymnal.net",
            "and the `www.` is not part of it"
        );

        assert_eq!(song_name_for(&pdf("landslide.pdf")), "landslide");
        assert_eq!(song_name_for(&pdf("Real_Book_Autumn Leaves.pdf")), "Real Book Autumn Leaves");
        assert_eq!(
            song_name_for(&SharedItem::Pdf { name: None, bytes: std::sync::Arc::new(Vec::new()) }),
            "Shared PDF",
            "half the providers on a phone carry no name at all — see \
             picker::name_from_content_uri"
        );

        assert_eq!(
            song_name_for(&SharedItem::Chart("\n\n  Wonderwall\nAm  C  G\n".to_string())),
            "Wonderwall"
        );
        assert_eq!(song_name_for(&SharedItem::Chart("   \n\n".to_string())), "Shared text");
    }

    /// No name this module invents runs past the cut every other producer of a
    /// title in this app already agrees on.
    #[test]
    fn a_minted_name_is_cut_where_every_other_title_in_this_app_is() {
        let long = SharedItem::Link(format!("https://example.com/{}", "a".repeat(200)));
        let name = song_name_for(&long);
        assert!(
            name.chars().count() <= NAME_LIMIT + 1,
            "a 200-character slug came through as {} characters: {name}",
            name.chars().count()
        );
        assert!(name.ends_with('…'));
    }

    /// The placeholder-then-rename rule, and all three of its refusals.
    #[test]
    fn a_minted_placeholder_takes_the_captured_pages_title_and_nothing_else_does() {
        assert_eq!(
            rename_to_captured_title(
                "free fallin chords",
                Some("free fallin chords"),
                "Free Fallin' — Tom Petty (Chords)"
            ),
            Some("Free Fallin' — Tom Petty (Chords)".to_string()),
            "a song minted for this share takes the page's own title once the fetch lands"
        );
        assert_eq!(
            rename_to_captured_title("Landslide", None, "Landslide Chords — Some Site"),
            None,
            "the user filed this link under a song that already existed; renaming it \
             would be the worst thing this feature could do"
        );
        assert_eq!(
            rename_to_captured_title("Free Fallin'", Some("free fallin chords"), "Whatever"),
            None,
            "the name changed while the capture ran, so somebody chose it and it wins"
        );
        assert_eq!(
            rename_to_captured_title("free fallin chords", Some("free fallin chords"), "  \n "),
            None,
            "a page with no title leaves the slug, which at least came from the share"
        );
        assert_eq!(
            rename_to_captured_title("Wonderwall", Some("Wonderwall"), "Wonderwall"),
            None,
            "a rename to the name it already has is not a write worth making"
        );
    }

    /// What the chooser puts at the top of the screen: the thing itself, and
    /// the thumb it is about to become.
    #[test]
    fn the_chooser_identifies_the_shared_thing_by_what_it_would_become() {
        let link = SharedItem::Link("https://tabs.example.com/wonderwall".to_string());
        assert_eq!(link.kind(), Some(AttachmentKind::CapturedPage));
        assert_eq!(link.headline(), "https://tabs.example.com/wonderwall");
        assert!(link.savable());

        let chart = SharedItem::Chart("Wonderwall\nAm  C  G".to_string());
        assert_eq!(chart.kind(), Some(AttachmentKind::Text));
        assert_eq!(chart.headline(), "Wonderwall");

        // The line under the headline is a sentence, and it was not one when
        // this borrowed `AttachmentKind::descriptor` — see `destination`.
        for item in [&link, &chart, &pdf("landslide.pdf")] {
            let line = format!("This will be {}.", item.destination());
            assert!(
                !line.contains("a saved page") && !line.contains("a typed."),
                "the chooser's sub-line reads badly: {line}"
            );
        }

        assert_eq!(pdf("landslide.pdf").kind(), Some(AttachmentKind::Pdf));
        assert_eq!(pdf("landslide.pdf").headline(), "landslide.pdf");

        // The framework's own words, as they actually arrive off a phone.
        let broken = SharedItem::Unreadable(
            "readContentUri returned null (IO error or invalid URI)".to_string(),
        );
        assert_eq!(broken.kind(), None);
        assert!(
            !broken.savable(),
            "a share with nothing in it draws no song list — there is nothing to file"
        );
        assert_eq!(
            broken.headline(),
            "That could not be read.",
            "the framework's sentence must not be the largest thing on the screen"
        );
        assert_eq!(
            broken.trouble(),
            Some("readContentUri returned null (IO error or invalid URI)"),
            "...but it is not thrown away either"
        );
        assert_eq!(
            SharedItem::Link("https://example.com/x".to_string()).trouble(),
            None
        );
    }
}

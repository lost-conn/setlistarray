//! Values the app computes rather than stores.
//!
//! Group buckets, sort order, cumulative setlist times, total runtime and the
//! "Before you start" prep facts are all derived on read. Keeping them here as
//! plain functions — no signals, no stores — means the rules the screens are
//! thin over can be tested without a window.

use rinch_tabler_icons::TablerIcon;

use std::collections::BTreeSet;

use crate::model::{
    AttachmentId, AttachmentKind, Confidence, Day, Setlist, SetlistId, Song, SongId, fmt_bytes,
    fmt_duration,
};
use crate::store::{
    AccentChoice, Density, Filters, Group, GroupBy, PerformanceTheme, Route, SortDir, SortField,
    ThemeChoice,
};
use crate::theme::Rgb;

/// How many rows a group shows before "Show N more".
pub const GROUP_PREVIEW: usize = 6;

/// Songs matching a search query, over title, artist and tags. An empty query
/// matches everything.
///
/// Two callers, and neither of them is the library list any more: the song
/// picker (`1i`) and the search screen (`1p`). Card G1 took the library's
/// in-place filter out — see [`grouped`] — so this is now the matcher a screen
/// with a field of its own asks, rather than a rule the book is read through.
/// The "empty matches everything" answer is right for both of those and wrong
/// for `1p`'s first frame, which is why [`search_songs`] does not simply call
/// this one and hope.
pub fn filter_songs(songs: Vec<Song>, query: &str) -> Vec<Song> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return songs;
    }
    songs
        .into_iter()
        .filter(|s| {
            s.title.to_lowercase().contains(&query)
                || s.artist.to_lowercase().contains(&query)
                || s.tags.iter().any(|t| t.to_lowercase().contains(&query))
        })
        .collect()
}

/// The library list, filtered, grouped and sorted.
///
/// **It no longer takes a query, and that is card G1 removing a behaviour
/// rather than an oversight.** The library's search field used to filter this
/// list as you typed; since G1 it is a way in to the search screen (`1p`) and
/// types nothing. Leaving the filter here would have meant a library that comes
/// back from a search still showing four of its three hundred songs, with no
/// field on the screen to say why and nothing to clear — the dead-end H1 spent
/// its own effort avoiding, one screen over. So the book reads as the whole
/// book, and a question about it is asked somewhere the answer can be seen.
///
/// **`filters` is not that same mistake, and card G3 is why the two can sit
/// side by side without contradicting each other.** `query` was a text field
/// that filtered in place with no way to see or clear what was hiding the
/// rest of the book; `filters` is a sheet with its own chip, its own count,
/// its own Clear, and the library's own sub-line says "N of 300" the moment
/// it is doing anything — see `crate::screens::library`. Applied first, before
/// bucketing, so a group header counts what it actually contains rather than
/// promising twelve songs and delivering a header that still says forty.
pub fn grouped(
    songs: Vec<Song>,
    group_by: GroupBy,
    sort_field: SortField,
    sort_dir: SortDir,
    filters: &Filters,
) -> Vec<Group> {
    let songs = filter_by(songs, filters);
    let mut groups = group_songs(songs, group_by);
    for group in &mut groups {
        sort_songs(&mut group.songs, sort_field, sort_dir);
    }
    groups
}

/// The book with every song that fails an active filter facet removed. Its
/// own function, rather than folded into [`grouped`], because two other
/// readers need exactly this list and nothing past it: the Songs screen's
/// sub-line counts it (`library_subtitle`, below) and its "nothing matches"
/// panel asks whether it came back empty.
pub fn filter_by(songs: Vec<Song>, filters: &Filters) -> Vec<Song> {
    songs.into_iter().filter(|s| filters.matches(s)).collect()
}

/// Bucket songs by the active grouping. Empty buckets are dropped.
pub fn group_songs(songs: Vec<Song>, group_by: GroupBy) -> Vec<Group> {
    match group_by {
        GroupBy::Confidence => bucket_by_confidence(songs),
        GroupBy::FirstLetter => bucket_by(songs, |s| {
            s.title
                .chars()
                .next()
                .map(|c| c.to_ascii_uppercase().to_string())
                .unwrap_or_else(|| "#".into())
        }),
        GroupBy::Artist => bucket_by(songs, |s| s.artist.clone()),
        GroupBy::Tuning => bucket_by(songs, |s| {
            s.tuning.clone().unwrap_or_else(|| "No tuning set".into())
        }),
        GroupBy::Tag => bucket_by(songs, |s| {
            s.tags.first().cloned().unwrap_or_else(|| "Untagged".into())
        }),
        GroupBy::None => {
            if songs.is_empty() {
                Vec::new()
            } else {
                vec![Group {
                    label: "All songs".into(),
                    songs,
                }]
            }
        }
    }
}

/// Confidence groups in the fixed order Solid · Rusty · Learning · Unrated.
fn bucket_by_confidence(songs: Vec<Song>) -> Vec<Group> {
    let order = [
        (Some(Confidence::Solid), "Solid"),
        (Some(Confidence::Rusty), "Rusty"),
        (Some(Confidence::Learning), "Learning"),
        (None, "Unrated"),
    ];
    order
        .into_iter()
        .filter_map(|(confidence, label)| {
            let songs: Vec<Song> = songs
                .iter()
                .filter(|s| s.confidence == confidence)
                .cloned()
                .collect();
            (!songs.is_empty()).then(|| Group {
                label: label.into(),
                songs,
            })
        })
        .collect()
}

fn bucket_by(songs: Vec<Song>, key: impl Fn(&Song) -> String) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for song in songs {
        let label = key(&song);
        match groups.iter_mut().find(|g| g.label == label) {
            Some(group) => group.songs.push(song),
            None => groups.push(Group {
                label,
                songs: vec![song],
            }),
        }
    }
    groups.sort_by(|a, b| a.label.cmp(&b.label));
    groups
}

/// Which of A–Z have at least one group in `groups` — card H4's alphabet
/// scrubber reads this to decide which letters are live and which are drawn
/// dim.
///
/// Takes the already filtered-and-grouped list rather than the raw library,
/// so a filter that narrows the book down to no "Q" songs dims the same "Q"
/// the group list itself dropped — the scrubber and the list it sits beside
/// can never disagree about what is actually on screen. It is not scoped to
/// [`GroupBy::FirstLetter`] internally; the caller only calls this while that
/// grouping is active (the scrubber does not exist otherwise), so this stays
/// a plain reduction over whatever groups it is handed rather than a second
/// place that re-decides which grouping mode it is willing to answer for.
pub fn present_letters(groups: &[Group]) -> BTreeSet<String> {
    groups.iter().map(|g| g.label.clone()).collect()
}

/// Sort within a group. Songs missing the active field sort to the bottom
/// rather than disappearing, in either direction.
pub fn sort_songs(songs: &mut [Song], field: SortField, dir: SortDir) {
    songs.sort_by(|a, b| {
        match (field.is_present(a), field.is_present(b)) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        let ordering = match field {
            SortField::Artist => a.artist.to_lowercase().cmp(&b.artist.to_lowercase()),
            SortField::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            SortField::Confidence => confidence_rank(a).cmp(&confidence_rank(b)),
            // "newest first" is the ascending reading of a date field.
            SortField::LastPlayed => b.last_played.cmp(&a.last_played),
            SortField::DateAdded => b.created_at.cmp(&a.created_at),
            SortField::Key => a.key.cmp(&b.key),
            SortField::Tempo => a.tempo.cmp(&b.tempo),
            SortField::Duration => a.duration.cmp(&b.duration),
            SortField::Capo => a.capo.cmp(&b.capo),
            SortField::Tuning => a.tuning.cmp(&b.tuning),
        };
        match dir {
            SortDir::Asc => ordering,
            SortDir::Desc => ordering.reverse(),
        }
    });
}

fn confidence_rank(song: &Song) -> u8 {
    match song.confidence {
        Some(Confidence::Solid) => 0,
        Some(Confidence::Rusty) => 1,
        Some(Confidence::Learning) => 2,
        None => 3,
    }
}

/// The rows a group shows: a preview until the user asks for the rest.
pub fn visible_songs(songs: &[Song], expanded: bool) -> Vec<Song> {
    if expanded || songs.len() <= GROUP_PREVIEW {
        songs.to_vec()
    } else {
        songs[..GROUP_PREVIEW].to_vec()
    }
}

/// The running clock down the right edge of a setlist: when each song starts,
/// in seconds from the top of the set. A song with no duration takes no time.
pub fn cumulative_starts(songs: &[Song]) -> Vec<u32> {
    let mut running = 0;
    songs
        .iter()
        .map(|song| {
            let start = running;
            running += song.duration.unwrap_or(0);
            start
        })
        .collect()
}

/// Total runtime of a set, ignoring songs with no duration.
pub fn total_runtime(songs: &[Song]) -> u32 {
    songs.iter().filter_map(|s| s.duration).sum()
}

/// The "Before you start" panel: which tunings the set needs, how many songs
/// want a capo, and whether everything is available offline. Derived, not
/// authored.
///
/// An empty set gets an empty string, not "Every chart is on this phone and
/// works with no signal." — which is what the `missing == 0` branch below
/// would otherwise say about a set with no charts because it has no songs.
/// That sentence is true of nothing and reads as though it were true of
/// something, and card J4's audit is what caught it: `setlist_detail`'s own
/// empty-setlist state hides this whole panel on the same check, so the
/// wrong sentence should never have reached a screen, but the function itself
/// should not be able to produce it either.
pub fn prep_facts(songs: &[Song]) -> String {
    if songs.is_empty() {
        return String::new();
    }
    let mut tunings: Vec<String> = Vec::new();
    for song in songs {
        if let Some(t) = &song.tuning
            && !tunings.contains(t)
        {
            tunings.push(t.clone());
        }
    }
    let capos = songs.iter().filter(|s| s.capo.is_some()).count();
    let missing = songs.iter().filter(|s| !s.has_chart()).count();

    let mut sentences = Vec::new();
    match tunings.len() {
        0 => {}
        1 => sentences.push(format!("Everything is in {}.", tunings[0])),
        _ => sentences.push(format!("You'll need {}.", tunings.join(" and "))),
    }
    match capos {
        0 => {}
        1 => sentences.push("One song wants a capo.".to_string()),
        n => sentences.push(format!("{n} songs want a capo.")),
    }
    match missing {
        0 => sentences.push("Every chart is on this phone and works with no signal.".to_string()),
        1 => sentences.push("One song has no chart attached.".to_string()),
        n => sentences.push(format!("{n} songs have no chart attached.")),
    }
    sentences.join(" ")
}

// ---------------------------------------------------------------------------
// Add-to-setlist sheet (`2e`)
// ---------------------------------------------------------------------------

/// Setlists matching the sheet's search field, over the set name alone. An
/// empty query matches everything.
pub fn filter_setlists(setlists: Vec<Setlist>, query: &str) -> Vec<Setlist> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return setlists;
    }
    setlists
        .into_iter()
        .filter(|s| s.name.to_lowercase().contains(&query))
        .collect()
}

/// The sub-line under a setlist name in the sheet: `9 songs · 32:04`. Songs
/// with no duration contribute nothing to the clock, exactly as they do on the
/// setlist screen. An empty set says so rather than reading `0 songs · 0:00`.
pub fn setlist_summary(setlist: &Setlist, songs: &[Song]) -> String {
    let members: Vec<Song> = setlist
        .song_ids
        .iter()
        .filter_map(|id| songs.iter().find(|s| s.id == *id).cloned())
        .collect();
    match members.len() {
        0 => "empty".to_string(),
        1 => format!("1 song · {}", fmt_duration(total_runtime(&members))),
        n => format!("{n} songs · {}", fmt_duration(total_runtime(&members))),
    }
}

// ---------------------------------------------------------------------------
// Search & filter (`1p`)
// ---------------------------------------------------------------------------

/// One run of a result's display string, and whether the query matched it.
///
/// A `Vec<Highlight>` is the whole of what a highlighted row needs: the runs
/// concatenate back to the original string exactly, so the screen draws them in
/// order and gives the `matched` ones a background. Splitting the string here
/// rather than in the screen is what makes the fiddly half testable — see
/// [`highlight`], which is one `unwrap`-free function and eleven tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Highlight {
    pub text: String,
    pub matched: bool,
}

/// Split a display string into matched and unmatched runs, case-insensitively.
///
/// The runs are the *original* text, never the lowercased one: `1p` highlights
/// `Dylan` inside `Bob Dylan` for the query `dylan`, and a row that answered
/// `bob dylan` would be a search that quietly rewrote the library. Every match
/// in the string is marked, not just the first.
///
/// ## Why this is a byte map and not `to_lowercase().find()`
///
/// The obvious implementation lowercases the haystack, finds the needle in it,
/// and slices the *original* at the byte offsets it got back. That is wrong,
/// and on this app's content it is wrong in the way that ends a process rather
/// than the way that returns a wrong answer: `str::to_lowercase` is not
/// length-preserving. `İ` (U+0130, two bytes) lowercases to `i̇` — two chars,
/// three bytes — so every offset after it is shifted, and slicing the original
/// at a shifted offset lands mid-character and panics. Card K34 is about a flat
/// sign surviving a capture; this app ships Unicode in titles, artists, tags and
/// set names, and a search field is where all four are typed at.
///
/// So the lowercased haystack is built one character at a time alongside a map
/// from each of its bytes back to the byte that character *started* at in the
/// original. A match's start and end are both looked up through it, which means
/// both ends are always original character boundaries whatever the case folding
/// did to the lengths in between.
///
/// The one thing the map cannot do is highlight *half* a character. A query of
/// `i` against `İstanbul` finds a match inside the expansion of that first
/// character, whose start and end map to the same original byte; there is no
/// substring of the original that is the matched part, so that match is skipped
/// and the scan moves on. It is a wrong answer for one letter in one alphabet,
/// arrived at deliberately, and it is not a panic — which is the trade the whole
/// function is here to make.
pub fn highlight(text: &str, query: &str) -> Vec<Highlight> {
    // Trimmed to match [`filter_songs`] exactly. The matcher decided a row is
    // in the list by trimming the query; if the highlighter did not, a query
    // typed with a trailing space would put rows on screen with nothing marked
    // on them, and the field would look broken by one invisible character.
    let needle = query.trim().to_lowercase();
    let whole = |text: &str| match text.is_empty() {
        true => Vec::new(),
        false => vec![Highlight {
            text: text.to_string(),
            matched: false,
        }],
    };
    if needle.is_empty() {
        return whole(text);
    }

    let (hay, map) = lowered_with_map(text);

    let mut runs: Vec<Highlight> = Vec::new();
    let mut cursor = 0; // into `hay`
    let mut kept = 0; // into `text`: the start of the unmatched run being gathered
    while let Some(at) = hay[cursor..].find(&needle) {
        let (from, to) = (cursor + at, cursor + at + needle.len());
        cursor = to;
        let (from, to) = (map[from], map[to]);
        // Half a character, or a match already covered — see the module note
        // above. Skipped rather than sliced.
        if to <= from || from < kept {
            continue;
        }
        if from > kept {
            runs.push(Highlight {
                text: text[kept..from].to_string(),
                matched: false,
            });
        }
        runs.push(Highlight {
            text: text[from..to].to_string(),
            matched: true,
        });
        kept = to;
    }

    if runs.is_empty() {
        return whole(text);
    }
    if kept < text.len() {
        runs.push(Highlight {
            text: text[kept..].to_string(),
            matched: false,
        });
    }
    runs
}

/// The lower-cased haystack `highlight` (and card G2's [`first_match_span`])
/// search, and the map back from each of its bytes to the byte in the
/// original `text` its character started at.
///
/// Pulled out of `highlight` rather than inlined twice: both callers need the
/// exact same length-preserving construction — see `highlight`'s own doc for
/// why a lowercased copy alone is not enough — and a second, slightly
/// different copy of this loop is exactly how the two would quietly drift
/// apart on the one case (`İ`) that is hard to get right by eye.
fn lowered_with_map(text: &str) -> (String, Vec<usize>) {
    let mut hay = String::with_capacity(text.len());
    // `map[i]` is the byte in `text` that `hay`'s byte `i` came from; the extra
    // entry at the end is what lets a match that runs to the end of the string
    // be looked up the same way as every other one.
    let mut map: Vec<usize> = Vec::with_capacity(text.len() + 1);
    for (at, ch) in text.char_indices() {
        let before = hay.len();
        for lowered in ch.to_lowercase() {
            hay.push(lowered);
        }
        for _ in before..hay.len() {
            map.push(at);
        }
    }
    map.push(text.len());
    (hay, map)
}

/// The songs `1p` lists, in the order it lists them.
///
/// **An empty query matches nothing here, where [`filter_songs`] matches
/// everything.** That difference is the screen's first frame: the library is a
/// list you are shown and a filter narrows it, but a search screen with nothing
/// typed has not been asked anything yet, and answering "all three hundred of
/// them" is a result set that means nothing and buries the one row the user is
/// about to make appear. The screen draws its own empty state instead.
///
/// Sorted by title A–Z rather than by the library's own sort, which is the same
/// call [`picker_songs`] makes and for the same two reasons: the list is flat,
/// so the grouping the library sort exists to pair with is not here; and a
/// search is made with something already in mind, which is a thing you scan for
/// by name. The count line says so out loud — see [`search_count_line`].
pub fn search_songs(songs: Vec<Song>, query: &str) -> Vec<Song> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let mut found = filter_songs(songs, query);
    sort_songs(&mut found, SortField::Title, SortDir::Asc);
    found
}

/// The setlists `1p` lists, over the set name alone.
///
/// The name and nothing else, deliberately: a set's songs are already in the
/// Songs group above it, so matching a set on its members would list `Dylan
/// night` under Setlists because it contains a Dylan song, which is a different
/// claim from the one the row appears to make and the same rows twice over.
///
/// A–Z by name, for the same reason the songs are A–Z by title, and the empty
/// query answers empty for the same reason too.
pub fn search_setlists(setlists: Vec<Setlist>, query: &str) -> Vec<Setlist> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let mut found = filter_setlists(setlists, query);
    found.sort_by_key(|s| s.name.to_lowercase());
    found
}

/// One song on `1p`, with the query already segmented out of the two strings
/// the row prints.
///
/// **The segmentation is part of the row's data, and that is a reactivity
/// decision rather than a tidiness one.** Rinch's keyed `for` preserves the DOM
/// of an item whose key survives a change and re-renders one whose *data*
/// changed (`for_each_dom`'s equality pass). A row keyed by song id survives
/// every keystroke that leaves the song matching — `dyl` to `dyla` — so if the
/// highlight were computed inside the row body from a signal, it would be built
/// once and then never again, and the mark would stay frozen on `Dyl` under a
/// field reading `dyla`. That is not a hypothetical: it is what card G1 shipped
/// to the phone first and had to be shown, and the tell was subtle — the list
/// narrowed correctly, so everything *looked* live except the three pixels
/// under the mark.
///
/// Putting the runs in the item makes the highlight part of what `PartialEq`
/// compares, so the same pass that keeps a row's DOM when nothing about it
/// changed rebuilds it the moment its mark moves — and rebuilds only those.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SongHit {
    pub song: Song,
    pub title: Vec<Highlight>,
    pub meta: Vec<Highlight>,
}

/// One setlist on `1p`, segmented for the same reason [`SongHit`] is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetlistHit {
    pub setlist: Setlist,
    pub name: Vec<Highlight>,
    pub summary: String,
}

/// The song rows `1p` draws, in order, with their marks.
///
/// The second line is [`Song::meta_line`] — `Artist · key · tempo` — and not
/// the `meta_line_played` variant a library row falls back to. A result's meta
/// line has a second job the library's does not: it is where you see *why* this
/// row is in the list, which for a query like `dylan` is the artist and can
/// never be `played May`. So the line that always leads with the artist is the
/// one that gets highlighted.
pub fn song_hits(songs: Vec<Song>, query: &str) -> Vec<SongHit> {
    search_songs(songs, query)
        .into_iter()
        .map(|song| SongHit {
            title: highlight(&song.title, query),
            meta: highlight(&song.meta_line(), query),
            song,
        })
        .collect()
}

/// The setlist rows `1p` draws, with their marks and their `9 songs · 32:04`.
///
/// `songs` is the library, needed only to add up the sub-line — the same
/// [`setlist_summary`] the add-to-setlist sheet prints, so a set reads the same
/// wherever it is listed. The summary is not highlighted: it is a count and a
/// clock, and a query that matched digits in it would be marking a coincidence.
pub fn setlist_hits(setlists: Vec<Setlist>, songs: &[Song], query: &str) -> Vec<SetlistHit> {
    search_setlists(setlists, query)
        .into_iter()
        .map(|setlist| SetlistHit {
            name: highlight(&setlist.name, query),
            summary: setlist_summary(&setlist, songs),
            setlist,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Inside attachments (card G2)
// ---------------------------------------------------------------------------

/// One attachment, ready for [`attachment_hits`] to search: everything that
/// module needs to know about a chart *except* whether it matches.
///
/// The screen builds this list, not this module. `AttachmentsStore`'s own
/// header explains why the ownership arrow runs songs → attachments and never
/// back — an `Attachment` does not carry the id of the song it belongs to —
/// and this module's header explains the other half: no signals, no I/O, so a
/// pure function here cannot go fetch a body out of the database itself. So
/// the screen walks every song's own `attachments` list, looks each one up,
/// reads its body once, and hands over the flattened result. See
/// `screens::search::attachment_texts`.
///
/// `kind` only ever arrives as [`AttachmentKind::Text`] or
/// [`AttachmentKind::CapturedPage`] in practice — a PDF's `body` is always
/// `None`, because nothing in this app extracts text from one yet (`hayro`
/// parses far enough to count pages and to rasterise a page as a picture;
/// neither is a text layer, and no other crate in this tree has one). This
/// struct does not enforce that split itself, on purpose: which kinds carry
/// extracted text is a fact about the capture and typed-chart pipelines, and
/// a search matcher that hard-codes an assumption about its own callers is
/// how a rule like that goes stale the day a fourth kind is added and nobody
/// remembers this file exists.
#[derive(Clone, Debug, PartialEq)]
pub struct AttachmentText {
    pub song: SongId,
    pub song_title: String,
    pub attachment: AttachmentId,
    pub attachment_title: String,
    pub kind: AttachmentKind,
    pub body: String,
}

/// One "Inside attachments" row: the song it belongs to, the chart to open,
/// and the window of text the match sits in.
///
/// `Default` for the same reason [`SongHit`] and [`SetlistHit`] carry one —
/// the row's own component takes this as a prop, and the rsx macro builds
/// every prop with `..Default::default()`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttachmentHit {
    pub song: SongId,
    pub song_title: String,
    pub attachment: AttachmentId,
    pub attachment_title: String,
    pub kind: AttachmentKind,
    pub snippet: Vec<Highlight>,
}

/// How many characters either side of a match [`attachment_snippet`] keeps.
///
/// Forty was picked on the phone against a 393px row with an icon and its
/// inset already spent: a chord line and the lyric under it both fit inside
/// eighty-odd monospace characters with room for the row's own padding, and a
/// captured page's prose reads as a real sentence fragment rather than three
/// words swimming in ellipses.
const SNIPPET_RADIUS: usize = 40;

/// The song title and artist are deliberately not searched a second time
/// here — [`song_hits`] already covers both, and a chart whose *song* also
/// happens to answer the query would otherwise put the same song under two
/// headings for one keystroke, which reads as a screen that cannot count.
///
/// This is the whole of card G2's "scan, not an inverted index": every
/// attachment's body, read once already (by the screen, into `entries`), is
/// asked whether it contains the query, in the order the songs were handed
/// over. At the stated library size — 300 songs — that is a few hundred
/// linear string scans per keystroke, not the kind of workload an index earns
/// its keep against; `derive_tests::searching_a_three_hundred_song_library_s_
/// attachments_stays_fast` is where that claim is measured rather than
/// assumed. If a real library ever gets large enough to make this slow, the
/// fix is the inverted index the stale card body already named — built once
/// at import time rather than walked at every keystroke — and this comment is
/// where the next person should start.
pub fn attachment_hits(entries: Vec<AttachmentText>, query: &str) -> Vec<AttachmentHit> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<AttachmentHit> = entries
        .into_iter()
        .filter_map(|entry| {
            let snippet = attachment_snippet(&entry.body, query)?;
            Some(AttachmentHit {
                song: entry.song,
                song_title: entry.song_title,
                attachment: entry.attachment,
                attachment_title: entry.attachment_title,
                kind: entry.kind,
                snippet,
            })
        })
        .collect();
    // A–Z by the song's own title, the same order the Songs group above it
    // reads in — so a query that lands in both groups does not read as two
    // unrelated shuffles of the same book.
    hits.sort_by(|a, b| {
        a.song_title
            .to_lowercase()
            .cmp(&b.song_title.to_lowercase())
            .then_with(|| {
                a.attachment_title
                    .to_lowercase()
                    .cmp(&b.attachment_title.to_lowercase())
            })
    });
    hits
}

/// The first case-insensitive match of `needle` (already trimmed and
/// lower-cased) in `text`, as a byte span into the *original* string.
///
/// `highlight`'s own loop collects every match because a title or an artist
/// name is a handful of words and the whole point is marking each one. A
/// captured page can be tens of thousands of characters, and a snippet only
/// ever shows the first hit, so this stops there rather than building a `Vec`
/// of every place the query appears in somebody else's whole webpage.
fn first_match_span(text: &str, needle: &str) -> Option<(usize, usize)> {
    let (hay, map) = lowered_with_map(text);
    let mut cursor = 0;
    while let Some(at) = hay[cursor..].find(needle) {
        let (from, to) = (cursor + at, cursor + at + needle.len());
        cursor = to;
        let (from, to) = (map[from], map[to]);
        if to > from {
            return Some((from, to));
        }
        // Half a character — see `highlight`'s doc on `İstanbul` — skipped,
        // never sliced; the scan just keeps going past it.
    }
    None
}

/// The byte offset that starts a window of up to `chars` characters ending
/// exactly at `idx`. Always a char boundary, because it only ever lands on
/// one `char_indices` gave back.
fn char_boundary_back(text: &str, idx: usize, chars: usize) -> usize {
    text[..idx]
        .char_indices()
        .rev()
        .nth(chars.saturating_sub(1))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// The byte offset that ends a window of up to `chars` characters starting at
/// `idx`. The mirror of [`char_boundary_back`].
fn char_boundary_forward(text: &str, idx: usize, chars: usize) -> usize {
    text[idx..]
        .char_indices()
        .nth(chars)
        .map(|(i, _)| idx + i)
        .unwrap_or(text.len())
}

/// A window of context around the first match inside an attachment's text,
/// marked the way every other group on this screen marks its hits.
///
/// **One line, always**, per card G2's own instruction — a hit inside a
/// 200-line chart has to show *where* it matched, not the whole chart, and
/// the row it sits in is one line tall like every other result on this
/// screen. So every run of whitespace in the window — including the chord
/// chart's own significant spacing and every newline a captured page's
/// extracted text still carries — collapses to a single space. That is an
/// honest cost, not a free one: a chord sitting three tabs above its syllable
/// reads as `G       D` run together with the lyric below it, on the one
/// line this function is allowed to draw. The alternative was drawing the
/// match's own line at full width and letting `ONE_LINE`'s ellipsis eat
/// whichever half did not fit, which for a chord chart is usually the half
/// with the highlight in it — worse than a legible line that has lost its
/// alignment.
///
/// An ellipsis marks whichever edge was actually cut, the way a search result
/// snippet reads anywhere else. `None` means the query is not in `text` at
/// all, which is the caller's cue to leave the attachment out of the group
/// entirely — the same shape [`search_songs`] answers with an empty `Vec`.
///
/// **One trade named rather than hidden.** Collapsing whitespace runs the
/// window through `str::split_whitespace` *before* handing it to
/// [`highlight`], so a query that itself contains more than one run of
/// whitespace — two spaces, or a line break, typed into the search field on
/// purpose — can fail to re-find itself in the now-collapsed window and fall
/// back to drawing the line unmarked. Accepted for the same reason
/// `highlight` accepts losing the first letter of `İstanbul`: a rare wrong
/// answer on an unusual query beats a panic, and beats the common case
/// staying legible.
pub fn attachment_snippet(text: &str, query: &str) -> Option<Vec<Highlight>> {
    let needle = query.trim();
    if needle.is_empty() {
        return None;
    }
    let (from, to) = first_match_span(text, &needle.to_lowercase())?;

    let start = char_boundary_back(text, from, SNIPPET_RADIUS);
    let end = char_boundary_forward(text, to, SNIPPET_RADIUS);

    let mut window = String::new();
    if start > 0 {
        window.push('…');
    }
    window.push_str(&text[start..end].split_whitespace().collect::<Vec<_>>().join(" "));
    if end < text.len() {
        window.push('…');
    }

    Some(highlight(&window, needle))
}

/// `1p`'s count line: `6 of 300 songs · 1 of 2 setlists · sorted A–Z`.
///
/// The wireframe draws `6 of 300 songs · sorted by title`, from a search screen
/// that only ever listed songs. Two things about it had to change once setlists
/// were in the results.
///
/// **What it counts.** Both groups, each against its own total, rather than one
/// number over a total that mixes two kinds of thing. `7 of 302` is arithmetic
/// nobody asked for: a book has three hundred songs and two sets, those are not
/// three hundred and two of anything, and the whole value of this line is
/// telling you how much of the book you are looking at.
///
/// **Its last clause.** `sorted by title` is true of the songs and meaningless
/// for a setlist, which has a name. `sorted A–Z` is the same promise in a word
/// that is true of both rows, and it is a promise worth printing because the
/// order is *not* the library's — see [`search_songs`].
///
/// The setlist clause is dropped entirely from a book with no sets in it. `0 of
/// 0 setlists` is not a fact about a search, it is a fact about a user who has
/// never made a setlist, and it does not belong on the line that says how the
/// search went.
///
/// **Card G2 does not add a third clause here.** This line answers "how much
/// of the book am I looking at", and a book is songs and setlists — the two
/// things this app lets you own and browse a list of. A hit inside an
/// attachment's text is not a third kind of thing in the book, it is a reason
/// one of the songs already counted above is in the list; a `2 of 4
/// attachments` clause would be counting *chart matches*, a number with no
/// "of how many" that means anything (of how many attachments exist? of how
/// many this song has? neither is what a user asking "how much of my search
/// came back" wants), on a line that has been careful until now to only ever
/// count things this app already shows you a browsable total of.
pub fn search_count_line(
    songs_found: usize,
    songs_total: usize,
    setlists_found: usize,
    setlists_total: usize,
) -> String {
    let counted = |found: usize, total: usize, noun: &str| match total {
        1 => format!("{found} of 1 {noun}"),
        _ => format!("{found} of {total} {noun}s"),
    };
    let mut parts = vec![counted(songs_found, songs_total, "song")];
    if setlists_total > 0 {
        parts.push(counted(setlists_found, setlists_total, "setlist"));
    }
    parts.push("sorted A–Z".to_string());
    parts.join(" · ")
}

/// One row of `1p`'s scrolling area, empty states included.
///
/// **The whole area is one list rather than four conditional blocks, and that
/// is a bug this card walked into rather than a preference.** The obvious
/// shape — an `if` for the Songs group, an `if` for the Setlists group, an `if`
/// for "nothing typed" and an `if` for "nothing matched", four siblings in one
/// scrolling column — was built, and on the phone the last of them did not
/// always appear when its condition turned true. Typing `d` (six songs and a
/// set), then `y`, emptied the list and left the screen blank: no rows, and no
/// "Nothing matches" either, with the count line above it correctly reading
/// `0 of 26 songs`. The same transition from a *different* starting state drew
/// it. Four `show_dom` markers inserting and removing siblings in one parent is
/// evidently not a shape to rely on here, and this card is not the place to fix
/// rinch's `if`.
///
/// A keyed `for` is the mechanism this app already leans on everywhere and
/// which is exercised by every list screen in it, so the area became one of
/// those: every row, heading and empty panel alike, is an item with a key, and
/// what is on screen is a value this module produces and tests rather than four
/// conditions evaluated in a layout. Card G2's "Inside attachments" group is a
/// heading and some [`SearchRow::Attachment`] rows appended in the same
/// function, on exactly this same rule — see [`attachment_hits`].
// `Default` because the screen draws each row through a component that takes
// one as a prop, and the rsx macro builds every prop struct with
// `..Default::default()`. `Prompt` is the variant to default to: it is the row
// an untouched screen is made of.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum SearchRow {
    /// A group heading and how many rows sit under it.
    Heading {
        label: &'static str,
        count: usize,
        /// The first heading is accent-coloured, later ones muted — the same
        /// rule the library's own group headers follow.
        first: bool,
    },
    Song(SongHit),
    Setlist(SetlistHit),
    /// One hit inside a chart's text — card G2. Named `Attachment` rather than
    /// `Inside` or `Text` because what it carries and what a row draws from it
    /// is an [`AttachmentHit`], and every other variant here is named after
    /// its own payload the same way.
    Attachment(AttachmentHit),
    /// Nothing typed yet.
    #[default]
    Prompt,
    /// Typed, and nothing in the book answers to it. Carries the trimmed query
    /// so the panel can name it back — and so that the row's *data* changes on
    /// every keystroke, which is what makes rinch rebuild it rather than leave
    /// a panel quoting the query before last.
    NoMatch(String),
}

impl SearchRow {
    /// The `key:` the screen gives this row.
    ///
    /// Stable across keystrokes on purpose: a row that survives a change keeps
    /// its DOM, and the equality pass rebuilds it only if what it draws moved.
    /// The two panels are singletons and say so.
    pub fn key(&self) -> String {
        match self {
            SearchRow::Heading { label, .. } => format!("h:{label}"),
            SearchRow::Song(hit) => format!("s:{}", hit.song.id),
            SearchRow::Setlist(hit) => format!("l:{}", hit.setlist.id),
            SearchRow::Attachment(hit) => format!("a:{}", hit.attachment),
            SearchRow::Prompt => "prompt".to_string(),
            SearchRow::NoMatch(_) => "nomatch".to_string(),
        }
    }
}

/// Everything `1p` scrolls, in order.
///
/// Four states, and each is the whole list rather than a layer over the
/// others: nothing typed is one [`SearchRow::Prompt`]; a query with no answer
/// is one [`SearchRow::NoMatch`]; anything else is a heading and its rows per
/// group that has any. A group with no matches contributes nothing at all — not
/// a heading over nothing, which is the reason card G1 left "Inside
/// attachments" undrawn until this card could answer it honestly rather than
/// with an empty shell — see this file's module header.
pub fn search_rows(
    songs: Vec<Song>,
    setlists: Vec<Setlist>,
    attachments: Vec<AttachmentText>,
    query: &str,
) -> Vec<SearchRow> {
    if query.trim().is_empty() {
        return vec![SearchRow::Prompt];
    }

    let sets = setlist_hits(setlists, &songs, query);
    let found = song_hits(songs, query);
    let inside = attachment_hits(attachments, query);
    if found.is_empty() && sets.is_empty() && inside.is_empty() {
        return vec![SearchRow::NoMatch(query.trim().to_string())];
    }

    let mut rows = Vec::new();
    if !found.is_empty() {
        rows.push(SearchRow::Heading {
            label: "Songs",
            count: found.len(),
            first: true,
        });
        rows.extend(found.into_iter().map(SearchRow::Song));
    }
    if !sets.is_empty() {
        // `first: false` even with no songs above it, so the headings never
        // trade colours depending on what the query happened to find — a
        // heading that is accent in one search and muted in the next reads as a
        // state rather than as a label. The same reasoning applies to the
        // Inside-attachments heading just below.
        rows.push(SearchRow::Heading {
            label: "Setlists",
            count: sets.len(),
            first: false,
        });
        rows.extend(sets.into_iter().map(SearchRow::Setlist));
    }
    if !inside.is_empty() {
        rows.push(SearchRow::Heading {
            label: "Inside attachments",
            count: inside.len(),
            first: false,
        });
        rows.extend(inside.into_iter().map(SearchRow::Attachment));
    }
    rows
}

// ---------------------------------------------------------------------------
// Performance mode (`1o`)
// ---------------------------------------------------------------------------

/// Which song of a set is being played, and the `2 / 5` that says so.
///
/// `index` is `PlaybackStore::index` and `len` is how many songs the set
/// actually resolves to — songs deleted out of the library underneath it are
/// already gone by the time this is asked, which is why the length is passed
/// rather than read off `Setlist::song_ids`.
///
/// Two rules, and both are about a number that has no business being out of
/// range but can be:
///
/// * **A set with nothing playable in it has no position at all**, so this
///   returns `None` and the screen says so in words. `0 / 0` would be a
///   counter, and a counter is a promise that there is something to count.
/// * **An index past the end is clamped to the last song** rather than
///   blanking the screen or panicking. It can happen for real: the set was
///   playing at song 5 of 6 and somebody removed two songs from it on another
///   screen. Clamping keeps a chart on the stand, and because the label is
///   derived from the *clamped* index the counter cannot then disagree with
///   what is under it.
pub fn playing_at(index: usize, len: usize) -> Option<(usize, String)> {
    if len == 0 {
        return None;
    }
    let index = index.min(len - 1);
    Some((index, format!("{} / {len}", index + 1)))
}

/// Whether the set has a song either side of the one being played.
///
/// Two bools rather than a tuple of them, because `(true, false)` at a call
/// site is two chances to read it the wrong way round and the compiler has an
/// opinion about neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Steps {
    /// There is a song before this one, so `1o`'s ‹ is a control.
    pub back: bool,
    /// There is a song after it, so `1o`'s › is a control.
    pub forward: bool,
}

/// Which way performance mode can move through the set from here.
///
/// The whole of what F2's two chevrons need to know, and it is derived rather
/// than asked of [`PlaybackStore::next`](crate::store::PlaybackStore::next) and
/// `prev` — those two clamp, which is the right behaviour for a tap and the
/// wrong shape for a question, because a call that clamps cannot be asked
/// whether it *would have* moved without moving.
///
/// **The index is clamped first, through [`playing_at`], and that is the whole
/// reason this is a function and not two comparisons written inline.** The
/// screen already draws the clamped song — `9` in a set of 3 puts song 3 on the
/// stand — so a raw `index + 1 < len` would light › over the last song in the
/// set and offer a move that cannot happen. Deriving both answers from the same
/// clamp `playing_at` performs is what keeps the chevrons agreeing with the
/// chart underneath them, exactly as the counter already does.
///
/// A set with nothing playable in it goes nowhere in either direction, and so
/// does a set of one. Both are dimmed at both ends rather than absent: the
/// chevrons are the only way through the set, and a control that vanishes when
/// it has nothing to do teaches you it was never there.
pub fn steps(index: usize, len: usize) -> Steps {
    match playing_at(index, len) {
        None => Steps {
            back: false,
            forward: false,
        },
        Some((index, _)) => Steps {
            back: index > 0,
            forward: index + 1 < len,
        },
    }
}

/// The line under the song title in performance mode: `G · capo 2 · 96 bpm`.
///
/// Three facts and no fourth, which is the whole reason this is not
/// [`Song::meta_line`] with a capo bolted on. The other meta lines in the app
/// answer "which song is this?" and so they lead with the artist; this one is
/// read *while playing it*, when the question has already been answered by the
/// chart filling the rest of the screen, and the only things left worth a
/// glance are the three you have to do something about with your hands. The
/// artist is deliberately absent for that reason and not by omission —
/// `1o` draws exactly these three and nothing else.
///
/// Everything is optional, so every part can be missing, and a song with none
/// of the three returns an empty string rather than a stray separator or a
/// line of nothing. The caller is expected to draw no line at all for that,
/// which is why this returns `""` instead of a placeholder: an empty slot is
/// not this app's house style, and a blank second row under a centred title
/// would push the title off centre in the top bar for no information at all.
pub fn performance_meta(song: &Song) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(key) = &song.key {
        // A key typed as spaces is a key nobody set. The form trims what it
        // stores, but a library restored from a backup or written by an older
        // build has no such promise, and " · capo 2" is a worse answer than
        // "capo 2".
        let key = key.trim();
        if !key.is_empty() {
            parts.push(key.to_string());
        }
    }
    if let Some(capo) = song.capo {
        // `parse_count` already refuses a nought — a capo on the nut is no
        // capo — but the field is a plain `u8` and a row can arrive from
        // anywhere, so the rule is re-stated where it is read rather than
        // trusted to have been applied where it was written.
        if capo > 0 {
            parts.push(format!("capo {capo}"));
        }
    }
    if let Some(tempo) = song.tempo {
        if tempo > 0 {
            parts.push(format!("{tempo} bpm"));
        }
    }
    parts.join(" · ")
}

/// What the left of the bottom bar says, in the two states it has.
///
/// `1o` draws `up next  Blackbird` and draws nothing else, which leaves the
/// last song in the set — the one moment this line has something worth saying —
/// undrawn. The one thing it must not do is print the label with nothing after
/// it: a word followed by an empty slot, read at a metre with a guitar in your
/// hands, is indistinguishable from a title that failed to load, and this app
/// has already decided twice (see `performance_meta`, and the `1o` meta line it
/// feeds) that an empty slot is not its house style.
///
/// So the last song replaces the whole line rather than blanking half of it,
/// and the two states are an enum rather than a label-and-title pair of strings
/// precisely so that the dangling half cannot be constructed at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpNext {
    /// There is another song after this one, and this is its title.
    Song(String),
    /// This is the last song in the set.
    ///
    /// Said out loud rather than left blank, because it is a fact somebody
    /// playing wants: whether to reach for the setlist afterwards or start the
    /// next one from memory is a decision made *during* the last song, not
    /// after it.
    Last,
}

impl UpNext {
    /// The muted words — the label before a title, or the whole line when there
    /// is no title to come.
    pub fn label(&self) -> &'static str {
        match self {
            UpNext::Song(_) => "up next",
            UpNext::Last => "last song",
        }
    }

    /// The title in ink after the label, as a nought-or-one vector: `rsx!`'s
    /// `for` is the only conditional the macro has, and "no title" has to be an
    /// absent element rather than an empty one.
    pub fn title(&self) -> Vec<String> {
        match self {
            UpNext::Song(title) => vec![title.clone()],
            UpNext::Last => Vec::new(),
        }
    }
}

/// The next song in the set, given the songs of it that still resolve.
///
/// `set` is what `performance::ordered` handed over, and that is the whole
/// reason this takes a list rather than a setlist: a song deleted out of the
/// library underneath a running gig is already gone from it, so "up next" names
/// the next song that can actually be *played* rather than the next id the set
/// happens to hold. That is the same list [`playing_at`] counts and [`steps`]
/// measures, which is what keeps `2 / 4`, a live › and this line agreeing with
/// one another instead of each being right about a different set.
///
/// `None` for a set with nothing playable in it. There is no song on the stand,
/// so there is nothing for a next song to be next to — the screen draws no bar
/// at all rather than a bar full of blanks. The index is clamped through
/// [`playing_at`] for the reason `steps` is: the screen already draws the
/// clamped song, and a line naming the song after an index nothing is drawn at
/// would be a promise about a chart that is not on the stand.
pub fn up_next(index: usize, set: &[Song]) -> Option<UpNext> {
    let (index, _) = playing_at(index, set.len())?;
    Some(match set.get(index + 1) {
        Some(next) => UpNext::Song(next.title.clone()),
        None => UpNext::Last,
    })
}

/// One song's worth of the progress strip along the very bottom of `1o`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment {
    /// Behind us — solid ink.
    Played,
    /// On the stand — accent.
    Current,
    /// Still to come — muted.
    Upcoming,
}

/// The whole strip: one segment per song in the set, in order.
///
/// **One per song, not five.** `1o` draws five because its example set is five
/// songs long; a strip that always drew five would be a decoration that lies
/// about a twelve-song set, which is the one thing a progress indicator cannot
/// do and still be one.
///
/// Empty for a set with nothing playable in it — a strip of no segments is
/// drawn as nothing at all, which is right: there is no progress through a set
/// that has nothing in it.
///
/// The index is clamped through [`playing_at`], so the accent segment is always
/// the song actually on the stand. Without that, an index past the end (two
/// songs removed from the set on another screen while it was playing) would
/// paint every segment `Played` and leave the strip claiming a gig had finished
/// while its last chart was still up.
pub fn strip_segments(index: usize, len: usize) -> Vec<Segment> {
    let Some((index, _)) = playing_at(index, len) else {
        return Vec::new();
    };
    (0..len)
        .map(|i| match i.cmp(&index) {
            std::cmp::Ordering::Less => Segment::Played,
            std::cmp::Ordering::Equal => Segment::Current,
            std::cmp::Ordering::Greater => Segment::Upcoming,
        })
        .collect()
}

/// How much space to leave between two segments of the strip, for a set this
/// long.
///
/// The wireframe's 3px gap is right for its five segments and wrong for a real
/// set, and it is the *gap* that goes wrong first rather than the segments. The
/// strip has about 365px to live in on the 393px viewport the designs assume
/// (`SCREEN_PAD`-ish padding off each end), so at a fixed 3px: five songs is 12
/// of those pixels spent on gaps, twenty is 57, forty is 117, and by sixty the
/// strip is more gap than segment and every song is a two-pixel sliver between
/// two three-pixel holes.
///
/// So the gap shrinks as the set grows, and — this is the part worth stating —
/// it is allowed to reach nought. A very long set stops being a row of segments
/// and becomes one continuous bar with the colour changing where the set has
/// got to, which is exactly what a progress bar for sixty songs should look
/// like anyway. It degrades into the right thing instead of into slivers, and
/// it does it without a second drawing path that would only ever be seen by
/// somebody with a sixty-song set.
///
/// The alternative — capping the segment count, or letting the strip scroll —
/// was rejected on the same ground: this is a thing glanced at from a metre
/// away between songs. A strip that has to be scrolled to be read is not a
/// glance, and a strip that stops counting at 40 is lying again.
pub fn strip_gap(len: usize) -> u32 {
    match len {
        0..=12 => 3,
        13..=24 => 2,
        25..=48 => 1,
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Setlist song picker (`1i`)
// ---------------------------------------------------------------------------

/// How far back "Recent" reaches, in days.
///
/// The wireframe names the chip and stops there, so the meaning is decided
/// here. A season is the unit a hobbyist's repertoire actually turns over in:
/// short enough that ninety days genuinely narrows a three-hundred-song book,
/// long enough that somebody who plays once a month still recognises the list
/// as theirs. A rank ("the twenty most recent") was the alternative and was
/// rejected — it can never say "you have not played anything lately", which is
/// the one honest answer this chip sometimes has to give.
pub const RECENT_DAYS: i64 = 90;

/// The picker's filter chips: All · Solid · Recent · Tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongFilter {
    All,
    /// Confidence `Solid` — the same word the library's first group header
    /// uses, and the same word the Songs screen counts in accent.
    Solid,
    /// Played *or* added inside [`RECENT_DAYS`].
    ///
    /// Both halves, because both are "fresh": a song played last week is still
    /// in the fingers, and a song typed in this morning is the reason the
    /// picker is probably open at all. Either one alone leaves out a case the
    /// user would call recent.
    Recent,
    /// Carries a tag. Which tag is a second choice — see
    /// [`chosen_tag_matches`] and the tag row the sheet reveals.
    Tag,
}

impl SongFilter {
    pub fn label(self) -> &'static str {
        match self {
            SongFilter::All => "All",
            SongFilter::Solid => "Solid",
            SongFilter::Recent => "Recent",
            SongFilter::Tag => "Tag",
        }
    }

    pub const ALL: [SongFilter; 4] = [
        SongFilter::All,
        SongFilter::Solid,
        SongFilter::Recent,
        SongFilter::Tag,
    ];
}

/// Whether a song carries `tag`, or — with no tag chosen — any tag at all.
///
/// Tags are compared the way [`parse_tags`] dedupes them: case-insensitively,
/// because `Campfire` and `campfire` are one tag to everyone but a byte
/// comparison.
fn chosen_tag_matches(song: &Song, tag: Option<&str>) -> bool {
    match tag {
        Some(tag) => song.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)),
        None => !song.tags.is_empty(),
    }
}

/// Played or added within [`RECENT_DAYS`] of `today`.
///
/// `today` is a parameter rather than a call to [`Day::today`] so the rule can
/// be tested without the system clock deciding the answer.
fn is_recent(song: &Song, today: Day) -> bool {
    let cutoff = today.days_since_epoch() - RECENT_DAYS;
    let played = song
        .last_played
        .is_some_and(|d| d.days_since_epoch() >= cutoff);
    // `created_at` is epoch milliseconds; the whole days in it are what the
    // cutoff is expressed in.
    let added = (song.created_at / 1000 / 86_400) as i64 >= cutoff;
    played || added
}

/// Whether one song survives the active filter chip.
pub fn matches_filter(song: &Song, filter: SongFilter, tag: Option<&str>, today: Day) -> bool {
    match filter {
        SongFilter::All => true,
        SongFilter::Solid => song.confidence == Some(Confidence::Solid),
        SongFilter::Recent => is_recent(song, today),
        SongFilter::Tag => chosen_tag_matches(song, tag),
    }
}

/// The picker's list: the library, searched, filtered, and sorted A–Z by title.
///
/// Title order rather than the library's own sort because this list is flat —
/// the grouping the library sort is paired with is not here — and because a
/// picker is used with a song already in mind. It is fixed rather than
/// tracking the ticks: a list that reordered itself as rows were ticked would
/// move the next row out from under a finger already on its way down.
pub fn picker_songs(
    songs: Vec<Song>,
    query: &str,
    filter: SongFilter,
    tag: Option<&str>,
    today: Day,
) -> Vec<Song> {
    let mut songs: Vec<Song> = filter_songs(songs, query)
        .into_iter()
        .filter(|song| matches_filter(song, filter, tag, today))
        .collect();
    sort_songs(&mut songs, SortField::Title, SortDir::Asc);
    songs
}

/// Every tag in the book, once each, A–Z.
///
/// Deduped case-insensitively and keeping the spelling that was typed first,
/// which is the rule [`parse_tags`] already applies inside one song — applied
/// across the whole book so the tag row does not offer `campfire` twice.
pub fn library_tags(songs: &[Song]) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for song in songs {
        for tag in &song.tags {
            if !tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                tags.push(tag.clone());
            }
        }
    }
    tags.sort_by_key(|t| t.to_lowercase());
    tags
}

/// Every distinct tuning in the book, once each, A–Z — the filter sheet's
/// (card G3) Tuning facet, and [`library_tags`]'s counterpart. Deduped and
/// sorted the same case-insensitive way, for the same reason: "Drop D" and
/// "drop d" are one tuning to everyone who did not happen to type it twice.
pub fn library_tunings(songs: &[Song]) -> Vec<String> {
    let mut tunings: Vec<String> = Vec::new();
    for song in songs {
        if let Some(tuning) = &song.tuning
            && !tunings.iter().any(|t| t.eq_ignore_ascii_case(tuning))
        {
            tunings.push(tuning.clone());
        }
    }
    tunings.sort_by_key(|t| t.to_lowercase());
    tunings
}

/// The running count beside the picker's title: `2 picked`.
///
/// Nothing at zero. An empty slot is not this app's house style, and a count
/// that appears with the first tick is the feedback the tick is asking for.
pub fn picked_note(picked: usize) -> String {
    match picked {
        0 => String::new(),
        1 => "1 picked".to_string(),
        n => format!("{n} picked"),
    }
}

/// The picker's footer button: `Add 2 songs`.
///
/// Zero reads `Add songs` rather than `Add 0 songs`; the button is disabled
/// there anyway, and the plural is what the row it sits under promises.
pub fn add_songs_label(picked: usize) -> String {
    match picked {
        0 => "Add songs".to_string(),
        1 => "Add 1 song".to_string(),
        n => format!("Add {n} songs"),
    }
}

/// What the picker says when nothing survives the search and the chip.
///
/// The search text wins when there is any, because a typed query is the thing
/// the user just did; otherwise the chip explains itself in its own terms.
pub fn picker_empty_note(filter: SongFilter, tag: Option<&str>, query: &str) -> String {
    let query = query.trim();
    if !query.is_empty() {
        return format!("Nothing in your book matches \u{201c}{query}\u{201d}.");
    }
    match filter {
        SongFilter::All => "Your book has no songs in it yet.".to_string(),
        SongFilter::Solid => "No song is marked solid yet.".to_string(),
        SongFilter::Recent => {
            format!("Nothing played or added in the last {RECENT_DAYS} days.")
        }
        SongFilter::Tag => match tag {
            Some(tag) => format!("Nothing is tagged \u{201c}{tag}\u{201d}."),
            None => "No song carries a tag yet.".to_string(),
        },
    }
}

// ---------------------------------------------------------------------------
// Sort & group sheet (`2c`)
// ---------------------------------------------------------------------------

/// How many songs have a given metadata field filled in.
pub fn field_fill_count(songs: &[Song], field: SortField) -> usize {
    songs.iter().filter(|s| field.is_present(s)).count()
}

/// Whether the sort sheet greys a field out. "Almost nobody has filled this
/// in" is fewer than one song in four — which leaves capo and tuning greyed in
/// a typical book while last-played, filled by a handful of gigs, stays live.
/// A greyed field is still selectable; the grey only says it will not help.
pub fn is_sparse_field(songs: &[Song], field: SortField) -> bool {
    !songs.is_empty() && field_fill_count(songs, field) * 4 < songs.len()
}

/// The arrow beside a sort row. It points the way the row's own words read,
/// which is not always the way the field sorts: `Asc` on a date field means
/// "newest first", and newest-first reads downward. Everything else ascends.
pub fn direction_arrow(field: SortField, dir: SortDir) -> TablerIcon {
    let downward = matches!(field, SortField::LastPlayed | SortField::DateAdded);
    match (dir, downward) {
        (SortDir::Asc, false) | (SortDir::Desc, true) => TablerIcon::ArrowNarrowUp,
        (SortDir::Asc, true) | (SortDir::Desc, false) => TablerIcon::ArrowNarrowDown,
    }
}

/// The greyed row's explanation: `3 songs have this`.
pub fn fill_note(count: usize) -> String {
    if count == 1 {
        "1 song has this".to_string()
    } else {
        format!("{count} songs have this")
    }
}

// ---------------------------------------------------------------------------
// Filter sheet (no wireframe — card G3)
// ---------------------------------------------------------------------------

/// The `Filter` chip's own label: muted `Filter` with nothing picked,
/// `Filter · N` the instant anything is — the same "count in the chip"
/// treatment `crate::store::Filters::count` exists to feed.
pub fn filter_chip_label(selected: usize) -> String {
    if selected == 0 {
        "Filter".to_string()
    } else {
        format!("Filter · {selected}")
    }
}

/// The Songs screen's sub-line, and the number it colours in accent beside it,
/// together — so the two can never disagree about what they are each counting.
///
/// **`41 solid` used to be a fact about the whole book; it is now a fact about
/// whatever the filter left in front of you.** A book of 300 with 41 solid
/// songs that a filter narrows to 12 would otherwise go on claiming 41 while
/// the list under it can show at most 12 — so both numbers here are read off
/// the *filtered* list, [`filter_by`]'s own output, not the unfiltered book.
///
/// The lead clause only switches away from `"N in your book · "` once a
/// filter is actually doing something: an inactive `Filters` leaves `shown`
/// equal to `all.len()`, and printing "12 of 12" on an ordinary, unfiltered
/// library would be arithmetic nobody asked for, the same objection
/// [`search_count_line`] raises about mixing two kinds of total.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibrarySubtitle {
    /// `"12 of 300 · "` or `"300 in your book · "` — the words before the
    /// accent-coloured solid count.
    pub lead: String,
    pub solid: usize,
}

/// The filter sheet's own footer note, alongside its `Clear`/`Done` pair —
/// [`fill_note`] above is the sort sheet's counterpart, and this is the same
/// kind of sentence for the same reason: a footer that only ever draws a
/// static hint is not telling anybody what the sheet in front of them is
/// currently set to do.
pub fn filter_sheet_note(selected: usize) -> String {
    match selected {
        0 => "Nothing selected — every song shows.".to_string(),
        1 => "1 filter selected.".to_string(),
        n => format!("{n} filters selected."),
    }
}

pub fn library_subtitle(all: Vec<Song>, filters: &Filters) -> LibrarySubtitle {
    let total = all.len();
    let shown = filter_by(all, filters);
    let solid = shown
        .iter()
        .filter(|s| s.confidence == Some(Confidence::Solid))
        .count();
    let lead = if filters.is_active() {
        format!("{} of {total} · ", shown.len())
    } else {
        format!("{total} in your book · ")
    };
    LibrarySubtitle { lead, solid }
}

// ---------------------------------------------------------------------------
// Add / edit song (`1j`)
// ---------------------------------------------------------------------------

/// One row of the artist autocomplete: an artist already in the book, spelled
/// the way the book spells it, and how many songs are filed under it.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtistSuggestion {
    pub name: String,
    pub songs: usize,
}

/// How many suggestions the artist field offers at once. Four fits under the
/// input without pushing "More details" off a phone screen, and a fifth
/// near-match has never been the one you meant.
pub const ARTIST_SUGGESTION_LIMIT: usize = 4;

/// Every artist in the book, folded case-insensitively.
///
/// Derived rather than stored: there is no artist table, and there should not
/// be one — an artist exists exactly as long as a song credits them, and a
/// separate list would need reconciling on every edit and every delete.
///
/// A book that holds both "the beatles" and "The Beatles" has one artist with
/// two spellings, not two artists. The spelling offered back is the one used
/// most; ties go to whichever sorts first, so the same library always suggests
/// the same word however the songs happen to be ordered.
pub fn known_artists(songs: &[Song]) -> Vec<ArtistSuggestion> {
    let mut folded: Vec<(String, Vec<String>)> = Vec::new();
    for song in songs {
        let name = song.artist.trim();
        if name.is_empty() {
            continue;
        }
        let key = name.to_lowercase();
        match folded.iter_mut().find(|(k, _)| *k == key) {
            Some((_, spellings)) => spellings.push(name.to_string()),
            None => folded.push((key, vec![name.to_string()])),
        }
    }

    let mut artists: Vec<ArtistSuggestion> = folded
        .into_iter()
        .map(|(_, spellings)| ArtistSuggestion {
            songs: spellings.len(),
            name: commonest_spelling(&spellings),
        })
        .collect();
    artists.sort_by_key(|a| a.name.to_lowercase());
    artists
}

/// The spelling used by the most songs, with ties broken alphabetically so the
/// answer does not depend on the order the songs arrived in.
fn commonest_spelling(spellings: &[String]) -> String {
    let mut distinct: Vec<&String> = Vec::new();
    for spelling in spellings {
        if !distinct.contains(&spelling) {
            distinct.push(spelling);
        }
    }
    distinct.sort();
    distinct
        .into_iter()
        .max_by_key(|candidate| spellings.iter().filter(|s| s == candidate).count())
        .cloned()
        .unwrap_or_default()
}

/// The artists to offer for what has been typed so far.
///
/// Matching is case-insensitive — someone typing `led zep` mid-practice should
/// not have to reach for the shift key — but nothing here rewrites the field.
/// What gets stored is what was typed, unless a suggestion is actually tapped.
///
/// Names that *start* with the typed text come before names that merely
/// contain it, because the first is nearly always what was meant and the
/// second is what saves you when you remember the surname first.
///
/// An empty field asks no question, and neither does a name typed out in full:
/// both return nothing, so the list appears while it can help and gets out of
/// the way the moment it cannot.
pub fn artist_suggestions(songs: &[Song], typed: &str) -> Vec<ArtistSuggestion> {
    let typed = typed.trim().to_lowercase();
    if typed.is_empty() {
        return Vec::new();
    }

    let known = known_artists(songs);
    if known.iter().any(|a| a.name.to_lowercase() == typed) {
        return Vec::new();
    }

    let mut opens: Vec<ArtistSuggestion> = Vec::new();
    let mut holds: Vec<ArtistSuggestion> = Vec::new();
    for artist in known {
        let folded = artist.name.to_lowercase();
        if folded.starts_with(&typed) {
            opens.push(artist);
        } else if folded.contains(&typed) {
            holds.push(artist);
        }
    }
    opens.append(&mut holds);
    opens.truncate(ARTIST_SUGGESTION_LIMIT);
    opens
}

/// The grey half of a suggestion row: `· 12 songs`.
pub fn artist_count_note(songs: usize) -> String {
    if songs == 1 {
        "· 1 song".to_string()
    } else {
        format!("· {songs} songs")
    }
}

/// A number typed into one of the optional metadata fields.
///
/// Blank is "not set", and so is zero: a capo on the nut is no capo, and a
/// song at nought beats a minute is a song nobody has timed. Anything that is
/// not a plain positive number stays unset rather than being guessed at.
pub fn parse_count(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value: u32 = text.parse().ok()?;
    (value > 0).then_some(value)
}

/// `3:44` back into the 224 seconds `fmt_duration` made it from.
///
/// A bare number is minutes. The field's example reads `3:44`, so a lone `4`
/// typed under it means four minutes to everyone who has ever timed a song —
/// nothing about that field suggests four seconds.
pub fn parse_duration(text: &str) -> Option<u32> {
    let text = text.trim();
    let Some((minutes, seconds)) = text.split_once(':') else {
        return parse_count(text).map(|m| m * 60);
    };
    let minutes: u32 = parse_count(minutes).unwrap_or(0);
    let seconds = seconds.trim();
    // `3:4` is not three minutes and four seconds to anyone reading a clock.
    if seconds.len() != 2 || !seconds.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let seconds: u32 = seconds.parse().ok()?;
    if seconds > 59 {
        return None;
    }
    let total = minutes * 60 + seconds;
    (total > 0).then_some(total)
}

/// One line of comma-separated words into the tag list.
///
/// Two spellings of one tag would split a group header in two on the library
/// screen, so a repeat is dropped case-insensitively — but the spelling that
/// survives is the one that was typed first, not a lowercased version of it.
pub fn parse_tags(text: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for raw in text.split(',') {
        let tag = raw.trim();
        if tag.is_empty() {
            continue;
        }
        if tags.iter().any(|t| t.to_lowercase() == tag.to_lowercase()) {
            continue;
        }
        tags.push(tag.to_string());
    }
    tags
}

/// The tag list back into the one line the field edits.
pub fn format_tags(tags: &[String]) -> String {
    tags.join(", ")
}

// ---------------------------------------------------------------------------
// Settings (`1q`)
// ---------------------------------------------------------------------------
//
// Every right-hand side of a settings row is here rather than in the screen,
// for the reason card X1 gives: a summary computed inside `rsx!` is a sentence
// nothing can test, and these are the sentences that tell somebody what state
// their app is actually in. The screen is a list of labels and controls; what
// the controls *say* is below.

/// The right-hand side of "Attachments on device".
///
/// [`fmt_bytes`] was written for this row — its own doc comment cites the
/// `248 MB ›` the wireframe draws — so the only thing added here is the empty
/// case. A book with no charts in it reads `Nothing yet` rather than `0 B`,
/// because `0 B` is a measurement of a thing that is not there and reads like a
/// fault; a fresh install is not a fault.
pub fn attachments_note(bytes: u64) -> String {
    if bytes == 0 {
        return "Nothing yet".to_string();
    }
    fmt_bytes(bytes)
}

/// The right-hand side of "Saved webpages" — a count, or the same admission.
pub fn saved_pages_note(pages: usize) -> String {
    match pages {
        0 => "None yet".to_string(),
        n => n.to_string(),
    }
}

/// What a switch row says beside its switch.
///
/// The wireframe writes this lowercase (`off ▾`, from a sketch face that has no
/// case of its own to speak of); the hi-fi type set does not, and every other
/// value in this app's rows is sentence case. So `On` and `Off`.
///
/// It exists at all — rather than the switch alone carrying the state — because
/// a switch is a shape and this is a word, and on a phone at arm's length in a
/// pub the word is the one that survives.
pub fn on_off(on: bool) -> &'static str {
    if on { "On" } else { "Off" }
}

/// The right-hand side of "Library sort": `Artist · A to Z`.
///
/// Both halves come from [`SortField`]'s own vocabulary — the label the sort
/// sheet draws and the direction sentence it draws beside it — so this row and
/// the sheet it opens can never describe the same state in two different sets
/// of words.
pub fn library_sort_note(field: SortField, dir: SortDir) -> String {
    format!("{} · {}", field.label(), field.direction_label(dir))
}

/// What one density is called on screen.
///
/// Deliberately not [`Density::name`], which is the name the value is *stored*
/// under and must not be reworded — the same split `GroupBy` already makes and
/// says why.
pub fn density_label(density: Density) -> &'static str {
    match density {
        Density::Comfortable => "Comfortable",
        Density::Compact => "Compact",
    }
}

/// What one performance-mode theme is called on screen.
///
/// `Follow app` rather than `Automatic`: the handoff's sentence is *"performance
/// mode defaults to following the app theme, with a Settings option to force it
/// dark"*, and "follow the app" is the thing that is actually true. "Automatic"
/// would suggest something is being worked out from the room or the hour, and
/// nothing is.
pub fn performance_theme_label(theme: PerformanceTheme) -> &'static str {
    match theme {
        PerformanceTheme::FollowApp => "Follow app",
        PerformanceTheme::AlwaysDark => "Always dark",
    }
}

/// What the Backup section says about the last export, when nothing more
/// recent has happened to say instead (card I3).
///
/// The card's whole instruction is **"nag never, mention once"**, and that is
/// a rule about what this function may return rather than about where it is
/// drawn. So: one muted line, no exclamation, no count of days since, and
/// nothing that escalates the longer it has been. A library that has never
/// been exported says so once and then says the same thing forever; a
/// library exported this morning says today. There is deliberately no
/// threshold at which the wording changes, because a threshold is how a
/// mention becomes a nag.
///
/// `now` is handed in rather than read from the clock so the wording is
/// testable without waiting for tomorrow.
pub fn last_export_note(last_export_at: Option<i64>, now: Day) -> String {
    let Some(ms) = last_export_at else {
        return "Not backed up yet".to_string();
    };
    // Milliseconds since the epoch, floored to the calendar day the same way
    // `Day::today` does it — the day is all this line ever says, so the wall
    // clock inside it is not worth carrying.
    let day = Day::from_days_since_epoch(ms.div_euclid(86_400_000));
    if day == now {
        "Exported today".to_string()
    } else {
        format!("Exported {}", day.short())
    }
}

/// The name of the accent the app is *painted in*, which is not always the name
/// of the accent that was *chosen*.
///
/// The row says "Rust" when [`AccentChoice::FromSystem`] fell all the way
/// through to Rust, because the screen is rust-coloured and the note's whole
/// contract is to name the colour the pixels actually are rather than the
/// control that was tapped. The chip two lines to the left goes on saying
/// "System", because the *control* is still doing what it says — that split is
/// argued in `AccentChoice::label`.
///
/// **This doc comment used to say that `FromSystem` resolves to Rust
/// *because* wallpaper extraction was a platform call Rinch did not expose.**
/// Card K8 made that false and did not come back here for it, which is how the
/// house style earns its keep: a comment describing an absent call while the
/// tests below assert the present one. There are two device colours now, they
/// are asked in a fixed order, and the word this returns says which one
/// answered — see `AccentChoice::resolve` for the order and
/// [`theme::AccentSource`](crate::theme::AccentSource) for why the difference
/// is visible to the user at all.
pub fn accent_note(
    accent: AccentChoice,
    palette: Option<Rgb>,
    wallpaper: Option<Rgb>,
) -> &'static str {
    accent.resolve(palette, wallpaper).name()
}

/// Whether the app paints itself dark, given what the user chose and what the
/// system last said.
///
/// The whole of card K8's theme resolution, as a two-argument function with no
/// store and no platform in it — which is the point. "Follow system" is a
/// three-way choice crossed with a three-way reading (night, day, and a
/// platform that declined to answer), and a table that small is either pinned
/// by tests or discovered on a phone at sunset.
///
/// **`None` resolves to light**, and that is this app's default rather than a
/// reading of the platform's. `platform::night_mode`'s own doc comment is
/// emphatic that `UI_MODE_NIGHT_UNDEFINED` is a real third answer and not a
/// failure code — some OEM skins and every non-phone UI mode sit in it for the
/// life of the process, and so does every desktop build — so somebody has to
/// decide what an app does when the system has no opinion. Light, because it
/// is what this app did before this card for every user who had never touched
/// the switch, and because the alternative (guessing dark) would flip an
/// existing install's appearance on upgrade on exactly the devices that cannot
/// tell us they meant it.
///
/// An explicit `Light` or `Dark` ignores the reading completely. That is worth
/// stating because it is the half people assume and nothing enforced until
/// this function existed: somebody who has said "Dark" has said it about this
/// app, and a sunset must not move it.
pub fn dark_active(choice: ThemeChoice, night: Option<bool>) -> bool {
    match choice {
        ThemeChoice::Light => false,
        ThemeChoice::Dark => true,
        ThemeChoice::FollowSystem => night.unwrap_or(false),
    }
}

/// Whether the chrome around a route is dark **whatever the app's theme says**.
///
/// [`Route::dark_chrome`] used to be the whole of this rule and is now half of
/// it, which is exactly the split its own doc comment predicted: *"the day the
/// Settings screen grows that toggle this stops being a question the route alone
/// can answer — it becomes route and `SettingsStore::performance_theme`"*. H1 is
/// that day.
///
/// The route half is the attachment viewer (`1k`), which the handoff fixes as
/// dark chrome regardless of theme. The setting half is performance mode (`1o`),
/// which follows the app theme unless somebody has said otherwise — and a
/// person who has said otherwise is standing in front of an audience with a
/// phone on a stand, which is the whole reason the option exists.
///
/// It is a free function taking both, rather than a method on either, because
/// its three call sites do not share a type: two are in [`crate::app`] (the
/// strip behind the status bar, and whether Android draws its clock in dark
/// glyphs) and the third is performance mode's own root. A rule read in three
/// places is a rule that has to be written once.
pub fn dark_chrome(route: Route, performance_theme: PerformanceTheme) -> bool {
    route.dark_chrome()
        || (matches!(route, Route::Performance(_))
            && performance_theme == PerformanceTheme::AlwaysDark)
}

/// Whether a route's subject — the song or setlist it is a screen for — has
/// been deleted out from under it.
///
/// Card J7: delete a song from its own detail screen and the delete commits
/// but the screen does not know to leave, because nothing told the *route* to.
/// `song_detail.rs`'s "This song is gone." was the only answer there was, and
/// it only ever caught a route arrived at some other way — it cannot save you
/// from a delete that fires while you are looking at the very row being
/// deleted, because a handler that fires the delete and a handler that leaves
/// the screen are two different pieces of code, and `SongMenuItems`' delete
/// item is shared with a library row's long-press, which must *not* navigate.
/// So this is asked of the route, the same shape as [`dark_chrome`] above it
/// and [`crate::keep_awake::wanted`]: a pure recomputation of "does this
/// screen still have something to show", with no handler anywhere that has to
/// remember to ask it.
///
/// `song_exists`/`setlist_exists` are handed in rather than a store, so the
/// rule can be checked against every `Route` variant with a plain closure over
/// a `Vec` or a `HashSet` and no database, exactly the way [`keep_awake::wanted`]
/// is checked against a recorded sequence rather than a screen lock. The one
/// caller that matters, `crate::app`'s effect, hands in `SongsStore::get` and
/// `SetlistsStore::get` themselves.
///
/// Matched without a wildcard arm on purpose: a new route that carries a song
/// or a setlist and is added here without a line in this function is a
/// compile error, not a screen that quietly forgets to leave.
pub fn route_orphaned(
    route: Route,
    song_exists: impl Fn(SongId) -> bool,
    setlist_exists: impl Fn(SetlistId) -> bool,
) -> bool {
    match route {
        Route::SongDetail(id) | Route::EditSong(id) => !song_exists(id),
        Route::TypeChart { song, .. }
        | Route::CaptureWebpage { song }
        | Route::ViewAttachment { song, .. } => !song_exists(song),
        Route::SetlistDetail(id) | Route::Performance(id) => !setlist_exists(id),
        // `SaveShared` is subject-less in the same way `AddSong` is, and for
        // a sharper version of the same reason: the song it will end up
        // attached to is the question the screen exists to ask, so there is
        // nothing on this route for a deleted record to orphan.
        Route::Library
        | Route::Setlists
        | Route::Settings
        | Route::AddSong
        | Route::Search
        | Route::SaveShared => false,
    }
}

/// Whether `Route::Library` should render the first-run screen (`1r`, card
/// H3) instead of the library it normally shows.
///
/// **There is no persisted "has run" flag, and this is why.** An empty book
/// has exactly one job regardless of how it got that way — a fresh install
/// that has never held a song, or a library somebody just emptied by
/// deleting the last one — and `1r`'s single question is the right answer to
/// both. A flag would only ever answer the first: it would have to be set
/// once and never again, which means the second case (an empty book that
/// *has* run before) reads the flag, finds it already tripped, and shows the
/// library's old "nothing here" text anyway — the exact two-answers-to-one-
/// question problem this card was written to remove. Recomputing the
/// question from the one fact that actually decides it — is the book empty
/// right now — makes both cases the same case, and makes the screen
/// reachable again by nothing more than deleting every song, with nothing to
/// reset.
///
/// A free function taking the count rather than a method on `SongsStore`, in
/// the same spirit as [`route_orphaned`] just above it: the decision belongs
/// where the route decisions live — `crate::app`'s `Route::Library` arm — and
/// a pure `usize -> bool` is what a render closure and a unit test can both
/// call without either one standing up a store.
pub fn first_run_active(song_count: usize) -> bool {
    song_count == 0
}

/// The sentence for a library that never opened, shown for as long as
/// [`crate::store::Storage`] is running in its memory-only fallback — card
/// J4's audit found that this state had an engine (`Storage::open`'s own doc
/// comment says "the app runs on memory alone, the fault is recorded") and no
/// screen. Everything that could go wrong up to this point already has a
/// place to be seen — a failed write says so on the row it touched, a failed
/// export says so on the export row — but a library that failed to *open at
/// all* had nowhere to be seen except `Storage::fault`, a signal nothing ever
/// read, and stderr, which nobody but a developer at a terminal ever sees.
/// That left the worst state on this card's own list silent: a user typing
/// songs into an app that will forget every one of them the moment it closes,
/// with nothing on screen to say so.
///
/// **No action, on purpose.** Card J4's own instruction is that a state with
/// nothing the reader can do must say what happened and stop, rather than
/// invent a button that cannot help. There is no "Retry" that means anything
/// here — the directory that failed to open is still whatever it was, on the
/// same process, and the actual fixes (freeing disk space, restoring
/// permissions, whatever else was holding the directory's lock letting go of
/// it) all happen outside this app, most of them requiring a fresh launch to
/// even attempt. A button that reran `Storage::open` against an unchanged
/// directory would fail the same way and teach the user that trying again is
/// the answer, when closing the app and fixing the machine is.
///
/// A nought-or-one shape — `Option<&'static str>` rather than a bare `bool` —
/// because that is what `crate::app`'s render tree wants: a `for` over this,
/// the same pattern `captured_page::Shown::note` and `performance::empty_note`
/// already use for "one sentence, or nothing at all".
pub fn library_unavailable_note(is_persistent: bool) -> Option<&'static str> {
    (!is_persistent).then_some(LIBRARY_UNAVAILABLE)
}

/// The sentence itself. A `const` rather than a literal inside the function
/// above so a test can hold it to the two things it has to promise: that the
/// library did not open, and that nothing typed from here on is being kept.
pub const LIBRARY_UNAVAILABLE: &str =
    "This library could not be opened. Nothing you do here will be saved.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AttachmentId, Day};

    fn song(id: u32, title: &str, artist: &str) -> Song {
        Song::new(id, title, artist)
    }

    fn rated(id: u32, title: &str, artist: &str, confidence: Confidence) -> Song {
        let mut s = song(id, title, artist);
        s.confidence = Some(confidence);
        s
    }

    fn with_chart(mut s: Song, attachment: AttachmentId) -> Song {
        s.attachments.push(attachment);
        s.primary_attachment = Some(attachment);
        s
    }

    // ── grouping ────────────────────────────────────────────────────────

    #[test]
    fn confidence_groups_keep_their_fixed_order() {
        let songs = vec![
            song(1, "Unrated one", "A"),
            rated(2, "Learning one", "B", Confidence::Learning),
            rated(3, "Solid one", "C", Confidence::Solid),
            rated(4, "Rusty one", "D", Confidence::Rusty),
        ];
        let labels: Vec<String> = group_songs(songs, GroupBy::Confidence)
            .into_iter()
            .map(|g| g.label)
            .collect();
        assert_eq!(labels, ["Solid", "Rusty", "Learning", "Unrated"]);
    }

    #[test]
    fn empty_confidence_groups_are_dropped() {
        let songs = vec![
            rated(1, "One", "A", Confidence::Solid),
            rated(2, "Two", "B", Confidence::Solid),
        ];
        let groups = group_songs(songs, GroupBy::Confidence);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label, "Solid");
        assert_eq!(groups[0].songs.len(), 2);
    }

    #[test]
    fn songs_with_no_tuning_group_under_a_named_bucket() {
        let mut tuned = song(1, "Open", "A");
        tuned.tuning = Some("Open D".into());
        let groups = group_songs(vec![tuned, song(2, "Plain", "B")], GroupBy::Tuning);
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert!(labels.contains(&"Open D"));
        assert!(labels.contains(&"No tuning set"));
    }

    #[test]
    fn grouping_by_none_of_an_empty_library_yields_no_groups() {
        assert!(group_songs(Vec::new(), GroupBy::None).is_empty());
    }

    // ── the alphabet scrubber's presence set (card H4) ───────────────────

    #[test]
    fn present_letters_is_exactly_the_first_letters_a_library_has_songs_under() {
        let songs = vec![
            song(1, "Angel From Montgomery", "Prine"),
            song(2, "Avocado", "Nobody"),
            song(3, "Blackbird", "Beatles"),
        ];
        let groups = group_songs(songs, GroupBy::FirstLetter);
        let present = present_letters(&groups);
        assert_eq!(
            present,
            BTreeSet::from(["A".to_string(), "B".to_string()]),
            "two songs share A, so it appears once, and C never appears at all"
        );
    }

    #[test]
    fn a_title_starting_with_a_digit_buckets_under_that_digit_not_a_letter() {
        // `group_songs`'s own `FirstLetter` closure only falls back to "#"
        // when a title has *no* first character at all; a digit is a
        // character like any other and uppercases to itself. So "500 Miles"
        // lands in a "5" bucket, not a fictional catch-all one.
        let groups = group_songs(vec![song(1, "500 Miles", "Trad.")], GroupBy::FirstLetter);
        let present = present_letters(&groups);
        // The rail only draws A–Z (card H4's spec), so a "5" landing in this
        // set is harmless — nothing in the rail ever looks it up — but it
        // must still be the true answer to "what groups exist", which is what
        // this function is for. A caller drawing a rail is the one that gets
        // to decide non-letter buckets are out of scope, not this function
        // pretending they aren't there.
        assert_eq!(present, BTreeSet::from(["5".to_string()]));
    }

    #[test]
    fn a_song_with_no_title_at_all_falls_back_to_the_hash_bucket() {
        let groups = group_songs(vec![song(1, "", "Trad.")], GroupBy::FirstLetter);
        let present = present_letters(&groups);
        assert_eq!(present, BTreeSet::from(["#".to_string()]));
    }

    // ── filtering (card G3) ─────────────────────────────────────────────

    #[test]
    fn no_filters_selected_leaves_grouped_untouched() {
        let songs = vec![
            rated(1, "Solid one", "A", Confidence::Solid),
            rated(2, "Rusty one", "B", Confidence::Rusty),
        ];
        let groups = grouped(
            songs,
            GroupBy::Confidence,
            SortField::Title,
            SortDir::Asc,
            &Filters::default(),
        );
        let total: usize = groups.iter().map(|g| g.songs.len()).sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn grouped_counts_only_what_survives_the_filter() {
        let songs = vec![
            rated(1, "Solid one", "A", Confidence::Solid),
            rated(2, "Solid two", "B", Confidence::Solid),
            rated(3, "Rusty one", "C", Confidence::Rusty),
        ];
        let filters = Filters {
            confidences: vec![Some(Confidence::Solid)],
            ..Default::default()
        };
        let groups = grouped(songs, GroupBy::Confidence, SortField::Title, SortDir::Asc, &filters);
        // Rusty has nothing left in it once the filter runs first, so its
        // header does not appear at all — not an empty one.
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label, "Solid");
        assert_eq!(groups[0].songs.len(), 2);
    }

    #[test]
    fn has_chart_filter_excludes_a_song_with_no_attachments() {
        let bare = song(1, "Bare", "A");
        let charted = with_chart(song(2, "Charted", "B"), 9);
        let filters = Filters {
            has_chart: true,
            ..Default::default()
        };
        let found = filter_by(vec![bare, charted], &filters);
        assert_eq!(titles(&found), ["Charted"]);
    }

    #[test]
    fn library_tunings_lists_only_distinct_tunings_present() {
        let mut drop_d = song(1, "One", "A");
        drop_d.tuning = Some("Drop D".into());
        let mut also_drop_d = song(2, "Two", "B");
        also_drop_d.tuning = Some("drop d".into());
        let mut standard = song(3, "Three", "C");
        standard.tuning = Some("Standard".into());
        let untuned = song(4, "Four", "D");

        let tunings = library_tunings(&[drop_d, also_drop_d, standard, untuned]);
        assert_eq!(tunings, vec!["Drop D".to_string(), "Standard".to_string()]);
    }

    #[test]
    fn library_tunings_of_a_library_with_no_tunings_set_is_empty() {
        assert!(library_tunings(&[song(1, "One", "A")]).is_empty());
    }

    #[test]
    fn filter_chip_reads_plain_filter_at_zero() {
        assert_eq!(filter_chip_label(0), "Filter");
    }

    #[test]
    fn filter_chip_counts_selections_once_any_exist() {
        assert_eq!(filter_chip_label(3), "Filter · 3");
    }

    #[test]
    fn filter_sheet_note_says_nothing_is_narrowing_the_book_at_zero() {
        assert_eq!(filter_sheet_note(0), "Nothing selected — every song shows.");
    }

    #[test]
    fn filter_sheet_note_pluralizes_past_one() {
        assert_eq!(filter_sheet_note(1), "1 filter selected.");
        assert_eq!(filter_sheet_note(4), "4 filters selected.");
    }

    #[test]
    fn library_subtitle_reads_the_whole_book_with_no_filter_active() {
        let songs = vec![
            rated(1, "One", "A", Confidence::Solid),
            song(2, "Two", "B"),
        ];
        let subtitle = library_subtitle(songs, &Filters::default());
        assert_eq!(subtitle.lead, "2 in your book · ");
        assert_eq!(subtitle.solid, 1);
    }

    #[test]
    fn library_subtitle_counts_the_filtered_subset_once_a_filter_narrows_it() {
        let songs = vec![
            rated(1, "One", "A", Confidence::Solid),
            rated(2, "Two", "B", Confidence::Solid),
            rated(3, "Three", "C", Confidence::Rusty),
        ];
        let filters = Filters {
            confidences: vec![Some(Confidence::Solid)],
            ..Default::default()
        };
        let subtitle = library_subtitle(songs, &filters);
        assert_eq!(subtitle.lead, "2 of 3 · ");
        assert_eq!(subtitle.solid, 2);
    }

    // ── sorting ─────────────────────────────────────────────────────────

    #[test]
    fn songs_missing_the_sort_field_go_last_in_both_directions() {
        let mut fast = song(1, "Fast", "A");
        fast.tempo = Some(160);
        let mut slow = song(2, "Slow", "B");
        slow.tempo = Some(60);
        let blank = song(3, "Blank", "C");

        let mut asc = vec![blank.clone(), fast.clone(), slow.clone()];
        sort_songs(&mut asc, SortField::Tempo, SortDir::Asc);
        assert_eq!(titles(&asc), ["Slow", "Fast", "Blank"]);

        let mut desc = vec![blank, fast, slow];
        sort_songs(&mut desc, SortField::Tempo, SortDir::Desc);
        assert_eq!(titles(&desc), ["Fast", "Slow", "Blank"]);
    }

    #[test]
    fn artist_sort_ignores_case() {
        let mut songs = vec![
            song(1, "One", "the band"),
            song(2, "Two", "Adele"),
            song(3, "Three", "Zappa"),
        ];
        sort_songs(&mut songs, SortField::Artist, SortDir::Asc);
        assert_eq!(titles(&songs), ["Two", "One", "Three"]);
    }

    #[test]
    fn last_played_ascending_means_most_recent_first() {
        let mut old = song(1, "Old", "A");
        old.last_played = Some(Day::new(2026, 1, 2));
        let mut recent = song(2, "Recent", "B");
        recent.last_played = Some(Day::new(2026, 8, 14));

        let mut songs = vec![old, recent];
        sort_songs(&mut songs, SortField::LastPlayed, SortDir::Asc);
        assert_eq!(titles(&songs), ["Recent", "Old"]);
    }

    #[test]
    fn every_sort_field_is_stable_on_an_empty_list() {
        for field in SortField::ALL {
            let mut songs: Vec<Song> = Vec::new();
            sort_songs(&mut songs, field, SortDir::Asc);
            assert!(songs.is_empty());
        }
    }

    // ── search ──────────────────────────────────────────────────────────

    #[test]
    fn search_covers_title_artist_and_tags_case_insensitively() {
        let mut tagged = song(3, "Ripple", "Grateful Dead");
        tagged.tags = vec!["campfire".into()];
        let songs = vec![
            song(1, "Blackbird", "The Beatles"),
            song(2, "Landslide", "Fleetwood Mac"),
            tagged,
        ];

        assert_eq!(titles(&filter_songs(songs.clone(), "BLACK")), ["Blackbird"]);
        assert_eq!(titles(&filter_songs(songs.clone(), "fleetwood")), ["Landslide"]);
        assert_eq!(titles(&filter_songs(songs.clone(), "campfire")), ["Ripple"]);
        assert_eq!(filter_songs(songs.clone(), "   ").len(), 3);
        assert!(filter_songs(songs, "nothing here").is_empty());
    }

    // ── search & filter screen (`1p`) ───────────────────────────────────

    /// The runs of a highlight, flattened to something an assertion can read.
    fn runs(text: &str, query: &str) -> Vec<(String, bool)> {
        highlight(text, query)
            .into_iter()
            .map(|h| (h.text, h.matched))
            .collect()
    }

    /// The invariant every other highlight test is written on top of: whatever
    /// the segmentation does, putting the runs back together has to give the
    /// original string back, byte for byte. A highlighter that drops or
    /// duplicates a character is a search that rewrites the library.
    fn rejoins(text: &str, query: &str) {
        let joined: String = highlight(text, query).into_iter().map(|h| h.text).collect();
        assert_eq!(joined, text, "runs of {text:?} against {query:?}");
    }

    #[test]
    fn an_empty_query_marks_nothing_and_keeps_the_whole_string() {
        assert_eq!(runs("Blackbird", ""), [("Blackbird".to_string(), false)]);
        assert_eq!(runs("Blackbird", "   "), [("Blackbird".to_string(), false)]);
    }

    #[test]
    fn an_empty_string_has_no_runs_at_all() {
        // A song with no artist draws no meta line rather than an empty one.
        assert!(highlight("", "dylan").is_empty());
        assert!(highlight("", "").is_empty());
    }

    #[test]
    fn a_query_longer_than_the_text_matches_nothing() {
        assert_eq!(
            runs("Bob", "Bob Dylan and then some"),
            [("Bob".to_string(), false)]
        );
        rejoins("Bob", "Bob Dylan and then some");
    }

    #[test]
    fn a_match_at_the_very_start_leads_with_the_marked_run() {
        assert_eq!(
            runs("Blackbird", "black"),
            [("Black".to_string(), true), ("bird".to_string(), false)]
        );
    }

    #[test]
    fn a_match_at_the_very_end_trails_with_the_marked_run() {
        assert_eq!(
            runs("Blackbird", "bird"),
            [("Black".to_string(), false), ("bird".to_string(), true)]
        );
    }

    /// The case `1p` actually draws: `Dylan` inside `Bob Dylan`, marked in the
    /// middle of the string rather than at either end.
    #[test]
    fn a_match_in_the_middle_is_marked_where_it_sits() {
        assert_eq!(
            runs("Bob Dylan and The Band", "dylan"),
            [
                ("Bob ".to_string(), false),
                ("Dylan".to_string(), true),
                (" and The Band".to_string(), false),
            ]
        );
    }

    #[test]
    fn the_whole_string_matching_leaves_one_marked_run() {
        assert_eq!(runs("Ripple", "ripple"), [("Ripple".to_string(), true)]);
    }

    #[test]
    fn every_occurrence_is_marked_not_only_the_first() {
        assert_eq!(
            runs("Row, row, row your boat", "row"),
            [
                ("Row".to_string(), true),
                (", ".to_string(), false),
                ("row".to_string(), true),
                (", ".to_string(), false),
                ("row".to_string(), true),
                (" your boat".to_string(), false),
            ]
        );
    }

    /// Two adjacent matches must not be run together into one marked span with
    /// nothing between them, and must not lose the character that separates
    /// them either.
    #[test]
    fn back_to_back_matches_stay_two_runs() {
        assert_eq!(
            runs("abab", "ab"),
            [("ab".to_string(), true), ("ab".to_string(), true)]
        );
        rejoins("abab", "ab");
    }

    #[test]
    fn case_differs_in_either_direction_and_the_book_keeps_its_own_spelling() {
        // Query shouted at a quietly spelled book...
        assert_eq!(
            runs("the beatles", "BEAT"),
            [("the ".to_string(), false), ("beat".to_string(), true), ("les".to_string(), false)]
        );
        // ...and the other way round. The run is the library's spelling, never
        // the query's.
        assert_eq!(
            runs("The Beatles", "beat"),
            [("The ".to_string(), false), ("Beat".to_string(), true), ("les".to_string(), false)]
        );
    }

    #[test]
    fn a_query_padded_with_spaces_marks_what_the_matcher_matched_on() {
        // `filter_songs` trims before it decides a row is in the list, so the
        // highlighter has to trim too or the row arrives with nothing marked.
        assert_eq!(
            runs("Blackbird", "  black  "),
            [("Black".to_string(), true), ("bird".to_string(), false)]
        );
    }

    /// Multi-byte characters, which is where a byte-index slip is a panic
    /// rather than a wrong answer. Every one of these strings is the kind of
    /// thing this app really holds: an accented artist, a flat sign in a key
    /// (card K34), a title in a script with no ASCII in it at all.
    #[test]
    fn a_query_with_characters_that_are_not_ascii_does_not_slip_a_byte() {
        // A match *after* a multi-byte character: every byte offset past the
        // é is shifted, which is what the map exists to survive.
        assert_eq!(
            runs("Café Society", "society"),
            [("Café ".to_string(), false), ("Society".to_string(), true)]
        );
        // The non-ASCII character inside the match itself, case-folded.
        assert_eq!(
            runs("Für Elise", "FÜR"),
            [("Für".to_string(), true), (" Elise".to_string(), false)]
        );
        // A flat sign — not a letter, three bytes, and the exact character
        // card K34 is about.
        assert_eq!(
            runs("E♭ Major", "♭"),
            [("E".to_string(), false), ("♭".to_string(), true), (" Major".to_string(), false)]
        );
        // No ASCII anywhere, and a query that is a strict substring of it.
        assert_eq!(
            runs("さくらさくら", "くら"),
            [
                ("さ".to_string(), false),
                ("くら".to_string(), true),
                ("さ".to_string(), false),
                ("くら".to_string(), true),
            ]
        );

        for (text, query) in [
            ("Café Society", "society"),
            ("Für Elise", "FÜR"),
            ("E♭ Major", "♭"),
            ("さくらさくら", "くら"),
            ("Blowin' In The Wind", "’"),
            ("Ünïcödé", "cö"),
        ] {
            rejoins(text, query);
        }
    }

    /// The one case the byte map cannot answer, pinned so that it stays a wrong
    /// answer rather than becoming a panic. `İ` (U+0130) lowercases to two
    /// characters, so a query of `i` matches half of it and there is no
    /// substring of the original that is the matched part.
    #[test]
    fn a_match_inside_one_characters_case_expansion_is_skipped_not_sliced() {
        assert_eq!(runs("İstanbul", "i"), [("İstanbul".to_string(), false)]);
        rejoins("İstanbul", "i");

        // The rest of the same string still highlights normally, which is the
        // proof that the skip is one match and not the whole scan.
        assert_eq!(
            runs("İstanbul", "stan"),
            [
                ("İ".to_string(), false),
                ("stan".to_string(), true),
                ("bul".to_string(), false),
            ]
        );
        rejoins("İstanbul", "stan");
    }

    #[test]
    fn an_untyped_query_finds_nothing_where_the_library_filter_finds_everything() {
        let songs = vec![song(1, "Blackbird", "The Beatles"), song(2, "Ripple", "GD")];
        let sets = vec![set(1, "Porch, Saturday", vec![]), set(2, "Quiet set", vec![])];

        // The difference this pair of functions exists for: a screen that has
        // not been asked anything must not answer "all of them".
        assert_eq!(filter_songs(songs.clone(), "").len(), 2);
        assert!(search_songs(songs.clone(), "").is_empty());
        assert!(search_songs(songs, "   ").is_empty());

        assert_eq!(filter_setlists(sets.clone(), "").len(), 2);
        assert!(search_setlists(sets.clone(), "").is_empty());
        assert!(search_setlists(sets, "  ").is_empty());
    }

    #[test]
    fn results_are_a_to_z_by_title_whatever_the_library_sort_is() {
        let songs = vec![
            song(1, "Zimmerman", "Bob Dylan"),
            song(2, "Absolutely Sweet Marie", "Bob Dylan"),
            song(3, "Ripple", "Grateful Dead"),
            song(4, "buckets of rain", "Bob Dylan"),
        ];
        assert_eq!(
            titles(&search_songs(songs, "dylan")),
            ["Absolutely Sweet Marie", "buckets of rain", "Zimmerman"]
        );
    }

    #[test]
    fn matching_setlists_are_a_to_z_by_name() {
        let sets = vec![
            set(1, "Wedding, second set", vec![]),
            set(2, "wedding, first set", vec![]),
            set(3, "Porch, Saturday", vec![]),
        ];
        let names: Vec<String> = search_setlists(sets, "wedding")
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, ["wedding, first set", "Wedding, second set"]);
    }

    #[test]
    fn a_query_can_find_a_song_and_a_setlist_at_once() {
        let songs = vec![
            song(1, "Don't Think Twice", "Bob Dylan"),
            song(2, "Ripple", "Grateful Dead"),
        ];
        let sets = vec![set(1, "Dylan night", vec![1]), set(2, "Quiet set", vec![])];

        assert_eq!(titles(&search_songs(songs, "dylan")), ["Don't Think Twice"]);
        assert_eq!(search_setlists(sets, "dylan").len(), 1);
    }

    /// Which kinds of row a query produces, in order. The labels are enough to
    /// pin the shape of the screen without restating every hit.
    fn shape(rows: &[SearchRow]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                SearchRow::Heading { label, count, first } => {
                    format!("heading {label} {count} first={first}")
                }
                SearchRow::Song(hit) => format!("song {}", hit.song.title),
                SearchRow::Setlist(hit) => format!("setlist {}", hit.setlist.name),
                SearchRow::Attachment(hit) => format!("attachment {}", hit.song_title),
                SearchRow::Prompt => "prompt".to_string(),
                SearchRow::NoMatch(q) => format!("nomatch {q}"),
            })
            .collect()
    }

    fn book() -> (Vec<Song>, Vec<Setlist>) {
        let songs = vec![
            song(1, "Don't Think Twice", "Bob Dylan"),
            song(2, "Ripple", "Grateful Dead"),
            song(3, "Buckets Of Rain", "Bob Dylan"),
        ];
        let sets = vec![set(1, "Dylan night", vec![1, 3]), set(2, "Quiet set", vec![])];
        (songs, sets)
    }

    /// An attachment entry for `book()`'s "Ripple", carrying a body a caller
    /// can search — the fixture [`attachment_hits`]'s own tests build by hand.
    fn ripple_chart(kind: AttachmentKind, body: &str) -> AttachmentText {
        AttachmentText {
            song: 2,
            song_title: "Ripple".to_string(),
            attachment: 20,
            attachment_title: "Ripple chart".to_string(),
            kind,
            body: body.to_string(),
        }
    }

    #[test]
    fn an_untouched_screen_is_one_prompt_row_and_nothing_else() {
        let (songs, sets) = book();
        assert_eq!(
            shape(&search_rows(songs.clone(), sets.clone(), Vec::new(), "")),
            ["prompt"]
        );
        assert_eq!(
            shape(&search_rows(songs, sets, Vec::new(), "   ")),
            ["prompt"]
        );
    }

    #[test]
    fn a_query_nothing_answers_to_is_one_row_naming_it_back() {
        let (songs, sets) = book();
        // Trimmed, because the panel prints it and a quoted trailing space is
        // an answer that looks like a bug.
        assert_eq!(
            shape(&search_rows(songs, sets, Vec::new(), "  wedding ")),
            ["nomatch wedding"]
        );
    }

    #[test]
    fn a_query_that_finds_both_gets_a_heading_over_each_group() {
        let (songs, sets) = book();
        assert_eq!(
            shape(&search_rows(songs, sets, Vec::new(), "dylan")),
            [
                "heading Songs 2 first=true",
                "song Buckets Of Rain",
                "song Don't Think Twice",
                "heading Setlists 1 first=false",
                "setlist Dylan night",
            ]
        );
    }

    /// The third group, over a query that also happens to answer both of the
    /// others — the shape a real "what's playing this song" search produces.
    #[test]
    fn a_query_that_finds_all_three_gets_a_heading_over_each_group() {
        let (songs, sets) = book();
        let attachments = vec![ripple_chart(
            AttachmentKind::Text,
            "Verse:\nG       D\nIf my words did glow\n",
        )];
        assert_eq!(
            shape(&search_rows(songs, sets, attachments, "glow")),
            [
                "heading Inside attachments 1 first=false",
                "attachment Ripple",
            ]
        );
    }

    /// A group with no matches contributes nothing — not a heading over
    /// nothing. Card G1 left "Inside attachments" out of the wireframe on
    /// exactly this rule, before this card could answer it; this pins the same
    /// rule now that the group is real.
    #[test]
    fn a_group_with_no_matches_has_no_heading() {
        let (songs, sets) = book();
        assert_eq!(
            shape(&search_rows(songs.clone(), sets.clone(), Vec::new(), "ripple")),
            ["heading Songs 1 first=true", "song Ripple"]
        );
        assert_eq!(
            shape(&search_rows(songs.clone(), sets.clone(), Vec::new(), "quiet")),
            ["heading Setlists 1 first=false", "setlist Quiet set"]
        );
        // A query that answers the Songs group but not a single attachment
        // draws no "Inside attachments" heading over an empty group.
        let attachments = vec![ripple_chart(AttachmentKind::Text, "G D Em C\n")];
        assert_eq!(
            shape(&search_rows(songs, sets, attachments, "ripple")),
            ["heading Songs 1 first=true", "song Ripple"]
        );
    }

    /// The one lie [`search::NoMatch`](crate::screens::search) must not tell:
    /// a query that only lives inside a chart still needs the other two
    /// groups to come back empty before this screen says "nothing matches".
    #[test]
    fn a_query_that_only_a_chart_answers_is_not_a_nomatch() {
        let (songs, sets) = book();
        let attachments = vec![ripple_chart(
            AttachmentKind::CapturedPage,
            "the ocean sighed under a paper moon\n",
        )];
        assert_eq!(
            shape(&search_rows(songs, sets, attachments, "paper moon")),
            [
                "heading Inside attachments 1 first=false",
                "attachment Ripple",
            ]
        );
    }

    #[test]
    fn every_row_has_a_key_and_no_two_rows_share_one() {
        let (songs, sets) = book();
        let attachments = vec![ripple_chart(AttachmentKind::Text, "the E chord rings out\n")];
        for query in ["", "nothing at all", "dylan", "e"] {
            let rows = search_rows(songs.clone(), sets.clone(), attachments.clone(), query);
            let mut keys: Vec<String> = rows.iter().map(|r| r.key()).collect();
            let total = keys.len();
            keys.sort();
            keys.dedup();
            assert_eq!(keys.len(), total, "duplicate key for query {query:?}");
        }
    }

    /// The key is what makes rinch keep a row's DOM across a keystroke, and the
    /// mark is what has to change under it. So the key must be stable while the
    /// runs must not be — this pins both halves at once.
    #[test]
    fn a_row_keeps_its_key_across_a_keystroke_and_changes_its_marks() {
        let (songs, sets) = book();
        let before = search_rows(songs.clone(), sets.clone(), Vec::new(), "dyl");
        let after = search_rows(songs, sets, Vec::new(), "dyla");

        let song_key = |rows: &[SearchRow]| {
            rows.iter()
                .find(|r| matches!(r, SearchRow::Song(_)))
                .map(|r| r.key())
        };
        assert_eq!(song_key(&before), song_key(&after));
        assert_ne!(before, after);
    }

    #[test]
    fn a_result_marks_the_artist_in_its_meta_line_the_way_the_wireframe_does() {
        let hits = song_hits(vec![song(1, "Buckets Of Rain", "Bob Dylan")], "dylan");
        assert_eq!(hits.len(), 1);
        // Nothing in the title matches, so it is one unmarked run...
        assert_eq!(
            hits[0].title,
            [Highlight {
                text: "Buckets Of Rain".into(),
                matched: false
            }]
        );
        // ...and the mark is on the artist, mid-line, which is exactly what
        // `1p` draws.
        assert_eq!(
            hits[0].meta,
            [
                Highlight {
                    text: "Bob ".into(),
                    matched: false
                },
                Highlight {
                    text: "Dylan".into(),
                    matched: true
                },
            ]
        );
    }

    #[test]
    fn a_setlist_result_carries_the_same_sub_line_the_sheet_prints() {
        let songs = vec![durated(1, 224), durated(2, 138)];
        let hits = setlist_hits(vec![set(1, "Dylan night", vec![1, 2])], &songs, "night");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].summary, "2 songs · 6:02");
        // The summary is a count and a clock and is never marked, even when the
        // query would match digits in it.
        let digits = setlist_hits(vec![set(1, "Set 2", vec![1, 2])], &songs, "2");
        assert_eq!(digits[0].summary, "2 songs · 6:02");
    }

    // ── inside attachments (card G2) ────────────────────────────────────

    fn text_entry(song: SongId, title: &str, body: &str) -> AttachmentText {
        AttachmentText {
            song,
            song_title: title.to_string(),
            attachment: song as AttachmentId + 100,
            attachment_title: format!("{title} chart"),
            kind: AttachmentKind::Text,
            body: body.to_string(),
        }
    }

    /// Acceptance criterion: a word that appears only inside a typed chart
    /// finds it, and the result names its song.
    #[test]
    fn a_word_found_only_inside_a_typed_chart_names_its_song() {
        let entries = vec![text_entry(
            1,
            "Carolina In My Mind",
            "Capo 3. Eb shapes played as C.\n",
        )];
        let hits = attachment_hits(entries, "capo 3");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].song_title, "Carolina In My Mind");
        assert_eq!(hits[0].kind, AttachmentKind::Text);
    }

    /// The same claim, for a captured page rather than a typed chart — the
    /// other kind [`AttachmentText::body`] actually carries text for.
    #[test]
    fn a_word_found_only_inside_a_captured_page_names_its_song() {
        let entries = vec![AttachmentText {
            kind: AttachmentKind::CapturedPage,
            ..text_entry(7, "Hallelujah", "a cold and it's a broken hallelujah\n")
        }];
        let hits = attachment_hits(entries, "broken hallelujah");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].song_title, "Hallelujah");
        assert_eq!(hits[0].kind, AttachmentKind::CapturedPage);
    }

    #[test]
    fn a_query_absent_from_every_body_finds_no_attachments() {
        let entries = vec![text_entry(1, "Ripple", "G D Em C\n")];
        assert!(attachment_hits(entries.clone(), "zzyzx").is_empty());
        // Card G1's own rule, carried into this group: nothing typed answers
        // nothing, not everything.
        assert!(attachment_hits(entries, "").is_empty());
    }

    #[test]
    fn the_song_s_own_title_is_not_searched_a_second_time_here() {
        // "Ripple" is the song's title, not a word in its chart, so a query
        // for it must not surface this attachment — song_hits already found
        // it, and a second copy under "Inside attachments" would be the same
        // song listed twice for one keystroke.
        let entries = vec![text_entry(1, "Ripple", "G D Em C\n")];
        assert!(attachment_hits(entries, "ripple").is_empty());
    }

    #[test]
    fn attachment_results_sort_a_to_z_by_song_title() {
        let entries = vec![
            text_entry(1, "Zimmerman", "the word rings out\n"),
            text_entry(2, "Absolutely Sweet Marie", "the word rings out\n"),
        ];
        let hits = attachment_hits(entries, "rings out");
        let titles: Vec<&str> = hits.iter().map(|h| h.song_title.as_str()).collect();
        assert_eq!(titles, ["Absolutely Sweet Marie", "Zimmerman"]);
    }

    #[test]
    fn a_snippet_marks_the_match_the_same_way_every_other_group_does() {
        let snippet = attachment_snippet("the word rings out clearly", "rings").unwrap();
        assert_eq!(
            snippet,
            [
                Highlight { text: "the word ".into(), matched: false },
                Highlight { text: "rings".into(), matched: true },
                Highlight { text: " out clearly".into(), matched: false },
            ]
        );
    }

    #[test]
    fn no_match_in_the_body_is_no_snippet_at_all() {
        assert_eq!(attachment_snippet("the word rings out", "zzyzx"), None);
        assert_eq!(attachment_snippet("the word rings out", ""), None);
        assert_eq!(attachment_snippet("the word rings out", "   "), None);
    }

    /// A match far from either edge of a long body gets an ellipsis on both
    /// sides — the reader is told there is more chart above and below this
    /// line, not just handed a fragment that looks like the whole thing.
    #[test]
    fn a_match_in_the_middle_of_a_long_body_is_windowed_on_both_sides() {
        let filler = "x".repeat(200);
        let body = format!("{filler} the loud chord rings out {filler}");
        let snippet = attachment_snippet(&body, "rings").unwrap();
        let text: String = snippet.iter().map(|h| h.text.as_str()).collect();
        assert!(text.starts_with('…'), "{text}");
        assert!(text.ends_with('…'), "{text}");
        assert!(text.contains("rings"), "{text}");
        // The window is a fixed budget either side of the match, not the
        // whole four-hundred-character body.
        assert!(text.len() < 150, "window ran long: {} chars: {text}", text.len());
    }

    /// A match at the very start, or the very end, of a body gets an ellipsis
    /// only on the side that was actually cut — the honest half of the claim
    /// the ellipsis makes.
    #[test]
    fn a_match_at_either_edge_of_a_body_gets_only_one_ellipsis() {
        let filler = "x".repeat(200);
        let at_start = format!("rings out loud {filler}");
        let snippet = attachment_snippet(&at_start, "rings").unwrap();
        let text: String = snippet.iter().map(|h| h.text.as_str()).collect();
        assert!(!text.starts_with('…'), "{text}");
        assert!(text.ends_with('…'), "{text}");

        let at_end = format!("{filler} it rings out loud");
        let snippet = attachment_snippet(&at_end, "rings").unwrap();
        let text: String = snippet.iter().map(|h| h.text.as_str()).collect();
        assert!(text.starts_with('…'), "{text}");
        assert!(!text.ends_with('…'), "{text}");
    }

    /// A short body that fits inside the window entirely gets no ellipsis on
    /// either side — nothing was cut, so nothing should claim to be.
    #[test]
    fn a_short_body_gets_no_ellipsis_at_all() {
        let snippet = attachment_snippet("rings out", "rings").unwrap();
        let text: String = snippet.iter().map(|h| h.text.as_str()).collect();
        assert_eq!(text, "rings out");
    }

    /// The chart's own significant whitespace — a chord sitting several tabs
    /// above its syllable — collapses to one space, and a newline separating
    /// two lines of a captured page's extracted text does too, because a
    /// snippet is one line by the card's own instruction and cannot draw
    /// either faithfully.
    #[test]
    fn a_snippet_is_always_one_line_whatever_shape_the_source_text_had() {
        // Short enough that the whole thing fits inside the window (so no
        // ellipsis complicates the expected string) and carries both kinds of
        // whitespace a real chart does: a chord's alignment spacing and the
        // newline between two lines.
        let body = "G   D\nCarolina in my mind\n";
        let snippet = attachment_snippet(body, "Carolina in").unwrap();
        let text: String = snippet.iter().map(|h| h.text.as_str()).collect();
        assert!(!text.contains('\n'), "{text}");
        assert_eq!(text, "G D Carolina in my mind");
    }

    /// Card G2's own instruction: build a library at the stated target size —
    /// 300 songs — and time a query against it, rather than trust that "it's
    /// just a scan" stays true as the code around it changes.
    ///
    /// Half the library gets a typed chart and half a captured page, each a
    /// few dozen lines of realistic filler — a chord line over a lyric line,
    /// repeated — so the scan has an actual corpus to walk rather than three
    /// hundred empty strings timing nothing. One attachment, buried in the
    /// middle of the book, carries a line the other 299 do not.
    ///
    /// The bound is generous on wall-clock time (CI boxes are noisy) and tight
    /// on what it would take to blow through: 300 attachments of a couple of
    /// kilobytes each is a few hundred KB total, and a linear scan over that
    /// is sub-millisecond work on any machine this runs on. A quadratic
    /// mistake — re-scanning the whole corpus once per attachment instead of
    /// once each, say, or rebuilding every song's own text on every one of
    /// its own attachments — would multiply that by the same 300 and land
    /// somewhere this bound catches on the first run, not on a flake.
    #[test]
    fn searching_a_three_hundred_song_library_s_attachments_stays_fast() {
        use std::time::Instant;

        const LIBRARY_SIZE: u32 = 300;
        const NEEDLE: &str = "zzyzxquery";

        let mut songs = Vec::with_capacity(LIBRARY_SIZE as usize);
        let mut attachments = Vec::with_capacity(LIBRARY_SIZE as usize);
        for i in 0..LIBRARY_SIZE {
            let mut s = song(i, &format!("Song {i}"), &format!("Artist {}", i % 40));
            let attachment_id = i + 1000;
            s.attachments.push(attachment_id);

            let mut body = String::new();
            for line in 0..30 {
                body.push_str(&format!(
                    "G       D        Em       C  -- verse {line} of song {i}\n"
                ));
            }
            let kind = if i % 2 == 0 {
                AttachmentKind::Text
            } else {
                AttachmentKind::CapturedPage
            };
            attachments.push(AttachmentText {
                song: s.id,
                song_title: s.title.clone(),
                attachment: attachment_id,
                attachment_title: format!("chart {i}"),
                kind,
                body,
            });
            songs.push(s);
        }
        // Song 150's chart is the only one that answers the query.
        attachments[150]
            .body
            .push_str(&format!("this line hides the word {NEEDLE} in it\n"));

        let start = Instant::now();
        let rows = search_rows(songs, Vec::new(), attachments, NEEDLE);
        let elapsed = start.elapsed();

        assert_eq!(
            shape(&rows),
            ["heading Inside attachments 1 first=false", "attachment Song 150"],
            "the scan found the one hit and nothing else"
        );
        assert!(
            elapsed.as_millis() < 200,
            "search over a {LIBRARY_SIZE}-song library's attachments took {elapsed:?}, \
             expected well under 200ms — see this test's own comment before raising the bound"
        );
        eprintln!(
            "searching_a_three_hundred_song_library_s_attachments_stays_fast: {elapsed:?} \
             for {LIBRARY_SIZE} songs"
        );
    }

    #[test]
    fn the_count_line_counts_each_group_against_its_own_total() {
        // The wireframe's own example, plus the setlist half it never had.
        assert_eq!(
            search_count_line(6, 300, 1, 2),
            "6 of 300 songs · 1 of 2 setlists · sorted A–Z"
        );
    }

    #[test]
    fn the_count_line_says_song_and_setlist_when_the_book_holds_one_of_each() {
        // Pluralised on the total, not on the number found: "1 of 300 song"
        // would be a sentence about the wrong number.
        assert_eq!(
            search_count_line(1, 300, 0, 2),
            "1 of 300 songs · 0 of 2 setlists · sorted A–Z"
        );
        assert_eq!(
            search_count_line(1, 1, 1, 1),
            "1 of 1 song · 1 of 1 setlist · sorted A–Z"
        );
    }

    #[test]
    fn the_count_line_drops_the_setlist_clause_for_a_book_with_no_sets() {
        assert_eq!(search_count_line(2, 25, 0, 0), "2 of 25 songs · sorted A–Z");
    }

    // ── truncation ──────────────────────────────────────────────────────

    #[test]
    fn a_group_shows_a_preview_until_expanded() {
        let songs: Vec<Song> = (0..10).map(|i| song(i, &format!("Song {i}"), "A")).collect();
        assert_eq!(visible_songs(&songs, false).len(), GROUP_PREVIEW);
        assert_eq!(visible_songs(&songs, true).len(), 10);
    }

    #[test]
    fn a_short_group_is_never_truncated() {
        let songs: Vec<Song> = (0..3).map(|i| song(i, &format!("Song {i}"), "A")).collect();
        assert_eq!(visible_songs(&songs, false).len(), 3);
    }

    // ── setlist arithmetic ──────────────────────────────────────────────

    #[test]
    fn cumulative_starts_are_the_running_clock() {
        let songs = vec![
            durated(1, 224), // 0:00
            durated(2, 242), // 3:44
            durated(3, 138), // 7:46
            durated(4, 275), // 10:04
        ];
        assert_eq!(cumulative_starts(&songs), [0, 224, 466, 604]);
        assert_eq!(total_runtime(&songs), 879);
    }

    #[test]
    fn a_song_with_no_duration_does_not_advance_the_clock() {
        let songs = vec![durated(1, 100), song(2, "Unknown", "A"), durated(3, 50)];
        assert_eq!(cumulative_starts(&songs), [0, 100, 100]);
        assert_eq!(total_runtime(&songs), 150);
    }

    #[test]
    fn an_empty_set_has_no_clock_and_no_runtime() {
        assert!(cumulative_starts(&[]).is_empty());
        assert_eq!(total_runtime(&[]), 0);
    }

    // ── prep facts ──────────────────────────────────────────────────────

    #[test]
    fn prep_facts_report_one_tuning_a_capo_and_full_coverage() {
        let mut a = with_chart(song(1, "One", "A"), 10);
        a.tuning = Some("Standard".into());
        let mut b = with_chart(song(2, "Two", "B"), 11);
        b.tuning = Some("Standard".into());
        b.capo = Some(2);

        let facts = prep_facts(&[a, b]);
        assert!(facts.contains("Everything is in Standard."), "{facts}");
        assert!(facts.contains("One song wants a capo."), "{facts}");
        assert!(facts.contains("works with no signal"), "{facts}");
    }

    #[test]
    fn prep_facts_list_several_tunings_and_count_missing_charts() {
        let mut a = song(1, "One", "A");
        a.tuning = Some("Open D".into());
        let mut b = song(2, "Two", "B");
        b.tuning = Some("Drop D".into());

        let facts = prep_facts(&[a, b]);
        assert!(facts.contains("You'll need Open D and Drop D."), "{facts}");
        assert!(facts.contains("2 songs have no chart attached."), "{facts}");
    }

    #[test]
    fn prep_facts_say_nothing_about_tunings_when_none_are_set() {
        let facts = prep_facts(&[with_chart(song(1, "One", "A"), 10)]);
        assert!(!facts.contains("tuning"), "{facts}");
        assert!(!facts.contains("capo"), "{facts}");
        assert_eq!(facts, "Every chart is on this phone and works with no signal.");
    }

    /// J4's catch: an empty set is not a set with full chart coverage, and the
    /// function must not say so.
    #[test]
    fn prep_facts_of_an_empty_set_says_nothing_at_all() {
        assert_eq!(prep_facts(&[]), "");
    }

    // ── add to setlist sheet ────────────────────────────────────────────

    fn set(id: u32, name: &str, song_ids: Vec<u32>) -> Setlist {
        Setlist {
            id,
            name: name.into(),
            song_ids,
            last_played: None,
        }
    }

    #[test]
    fn setlist_search_matches_the_name_and_ignores_case() {
        let sets = vec![
            set(1, "Porch, Saturday", vec![]),
            set(2, "Quiet set", vec![]),
        ];
        let names: Vec<String> = filter_setlists(sets.clone(), "quiet")
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, ["Quiet set"]);
        assert_eq!(filter_setlists(sets.clone(), "   ").len(), 2);
        assert!(filter_setlists(sets, "wedding").is_empty());
    }

    #[test]
    fn a_setlist_sub_line_counts_songs_and_adds_up_the_clock() {
        let songs = vec![durated(1, 224), durated(2, 138), song(3, "No clock", "A")];
        assert_eq!(
            setlist_summary(&set(1, "Set", vec![1, 2]), &songs),
            "2 songs · 6:02"
        );
        assert_eq!(
            setlist_summary(&set(2, "Set", vec![1]), &songs),
            "1 song · 3:44"
        );
    }

    #[test]
    fn a_song_with_no_duration_adds_to_the_count_but_not_the_clock() {
        let songs = vec![durated(1, 224), song(2, "No clock", "A")];
        assert_eq!(
            setlist_summary(&set(1, "Set", vec![1, 2]), &songs),
            "2 songs · 3:44"
        );
    }

    #[test]
    fn an_empty_setlist_says_so_rather_than_counting_to_zero() {
        assert_eq!(setlist_summary(&set(1, "Set", vec![]), &[]), "empty");
    }

    #[test]
    fn a_setlist_holding_a_deleted_song_only_counts_what_survives() {
        let songs = vec![durated(1, 224)];
        assert_eq!(
            setlist_summary(&set(1, "Set", vec![1, 99]), &songs),
            "1 song · 3:44"
        );
    }

    // ── performance mode (`1o`) ─────────────────────────────────────────

    /// The counter the wireframe draws, and the fact that it is 1-based while
    /// the index behind it is not.
    #[test]
    fn the_position_counter_counts_from_one() {
        assert_eq!(playing_at(1, 5), Some((1, "2 / 5".to_string())));
        assert_eq!(playing_at(0, 5), Some((0, "1 / 5".to_string())));
        assert_eq!(playing_at(4, 5), Some((4, "5 / 5".to_string())));
        assert_eq!(playing_at(0, 1), Some((0, "1 / 1".to_string())));
    }

    /// An index past the end keeps a chart on the stand instead of blanking
    /// the screen — two songs removed from the set on another screen while it
    /// was playing at song 5 is the way this actually happens — and the label
    /// follows the clamp, so the counter can never disagree with the song
    /// underneath it.
    #[test]
    fn an_index_past_the_end_lands_on_the_last_song_and_says_so() {
        assert_eq!(playing_at(9, 3), Some((2, "3 / 3".to_string())));
        assert_eq!(playing_at(usize::MAX, 2), Some((1, "2 / 2".to_string())));
    }

    /// A set with nothing playable in it has no position, because `0 / 0` is a
    /// counter and a counter promises there is something to count.
    #[test]
    fn an_empty_set_has_no_position_rather_than_a_nought() {
        assert_eq!(playing_at(0, 0), None);
        assert_eq!(playing_at(3, 0), None);
    }

    /// The middle of a set moves both ways; the two ends move one way each.
    /// This is the whole of what F2's chevrons draw, read off a five-song set.
    #[test]
    fn the_chevrons_are_live_in_the_direction_the_set_goes() {
        assert_eq!(steps(0, 5), Steps { back: false, forward: true });
        assert_eq!(steps(2, 5), Steps { back: true, forward: true });
        assert_eq!(steps(4, 5), Steps { back: true, forward: false });
    }

    /// A set of one is at both ends at once, so both chevrons are dim. It is a
    /// real set — the encore somebody plays on its own — and not a degenerate
    /// case to be shrugged at.
    #[test]
    fn a_set_of_one_song_goes_nowhere_in_either_direction() {
        assert_eq!(steps(0, 1), Steps { back: false, forward: false });
    }

    /// A set with nothing playable in it likewise, and without panicking on
    /// the `len - 1` a naive last-song test would reach for.
    #[test]
    fn an_empty_set_offers_no_move_at_all() {
        assert_eq!(steps(0, 0), Steps { back: false, forward: false });
        assert_eq!(steps(7, 0), Steps { back: false, forward: false });
    }

    /// The clamp `playing_at` performs is the one these answers are about. An
    /// index past the end already puts the *last* song on the stand, so ›
    /// must be dim over it: lit, it would offer a move the screen cannot make
    /// and would look exactly like a › that missed the tap.
    #[test]
    fn an_index_past_the_end_is_at_the_end_for_the_chevrons_too() {
        assert_eq!(steps(9, 3), Steps { back: true, forward: false });
        assert_eq!(steps(usize::MAX, 1), Steps { back: false, forward: false });
    }

    /// The line the wireframe draws, from a song that has all three of them.
    #[test]
    fn the_performance_meta_line_is_the_one_the_wireframe_draws() {
        let mut s = song(1, "Angel From Montgomery", "John Prine");
        s.key = Some("G".into());
        s.capo = Some(2);
        s.tempo = Some(96);
        assert_eq!(performance_meta(&s), "G · capo 2 · 96 bpm");
    }

    /// Every part is optional, and dropping one must not leave the separator
    /// it was sitting next to behind. Each of these is a real song in a real
    /// book — a piano part with a key and a tempo and no capo to speak of, a
    /// tune somebody has only ever written the capo down for.
    #[test]
    fn a_missing_part_takes_its_separator_with_it() {
        let mut key_and_tempo = song(1, "A", "B");
        key_and_tempo.key = Some("D".into());
        key_and_tempo.tempo = Some(120);
        assert_eq!(performance_meta(&key_and_tempo), "D · 120 bpm");

        let mut capo_only = song(2, "A", "B");
        capo_only.capo = Some(4);
        assert_eq!(performance_meta(&capo_only), "capo 4");

        let mut key_only = song(3, "A", "B");
        key_only.key = Some("Am".into());
        assert_eq!(performance_meta(&key_only), "Am");
    }

    /// A song with none of the three gets no line at all rather than an empty
    /// one. The top bar centres a title against this string, and a blank row
    /// under it would move the title off centre to say nothing.
    #[test]
    fn a_song_with_nothing_to_say_says_nothing() {
        assert_eq!(performance_meta(&song(1, "Untitled", "Nobody")), "");
    }

    /// The same two "not really set" values `parse_count` refuses on the way
    /// in, refused again on the way out — a capo on the nut is no capo, and
    /// nought beats a minute is no tempo. A row can arrive from a backup or an
    /// older build without having been through the form.
    #[test]
    fn a_nought_capo_or_tempo_is_not_a_fact_about_the_song() {
        let mut s = song(1, "A", "B");
        s.key = Some("E".into());
        s.capo = Some(0);
        s.tempo = Some(0);
        assert_eq!(performance_meta(&s), "E");
    }

    /// A key stored as whitespace is a key nobody set, and it must not put a
    /// leading separator in front of the capo.
    #[test]
    fn a_blank_key_is_no_key() {
        let mut s = song(1, "A", "B");
        s.key = Some("   ".into());
        s.capo = Some(2);
        assert_eq!(performance_meta(&s), "capo 2");
    }

    // ── the bottom bar and the progress strip (F3) ──────────────────────

    /// A three-song set, as it comes off `performance::ordered`. The bottom
    /// bar's own tests and the strip's are both about this list.
    fn a_set() -> Vec<Song> {
        vec![
            song(1, "Carolina", "M. Ward"),
            song(2, "Angel From Montgomery", "John Prine"),
            song(3, "Blackbird", "The Beatles"),
        ]
    }

    /// The line the wireframe draws, from anywhere but the end of the set.
    #[test]
    fn the_bar_names_the_song_that_comes_next() {
        let set = a_set();
        assert_eq!(
            up_next(0, &set),
            Some(UpNext::Song("Angel From Montgomery".into()))
        );
        assert_eq!(up_next(1, &set), Some(UpNext::Song("Blackbird".into())));
        assert_eq!(up_next(0, &set).unwrap().label(), "up next");
        assert_eq!(
            up_next(0, &set).unwrap().title(),
            ["Angel From Montgomery"]
        );
    }

    /// The state the wireframe never drew. The label changes rather than the
    /// title going blank, because `up next` with nothing after it reads as a
    /// title that failed to load — and there is no way to *say* it wrongly,
    /// since the variant with no title carries no title field to leave empty.
    #[test]
    fn the_last_song_says_so_rather_than_labelling_an_empty_slot() {
        let set = a_set();
        assert_eq!(up_next(2, &set), Some(UpNext::Last));
        assert_eq!(up_next(2, &set).unwrap().label(), "last song");
        assert!(up_next(2, &set).unwrap().title().is_empty());
    }

    /// A set of one is at the end of itself from the first bar of it — the
    /// encore somebody plays on its own, and not a degenerate case.
    #[test]
    fn a_set_of_one_song_is_already_on_its_last_song() {
        assert_eq!(
            up_next(0, &[song(1, "Carolina", "M. Ward")]),
            Some(UpNext::Last)
        );
    }

    /// Nothing playable means no line at all, not a line about nothing. The
    /// screen hides the whole bar for this, which is why `None` is a state and
    /// not an empty string.
    #[test]
    fn an_empty_set_has_no_up_next_line() {
        assert_eq!(up_next(0, &[]), None);
        assert_eq!(up_next(4, &[]), None);
    }

    /// The clamp again, and it matters here for the same reason it matters to
    /// the chevrons: the screen draws the *last* song for an out-of-range
    /// index, so the bar has to agree that there is nothing after it.
    #[test]
    fn an_index_past_the_end_is_on_the_last_song_for_the_bar_too() {
        assert_eq!(up_next(9, &a_set()), Some(UpNext::Last));
        assert_eq!(up_next(usize::MAX, &a_set()), Some(UpNext::Last));
    }

    /// One segment per song, and the three states in the places the handoff
    /// puts them: played behind, current here, upcoming ahead.
    #[test]
    fn the_strip_is_one_segment_per_song_in_the_set() {
        use Segment::*;
        assert_eq!(strip_segments(0, 3), [Current, Upcoming, Upcoming]);
        assert_eq!(strip_segments(1, 3), [Played, Current, Upcoming]);
        assert_eq!(strip_segments(2, 3), [Played, Played, Current]);
    }

    /// Not five. `1o` draws five segments because its example set is five songs
    /// long, and a strip that always drew five would be a decoration that lies
    /// about every other set in the book.
    #[test]
    fn a_long_set_gets_a_long_strip_rather_than_the_wireframes_five() {
        assert_eq!(strip_segments(7, 22).len(), 22);
        assert_eq!(strip_segments(7, 22)[7], Segment::Current);
        assert_eq!(strip_segments(7, 22)[6], Segment::Played);
        assert_eq!(strip_segments(7, 22)[8], Segment::Upcoming);
        assert_eq!(strip_segments(0, 1), [Segment::Current]);
    }

    /// A set with nothing playable in it draws no strip, rather than one
    /// segment's worth of nothing.
    #[test]
    fn an_empty_set_draws_no_strip_at_all() {
        assert!(strip_segments(0, 0).is_empty());
        assert!(strip_segments(5, 0).is_empty());
    }

    /// An index past the end still marks the song that is actually up. Without
    /// the clamp every segment would come out `Played` and the strip would say
    /// the gig had finished while its last chart was still on the stand.
    #[test]
    fn an_index_past_the_end_still_marks_the_song_on_the_stand() {
        use Segment::*;
        assert_eq!(strip_segments(9, 3), [Played, Played, Current]);
    }

    /// The gap closes as the set grows, and is allowed to reach nought: past
    /// roughly fifty songs the strip stops being a row of segments and becomes
    /// one bar whose colour changes where the set has got to, which is what a
    /// progress bar for that many songs should look like anyway.
    #[test]
    fn the_gap_between_segments_closes_as_the_set_grows() {
        assert_eq!(strip_gap(5), 3);
        assert_eq!(strip_gap(12), 3);
        assert_eq!(strip_gap(13), 2);
        assert_eq!(strip_gap(24), 2);
        assert_eq!(strip_gap(25), 1);
        assert_eq!(strip_gap(48), 1);
        assert_eq!(strip_gap(49), 0);
        assert_eq!(strip_gap(200), 0);
    }

    /// The arithmetic the thresholds were chosen for, checked rather than
    /// asserted in prose: on the 365px the strip has to live in, no set ever
    /// spends more than about a third of the strip on the spaces between its
    /// segments, and every segment stays at least a couple of pixels wide.
    #[test]
    fn the_strip_never_becomes_more_gap_than_segment() {
        const WIDTH: f32 = 365.0;
        for len in 1..=60usize {
            let gaps = (len.saturating_sub(1)) as f32 * strip_gap(len) as f32;
            assert!(
                gaps < WIDTH * 0.35,
                "a set of {len} spends {gaps}px of {WIDTH}px on gaps"
            );
            let segment = (WIDTH - gaps) / len as f32;
            assert!(
                segment >= 2.0,
                "a set of {len} leaves {segment}px per segment"
            );
        }
    }

    // ── sort & group sheet ──────────────────────────────────────────────

    fn capoed(id: u32, capo: u8) -> Song {
        let mut s = song(id, &format!("Song {id}"), "A");
        s.capo = Some(capo);
        s
    }

    #[test]
    fn a_field_almost_nobody_fills_in_is_sparse() {
        // One song in five has a capo — under the one-in-four line.
        let mut songs: Vec<Song> = (1..=4).map(|i| song(i, "Plain", "A")).collect();
        songs.push(capoed(5, 2));
        assert_eq!(field_fill_count(&songs, SortField::Capo), 1);
        assert!(is_sparse_field(&songs, SortField::Capo));
    }

    #[test]
    fn a_field_a_quarter_of_the_book_fills_in_is_not_sparse() {
        let mut songs: Vec<Song> = (1..=3).map(|i| song(i, "Plain", "A")).collect();
        songs.push(capoed(4, 2));
        assert!(!is_sparse_field(&songs, SortField::Capo));
    }

    #[test]
    fn fields_every_song_has_are_never_sparse() {
        let songs: Vec<Song> = (1..=40).map(|i| song(i, "Plain", "A")).collect();
        for field in [SortField::Artist, SortField::Title, SortField::DateAdded] {
            assert_eq!(field_fill_count(&songs, field), 40);
            assert!(!is_sparse_field(&songs, field));
        }
    }

    #[test]
    fn an_empty_library_greys_nothing() {
        assert!(!is_sparse_field(&[], SortField::Capo));
    }

    #[test]
    fn the_arrow_follows_the_words_beside_it_not_the_comparator() {
        // "A to Z" reads up the alphabet; "newest first" reads down the calendar.
        assert_eq!(direction_arrow(SortField::Artist, SortDir::Asc), TablerIcon::ArrowNarrowUp);
        assert_eq!(direction_arrow(SortField::Artist, SortDir::Desc), TablerIcon::ArrowNarrowDown);
        assert_eq!(direction_arrow(SortField::LastPlayed, SortDir::Asc), TablerIcon::ArrowNarrowDown);
        assert_eq!(direction_arrow(SortField::LastPlayed, SortDir::Desc), TablerIcon::ArrowNarrowUp);
        assert_eq!(direction_arrow(SortField::DateAdded, SortDir::Asc), TablerIcon::ArrowNarrowDown);
        assert_eq!(direction_arrow(SortField::Tempo, SortDir::Asc), TablerIcon::ArrowNarrowUp);
    }

    #[test]
    fn every_sort_field_has_words_and_an_arrow_in_both_directions() {
        for field in SortField::ALL {
            for dir in [SortDir::Asc, SortDir::Desc] {
                assert!(!field.direction_label(dir).is_empty(), "{field:?}");
                assert!(
                    [TablerIcon::ArrowNarrowUp, TablerIcon::ArrowNarrowDown]
                        .contains(&direction_arrow(field, dir)),
                    "{field:?}"
                );
            }
            // Reversing has to actually change what the row says.
            assert_ne!(
                field.direction_label(SortDir::Asc),
                field.direction_label(SortDir::Desc),
                "{field:?}"
            );
            assert_ne!(
                direction_arrow(field, SortDir::Asc),
                direction_arrow(field, SortDir::Desc),
                "{field:?}"
            );
        }
    }

    #[test]
    fn the_grey_note_counts_in_words_the_row_has_room_for() {
        assert_eq!(fill_note(0), "0 songs have this");
        assert_eq!(fill_note(1), "1 song has this");
        assert_eq!(fill_note(3), "3 songs have this");
    }

    // ── add / edit song ─────────────────────────────────────────────────

    fn by(id: u32, artist: &str) -> Song {
        song(id, &format!("Song {id}"), artist)
    }

    fn names(suggestions: &[ArtistSuggestion]) -> Vec<String> {
        suggestions.iter().map(|s| s.name.clone()).collect()
    }

    #[test]
    fn artists_are_derived_from_the_songs_that_credit_them() {
        let songs = vec![
            by(1, "Led Zeppelin"),
            by(2, "Joni Mitchell"),
            by(3, "Led Zeppelin"),
        ];
        let known = known_artists(&songs);
        assert_eq!(names(&known), ["Joni Mitchell", "Led Zeppelin"]);
        assert_eq!(known[1].songs, 2);
    }

    #[test]
    fn one_artist_spelled_two_ways_is_still_one_artist() {
        // Three songs, one artist, and the majority spelling is what gets
        // offered back — not the first one typed.
        let songs = vec![by(1, "the beatles"), by(2, "The Beatles"), by(3, "The Beatles")];
        let known = known_artists(&songs);
        assert_eq!(names(&known), ["The Beatles"]);
        assert_eq!(known[0].songs, 3);
    }

    #[test]
    fn a_tie_between_spellings_resolves_the_same_way_whatever_the_order() {
        let one = vec![by(1, "the beatles"), by(2, "The Beatles")];
        let other = vec![by(1, "The Beatles"), by(2, "the beatles")];
        assert_eq!(known_artists(&one), known_artists(&other));
    }

    #[test]
    fn a_song_with_no_artist_credits_nobody() {
        let songs = vec![by(1, ""), by(2, "   "), by(3, "Nick Drake")];
        assert_eq!(names(&known_artists(&songs)), ["Nick Drake"]);
    }

    #[test]
    fn suggestions_match_case_insensitively() {
        let songs = vec![by(1, "Led Zeppelin"), by(2, "Joni Mitchell")];
        assert_eq!(names(&artist_suggestions(&songs, "led zep")), ["Led Zeppelin"]);
        assert_eq!(names(&artist_suggestions(&songs, "LED")), ["Led Zeppelin"]);
        assert_eq!(names(&artist_suggestions(&songs, "  joni ")), ["Joni Mitchell"]);
    }

    #[test]
    fn a_name_that_starts_with_the_typing_beats_one_that_merely_contains_it() {
        // Remembering the surname first is what the second kind of match is
        // for; it should not outrank the artist whose name opens that way.
        let songs = vec![by(1, "Neil Young"), by(2, "Young Marble Giants")];
        assert_eq!(
            names(&artist_suggestions(&songs, "young")),
            ["Young Marble Giants", "Neil Young"]
        );
    }

    #[test]
    fn an_untouched_artist_field_asks_nothing() {
        let songs = vec![by(1, "Led Zeppelin")];
        assert!(artist_suggestions(&songs, "").is_empty());
        assert!(artist_suggestions(&songs, "   ").is_empty());
    }

    #[test]
    fn a_name_typed_out_in_full_stops_being_a_question() {
        // The list is there to save typing. Once the typing is done — in any
        // case — it has nothing left to offer and gets out of the way.
        let songs = vec![by(1, "Led Zeppelin")];
        assert!(artist_suggestions(&songs, "Led Zeppelin").is_empty());
        assert!(artist_suggestions(&songs, "led zeppelin").is_empty());
        assert!(!artist_suggestions(&songs, "Led Zeppeli").is_empty());
    }

    #[test]
    fn an_artist_nobody_has_played_yet_matches_nothing() {
        let songs = vec![by(1, "Led Zeppelin")];
        assert!(artist_suggestions(&songs, "Sibylle Baier").is_empty());
    }

    #[test]
    fn the_suggestion_list_stays_short_enough_to_read() {
        let songs: Vec<Song> = (1..=9).map(|i| by(i, &format!("Artist {i}"))).collect();
        assert_eq!(
            artist_suggestions(&songs, "artist").len(),
            ARTIST_SUGGESTION_LIMIT
        );
    }

    #[test]
    fn an_empty_library_suggests_nothing_and_does_not_panic() {
        assert!(known_artists(&[]).is_empty());
        assert!(artist_suggestions(&[], "anyone").is_empty());
    }

    #[test]
    fn the_suggestion_count_reads_as_words() {
        assert_eq!(artist_count_note(1), "· 1 song");
        assert_eq!(artist_count_note(12), "· 12 songs");
    }

    #[test]
    fn an_optional_number_is_unset_unless_it_is_a_real_one() {
        assert_eq!(parse_count("96"), Some(96));
        assert_eq!(parse_count("  3  "), Some(3));
        assert_eq!(parse_count(""), None);
        assert_eq!(parse_count("   "), None);
        // A capo on the nut is no capo; nought beats a minute is no tempo.
        assert_eq!(parse_count("0"), None);
        assert_eq!(parse_count("fast"), None);
        assert_eq!(parse_count("-4"), None);
        assert_eq!(parse_count("3.5"), None);
    }

    #[test]
    fn a_duration_round_trips_through_the_form_it_is_shown_in() {
        for seconds in [1u32, 59, 60, 224, 3599, 3600] {
            assert_eq!(parse_duration(&fmt_duration(seconds)), Some(seconds));
        }
    }

    #[test]
    fn a_bare_number_in_the_duration_field_is_minutes() {
        assert_eq!(parse_duration("4"), Some(240));
        assert_eq!(parse_duration(" 4 "), Some(240));
    }

    #[test]
    fn a_duration_that_is_not_a_clock_reading_stays_unset() {
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("3:4"), None);
        assert_eq!(parse_duration("3:444"), None);
        assert_eq!(parse_duration("3:60"), None);
        assert_eq!(parse_duration("3:ab"), None);
        assert_eq!(parse_duration("about four minutes"), None);
        assert_eq!(parse_duration("0:00"), None);
    }

    #[test]
    fn a_duration_under_a_minute_keeps_its_zero_minutes() {
        assert_eq!(parse_duration("0:45"), Some(45));
    }

    #[test]
    fn tags_are_one_line_of_commas_and_survive_a_round_trip() {
        let tags = parse_tags("campfire, open mic , wedding");
        assert_eq!(tags, ["campfire", "open mic", "wedding"]);
        assert_eq!(format_tags(&tags), "campfire, open mic, wedding");
        assert_eq!(parse_tags(&format_tags(&tags)), tags);
    }

    #[test]
    fn empty_tags_and_stray_commas_are_dropped() {
        assert!(parse_tags("").is_empty());
        assert!(parse_tags(" , ,, ").is_empty());
        assert_eq!(parse_tags("campfire,,"), ["campfire"]);
    }

    #[test]
    fn one_tag_typed_twice_stays_one_tag_in_the_spelling_that_came_first() {
        // Two spellings would split a group header in two on the library
        // screen; the first one typed is the one kept.
        assert_eq!(parse_tags("Campfire, campfire"), ["Campfire"]);
        assert_eq!(parse_tags("campfire, Campfire"), ["campfire"]);
    }

    // ── setlist song picker (`1i`) ──────────────────────────────────────

    /// A fixed "today" so the window under test is the rule's, not the
    /// clock's.
    const TODAY: Day = Day::new(2026, 8, 26);

    /// A song last played `days` before `TODAY`, and added at the epoch — so
    /// only last-played can make it recent.
    fn played_days_ago(id: u32, days: i64) -> Song {
        let mut s = song(id, &format!("Song {id}"), "A");
        s.last_played = Some(Day::from_days_since_epoch(TODAY.days_since_epoch() - days));
        s.created_at = 0;
        s
    }

    fn tagged(id: u32, tags: &[&str]) -> Song {
        let mut s = song(id, &format!("Song {id}"), "A");
        s.tags = tags.iter().map(|t| (*t).to_string()).collect();
        s
    }

    #[test]
    fn the_all_chip_keeps_every_song() {
        let songs = vec![song(1, "One", "A"), rated(2, "Two", "B", Confidence::Rusty)];
        let kept = picker_songs(songs, "", SongFilter::All, None, TODAY);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn the_solid_chip_keeps_only_songs_marked_solid() {
        let songs = vec![
            rated(1, "Solid one", "A", Confidence::Solid),
            rated(2, "Rusty one", "B", Confidence::Rusty),
            rated(3, "Learning one", "C", Confidence::Learning),
            song(4, "Unrated one", "D"),
        ];
        assert_eq!(
            titles(&picker_songs(songs, "", SongFilter::Solid, None, TODAY)),
            ["Solid one"]
        );
    }

    #[test]
    fn the_recent_chip_reaches_exactly_ninety_days_back() {
        // The boundary is inclusive: a song played on the ninetieth day back
        // is still recent, the ninety-first is not.
        assert!(matches_filter(
            &played_days_ago(1, RECENT_DAYS),
            SongFilter::Recent,
            None,
            TODAY
        ));
        assert!(!matches_filter(
            &played_days_ago(2, RECENT_DAYS + 1),
            SongFilter::Recent,
            None,
            TODAY
        ));
        assert!(matches_filter(
            &played_days_ago(3, 0),
            SongFilter::Recent,
            None,
            TODAY
        ));
    }

    #[test]
    fn a_song_added_recently_is_recent_even_if_it_has_never_been_played() {
        let mut fresh = song(1, "Typed in this morning", "A");
        fresh.last_played = None;
        fresh.created_at = (TODAY.days_since_epoch() as u64) * 86_400 * 1000;
        assert!(matches_filter(&fresh, SongFilter::Recent, None, TODAY));

        // ...and an old song nobody has played is not.
        let stale = song(2, "Filed years ago", "A");
        assert_eq!(stale.created_at, 2); // `Song::new` seeds it from the id
        assert!(!matches_filter(&stale, SongFilter::Recent, None, TODAY));
    }

    #[test]
    fn a_never_played_song_is_not_recent_by_default() {
        let songs = vec![song(1, "Never played", "A")];
        assert!(picker_songs(songs, "", SongFilter::Recent, None, TODAY).is_empty());
    }

    #[test]
    fn the_tag_chip_with_no_tag_chosen_keeps_every_tagged_song() {
        let songs = vec![
            tagged(1, &["campfire"]),
            tagged(2, &["fingerstyle", "crowd"]),
            song(3, "Untagged", "A"),
        ];
        assert_eq!(
            titles(&picker_songs(songs, "", SongFilter::Tag, None, TODAY)),
            ["Song 1", "Song 2"]
        );
    }

    #[test]
    fn a_chosen_tag_narrows_to_it_whatever_its_case() {
        let songs = vec![tagged(1, &["Campfire"]), tagged(2, &["crowd"])];
        assert_eq!(
            titles(&picker_songs(
                songs.clone(),
                "",
                SongFilter::Tag,
                Some("campfire"),
                TODAY
            )),
            ["Song 1"]
        );
        assert!(
            picker_songs(songs, "", SongFilter::Tag, Some("wedding"), TODAY).is_empty()
        );
    }

    #[test]
    fn the_search_field_and_the_chip_both_apply() {
        let mut solid = rated(1, "Blackbird", "The Beatles", Confidence::Solid);
        solid.tags = vec!["campfire".into()];
        let songs = vec![
            solid,
            rated(2, "Blowin' In The Wind", "Bob Dylan", Confidence::Solid),
            rated(3, "Blackwater", "Doobie Brothers", Confidence::Rusty),
        ];
        assert_eq!(
            titles(&picker_songs(songs, "black", SongFilter::Solid, None, TODAY)),
            ["Blackbird"]
        );
    }

    #[test]
    fn the_picker_list_is_alphabetical_whatever_order_the_book_is_in() {
        let songs = vec![
            song(1, "Cortez The Killer", "Neil Young"),
            song(2, "Atlantic City", "Springsteen"),
            song(3, "big yellow taxi", "Joni Mitchell"),
        ];
        assert_eq!(
            titles(&picker_songs(songs, "", SongFilter::All, None, TODAY)),
            ["Atlantic City", "big yellow taxi", "Cortez The Killer"]
        );
    }

    #[test]
    fn the_tag_row_lists_every_tag_once_in_the_spelling_that_came_first() {
        let songs = vec![
            tagged(1, &["Campfire", "wedding"]),
            tagged(2, &["campfire"]),
            tagged(3, &["crowd"]),
        ];
        assert_eq!(library_tags(&songs), ["Campfire", "crowd", "wedding"]);
    }

    #[test]
    fn a_book_with_no_tags_offers_no_tag_row() {
        assert!(library_tags(&[song(1, "One", "A")]).is_empty());
    }

    #[test]
    fn the_running_count_says_nothing_until_something_is_picked() {
        assert_eq!(picked_note(0), "");
        assert_eq!(picked_note(1), "1 picked");
        assert_eq!(picked_note(2), "2 picked");
        assert_eq!(picked_note(11), "11 picked");
    }

    #[test]
    fn the_add_button_counts_and_pluralises_what_it_will_add() {
        assert_eq!(add_songs_label(0), "Add songs");
        assert_eq!(add_songs_label(1), "Add 1 song");
        assert_eq!(add_songs_label(2), "Add 2 songs");
        assert_eq!(add_songs_label(12), "Add 12 songs");
    }

    #[test]
    fn an_empty_list_explains_itself_in_the_terms_the_user_just_used() {
        // A typed query wins over the chip: it is the thing just done.
        assert_eq!(
            picker_empty_note(SongFilter::Solid, None, "  blackbird "),
            "Nothing in your book matches \u{201c}blackbird\u{201d}."
        );
        assert_eq!(
            picker_empty_note(SongFilter::Recent, None, ""),
            format!("Nothing played or added in the last {RECENT_DAYS} days.")
        );
        assert_eq!(
            picker_empty_note(SongFilter::Tag, Some("wedding"), ""),
            "Nothing is tagged \u{201c}wedding\u{201d}."
        );
        assert_eq!(
            picker_empty_note(SongFilter::Tag, None, ""),
            "No song carries a tag yet."
        );
        assert_eq!(
            picker_empty_note(SongFilter::All, None, ""),
            "Your book has no songs in it yet."
        );
    }

    // ── Settings (`1q`) ─────────────────────────────────────────────────

    #[test]
    fn an_empty_library_reports_nothing_rather_than_zero_bytes() {
        assert_eq!(attachments_note(0), "Nothing yet");
        assert_eq!(saved_pages_note(0), "None yet");
    }

    #[test]
    fn the_storage_rows_report_what_is_there() {
        // The wireframe's own two numbers, so the row it drew is the row this
        // produces: `248 MB` and `37`.
        assert_eq!(attachments_note(248_000_000), "248 MB");
        assert_eq!(saved_pages_note(37), "37");
        // A file that exists never rounds down to nothing.
        assert_eq!(attachments_note(1), "1 B");
    }

    #[test]
    fn a_switch_row_says_on_or_off_in_sentence_case() {
        assert_eq!(on_off(true), "On");
        assert_eq!(on_off(false), "Off");
    }

    #[test]
    fn the_library_sort_row_speaks_the_sort_sheets_own_words() {
        assert_eq!(
            library_sort_note(SortField::Artist, SortDir::Asc),
            "Artist \u{b7} A to Z"
        );
        // A date field ascends into "newest first", which is the sheet's
        // wording and not a direction arrow reinterpreted here.
        assert_eq!(
            library_sort_note(SortField::LastPlayed, SortDir::Asc),
            "Last played \u{b7} newest first"
        );
        assert_eq!(
            library_sort_note(SortField::Tempo, SortDir::Desc),
            "Tempo \u{b7} fast to slow"
        );
    }

    #[test]
    fn the_choice_rows_label_every_value_they_can_hold() {
        assert_eq!(density_label(Density::Comfortable), "Comfortable");
        assert_eq!(density_label(Density::Compact), "Compact");
        assert_eq!(
            performance_theme_label(PerformanceTheme::FollowApp),
            "Follow app"
        );
        assert_eq!(
            performance_theme_label(PerformanceTheme::AlwaysDark),
            "Always dark"
        );
    }

    #[test]
    fn a_display_label_is_not_the_name_a_value_is_stored_under() {
        // The two are equal for `Density` today and must be allowed to drift:
        // the stored name is a schema value and the label is copy. This test
        // exists so that rewording one does not silently reword the other.
        for density in [Density::Comfortable, Density::Compact] {
            assert!(!density_label(density).is_empty());
            assert!(!density.name().is_empty());
        }
    }

    // ── I3: what the Backup section says about the last export ──────────

    /// A library nobody has exported says so, and goes on saying exactly that
    /// — this is the assertion that keeps "mention once, nag never" true,
    /// because the way that rule gets broken is by someone later making the
    /// wording depend on how long it has been.
    #[test]
    fn a_library_that_has_never_been_exported_says_so_plainly() {
        assert_eq!(last_export_note(None, Day::new(2026, 9, 2)), "Not backed up yet");
    }

    #[test]
    fn an_export_earlier_the_same_day_reads_as_today() {
        let today = Day::new(2026, 9, 2);
        let ms = today.days_since_epoch() * 86_400_000 + 13 * 3_600_000;
        assert_eq!(last_export_note(Some(ms), today), "Exported today");
    }

    #[test]
    fn an_older_export_is_named_by_its_day_and_not_by_its_age() {
        let day = Day::new(2026, 6, 2);
        let ms = day.days_since_epoch() * 86_400_000;
        assert_eq!(last_export_note(Some(ms), Day::new(2026, 9, 2)), "Exported Jun 2");
    }

    /// Whatever the gap, the sentence keeps its shape: no "3 months ago", no
    /// warning, nothing that grows more insistent. A year later it still just
    /// names the day.
    #[test]
    fn a_very_old_export_says_the_same_kind_of_thing_as_a_recent_one() {
        let old = Day::new(2024, 1, 9);
        let ms = old.days_since_epoch() * 86_400_000;
        let note = last_export_note(Some(ms), Day::new(2026, 9, 2));
        assert_eq!(note, "Exported Jan 9");
        assert!(!note.contains("ago"), "{note}");
    }

    #[test]
    fn the_accent_row_names_the_colour_actually_on_screen() {
        let palette = Some(Rgb::new(0x6D, 0x5E, 0x8C));
        let wallpaper = Some(Rgb::new(0x3F, 0x51, 0xB5));

        assert_eq!(accent_note(AccentChoice::Named(1), None, None), "Pine");
        // `FromSystem` on a device that offers neither colour — every desktop
        // build, and any handset below API 31 whose wallpaper publishes
        // nothing — resolves to Rust, so that is what the row says. It says it
        // for the same reason it always did: the screen is rust-coloured.
        assert_eq!(accent_note(AccentChoice::FromSystem, None, None), "Rust");
        // And when there *is* a device colour, the row stops saying the name
        // of an accent nobody picked and says where the colour came from. This
        // is the assertion card K8 changed.
        assert_eq!(accent_note(AccentChoice::FromSystem, None, wallpaper), "Wallpaper");
        // **And this is the one K57 changed.** The row is the only place the
        // difference between the two device colours is ever visible, so a note
        // that went on saying "Wallpaper" for a colour that came out of the
        // system palette would be the row lying about the one thing it is for
        // — on a device themed from a *preset*, about a wallpaper colour the
        // user had explicitly overridden.
        assert_eq!(accent_note(AccentChoice::FromSystem, palette, wallpaper), "System");
        assert_eq!(
            accent_note(AccentChoice::FromSystem, palette, None),
            "System",
            "with no wallpaper to fall back to, the palette still names itself"
        );
        // A named pick ignores both, so the note does too.
        assert_eq!(accent_note(AccentChoice::Named(1), palette, wallpaper), "Pine");
        assert_eq!(accent_note(AccentChoice::Named(99), None, None), "Plum");
    }

    /// Card K8's resolution table, in full: three choices crossed with the
    /// three things the platform can say. The two rows that matter most are
    /// the ones an explicit choice is on — somebody who has said "Dark" has
    /// said it about this app, and a sunset must not move it.
    #[test]
    fn the_theme_resolution_table_is_exactly_these_nine_answers() {
        for night in [Some(true), Some(false), None] {
            assert!(
                !dark_active(ThemeChoice::Light, night),
                "explicit Light followed the system ({night:?})"
            );
            assert!(
                dark_active(ThemeChoice::Dark, night),
                "explicit Dark followed the system ({night:?})"
            );
        }

        assert!(dark_active(ThemeChoice::FollowSystem, Some(true)), "system night");
        assert!(!dark_active(ThemeChoice::FollowSystem, Some(false)), "system day");
    }

    /// The third state, on its own, because it is the one that has no obvious
    /// answer and so is the one somebody will change without meaning to.
    /// `None` is the platform saying it has no opinion — an undefined
    /// `uiMode`, a JNI failure, or any desktop build — and this app's answer
    /// is light, because light is what every untouched install looked like
    /// before this card.
    #[test]
    fn a_platform_with_no_opinion_leaves_follow_system_on_the_light_theme() {
        assert!(!dark_active(ThemeChoice::FollowSystem, None));
    }

    #[test]
    fn forcing_performance_mode_dark_darkens_the_chrome_and_nothing_else() {
        let gig = Route::Performance(1);
        assert!(!dark_chrome(gig, PerformanceTheme::FollowApp));
        assert!(dark_chrome(gig, PerformanceTheme::AlwaysDark));

        // The viewer is dark either way — that half is the route's, and the
        // setting must not be able to lighten it.
        let viewer = Route::ViewAttachment {
            song: 1,
            attachment: 1,
        };
        assert!(dark_chrome(viewer, PerformanceTheme::FollowApp));
        assert!(dark_chrome(viewer, PerformanceTheme::AlwaysDark));

        // And no ordinary screen is darkened by a setting about performance
        // mode, which is the mistake the whole two-argument shape prevents.
        for route in [
            Route::Library,
            Route::Setlists,
            Route::Settings,
            Route::SongDetail(1),
            Route::SetlistDetail(1),
            Route::AddSong,
        ] {
            assert!(!dark_chrome(route, PerformanceTheme::AlwaysDark));
        }
    }

    // ── J7: a route outliving its subject ──────────────────────────────

    /// Every route that names a song, checked against a song that is there
    /// and one that is not. Written as one table rather than one test per
    /// variant so that a new song-carrying variant added to the match in
    /// `route_orphaned` without a line added here is a glaring gap instead of
    /// a silent one.
    #[test]
    fn a_route_naming_a_song_is_orphaned_only_when_the_song_is_gone() {
        let has_song = |id: SongId| id == 1;
        let no_song = |_: SongId| false;
        let has_setlist = |_: SetlistId| true;

        for route in [
            Route::SongDetail(1),
            Route::EditSong(1),
            Route::TypeChart { song: 1, chart: None },
            Route::TypeChart { song: 1, chart: Some(9) },
            Route::CaptureWebpage { song: 1 },
            Route::ViewAttachment { song: 1, attachment: 9 },
        ] {
            assert!(
                !route_orphaned(route, has_song, has_setlist),
                "{route:?} must not be orphaned while its song is there"
            );
            assert!(
                route_orphaned(route, no_song, has_setlist),
                "{route:?} must be orphaned once its song is gone"
            );
        }
    }

    /// The setlist-carrying half of the same rule, including performance
    /// mode: `Performance(id)` is a screen for a setlist exactly as much as
    /// `SetlistDetail(id)` is, and a set deleted underneath a gig in progress
    /// has to leave this screen too.
    #[test]
    fn a_route_naming_a_setlist_is_orphaned_only_when_the_setlist_is_gone() {
        let has_setlist = |id: SetlistId| id == 1;
        let no_setlist = |_: SetlistId| false;
        let has_song = |_: SongId| true;

        for route in [Route::SetlistDetail(1), Route::Performance(1)] {
            assert!(
                !route_orphaned(route, has_song, has_setlist),
                "{route:?} must not be orphaned while its setlist is there"
            );
            assert!(
                route_orphaned(route, has_song, no_setlist),
                "{route:?} must be orphaned once its setlist is gone"
            );
        }
    }

    /// Every subject-less route, whatever the closures say. `AddSong` is the
    /// trap named in the card: there is no song yet, so it must never be
    /// caught by this rule no matter how the closures answer.
    #[test]
    fn a_route_with_no_subject_is_never_orphaned() {
        let always_gone_song = |_: SongId| false;
        let always_gone_setlist = |_: SetlistId| false;

        for route in [
            Route::Library,
            Route::Setlists,
            Route::Settings,
            Route::AddSong,
            Route::Search,
            Route::SaveShared,
        ] {
            assert!(!route_orphaned(route, always_gone_song, always_gone_setlist));
        }
    }

    // ── H3: the empty book shows one screen, not a flag ────────────────

    #[test]
    fn an_empty_book_shows_first_run_whether_fresh_or_freshly_emptied() {
        // Zero is zero, whatever emptied it — a fresh install and a library
        // that just lost its last song ask the same question.
        assert!(first_run_active(0));
    }

    #[test]
    fn a_book_with_even_one_song_does_not_show_first_run() {
        assert!(!first_run_active(1));
        assert!(!first_run_active(300));
    }

    // ── J4: a library that never opened says so ─────────────────────────

    #[test]
    fn a_persistent_library_shows_no_banner_at_all() {
        assert_eq!(library_unavailable_note(true), None);
    }

    #[test]
    fn a_library_that_never_opened_says_so_and_says_nothing_will_be_kept() {
        let sentence = library_unavailable_note(false).expect("a sentence, not nothing");
        assert_eq!(sentence, LIBRARY_UNAVAILABLE);
        assert!(sentence.contains("could not be opened"));
        assert!(sentence.contains("will be saved"));
        // House style: plain sentences, no exclamation marks.
        assert!(!sentence.contains('!'));
    }

    // ── J2: measuring the library list before virtualising ──────────────
    //
    // The card's premise — "every row is currently built up front" — is
    // false for the default rendering (`GROUP_PREVIEW` truncates every
    // group to 6 until the user asks for more), but the card was right to
    // ask for numbers rather than trust that observation on its own. Two
    // things needed a real measurement: how many rows the worst realistic
    // configuration actually renders, and what `library.rs` calling
    // `view.grouped(...)` from several places per frame — the main `for`,
    // the alphabet rail, and once *more* inside `songs_in_group` for every
    // mounted group, not the "four" the card guessed — actually costs.
    //
    // `library_of` below is one fixture shared by the row-count test and
    // the two timing tests, built so every `GroupBy` variant lands on more
    // than one group and every group clears `GROUP_PREVIEW`: 26 first
    // letters (~11-12 songs each), 40 distinct artists (~7-8 each), 4
    // confidences (75 each), 10 tags (30 each), 5 tunings (60 each) — all
    // over the card's stated 300-song target.

    fn library_of(n: u32) -> Vec<Song> {
        (0..n)
            .map(|i| {
                let letter = (b'A' + (i % 26) as u8) as char;
                let mut s = song(i, &format!("{letter} Song {i}"), &format!("Artist {}", i % 40));
                s.confidence = Some(match i % 4 {
                    0 => Confidence::Solid,
                    1 => Confidence::Rusty,
                    2 => Confidence::Learning,
                    _ => Confidence::Rusty, // "Unrated" needs `None`, handled below
                });
                if i % 4 == 3 {
                    s.confidence = None;
                }
                s.tags = vec![format!("tag{}", i % 10)];
                s.tuning = Some(format!("Tuning {}", i % 5));
                s
            })
            .collect()
    }

    /// What actually paints under `GROUP_PREVIEW` truncation: every group
    /// counts for `min(GROUP_PREVIEW, its total)` unless `expanded_label`
    /// names it, in which case it counts in full — mirroring exactly what
    /// `library.rs`'s `for` loop and its "Show N more" row put on screen.
    fn rendered_row_count(groups: &[Group], expanded_label: Option<&str>) -> usize {
        groups
            .iter()
            .map(|g| {
                if Some(g.label.as_str()) == expanded_label {
                    g.songs.len()
                } else {
                    g.songs.len().min(GROUP_PREVIEW)
                }
            })
            .sum()
    }

    #[test]
    fn a_three_hundred_song_library_renders_far_fewer_than_three_hundred_rows_by_default() {
        let songs = library_of(300);
        let filters = Filters::default();

        let confidence = grouped(songs.clone(), GroupBy::Confidence, SortField::Title, SortDir::Asc, &filters);
        assert_eq!(confidence.len(), 4, "Solid, Rusty, Learning, Unrated");
        assert_eq!(rendered_row_count(&confidence, None), 24, "4 groups × GROUP_PREVIEW");

        // First letter is the card's own "interesting one" — up to 26
        // groups. It lands on exactly 26 here, each comfortably over the
        // preview limit, so the default render is 26 × 6.
        let letters = grouped(songs.clone(), GroupBy::FirstLetter, SortField::Title, SortDir::Asc, &filters);
        assert_eq!(letters.len(), 26, "one group per letter of the alphabet");
        assert_eq!(rendered_row_count(&letters, None), 156, "26 groups × GROUP_PREVIEW — the card's own estimate");

        // Artist is not named in the card, and it is worse: 40 distinct
        // artists beats 26 letters, so a library grouped by artist with
        // many one-off songwriters renders *more* default rows than the
        // "interesting" first-letter case the card called out.
        let artists = grouped(songs.clone(), GroupBy::Artist, SortField::Title, SortDir::Asc, &filters);
        assert_eq!(artists.len(), 40);
        assert_eq!(rendered_row_count(&artists, None), 240, "40 groups × GROUP_PREVIEW — worse than first-letter");

        // A single expanded group, first-letter grouping: the other case
        // the card asked about. "A" holds 12 songs (300 songs, i % 26).
        assert_eq!(
            rendered_row_count(&letters, Some("A")),
            156 - GROUP_PREVIEW + 12,
            "one expanded group adds its members past the preview, the rest stay truncated"
        );

        // The true worst case: `GroupBy::None` is one group holding the
        // whole book, and expanding it renders every song — this is where
        // "up to 300" in the card actually lives, not in first-letter.
        let all = grouped(songs.clone(), GroupBy::None, SortField::Title, SortDir::Asc, &filters);
        assert_eq!(all.len(), 1);
        assert_eq!(rendered_row_count(&all, Some("All songs")), 300);

        eprintln!(
            "row counts at 300 songs — Confidence: {} groups/{} rows, \
             FirstLetter: {} groups/{} rows (one expanded: {} rows), \
             Artist: {} groups/{} rows, \
             GroupBy::None fully expanded: 300 rows",
            confidence.len(),
            rendered_row_count(&confidence, None),
            letters.len(),
            rendered_row_count(&letters, None),
            rendered_row_count(&letters, Some("A")),
            artists.len(),
            rendered_row_count(&artists, None),
        );
    }

    /// The cost of one `derive::grouped` call — filter, bucket and sort —
    /// over the stated 300-song target, for every `GroupBy` variant. There
    /// is no `GroupBy::ALL` constant in this codebase (the card assumed
    /// one); this enumerates the six variants by hand so a seventh doesn't
    /// silently go unmeasured.
    ///
    /// The bound is generous for the same reason G2's search timing test's
    /// is: CI boxes are noisy, and a linear pass over 300 small structs is
    /// sub-millisecond work on any machine this runs on. It exists to catch
    /// a quadratic regression, not to certify a performance target.
    #[test]
    fn grouping_a_three_hundred_song_library_stays_fast_however_you_bucket_it() {
        use std::time::Instant;

        let songs = library_of(300);
        let filters = Filters::default();
        let variants = [
            GroupBy::Confidence,
            GroupBy::FirstLetter,
            GroupBy::Artist,
            GroupBy::Tag,
            GroupBy::Tuning,
            GroupBy::None,
        ];

        for group_by in variants {
            let start = Instant::now();
            let groups = grouped(songs.clone(), group_by, SortField::Title, SortDir::Asc, &filters);
            let elapsed = start.elapsed();
            eprintln!(
                "grouped() once, {group_by:?}, 300 songs: {elapsed:?} ({} groups)",
                groups.len()
            );
            assert!(
                elapsed.as_millis() < 50,
                "a single grouped() call under {group_by:?} took {elapsed:?}, expected well under 50ms"
            );
        }
    }

    /// What `library.rs` actually pays per render, not per call.
    ///
    /// Card J2 flagged "`view.grouped(...)` four separate times per
    /// render" and asked for a measurement rather than trusting the count.
    /// The real number, read off `src/screens/library.rs`, is worse than
    /// four and depends on how many groups are mounted: the main `for`
    /// loop calls it once, the alphabet rail's `present_letters` call
    /// (First letter grouping only) calls it once more, and
    /// `group_entry`/`group_rows` call `songs_in_group`, which calls it
    /// *again*, once per mounted group — and every group is mounted by
    /// default, since `LibraryViewStore::is_collapsed` starts empty. For
    /// First letter grouping at the 300-song target that is 26 groups: 28
    /// full filter+bucket+sort passes over the whole book, to paint one
    /// frame nobody scrolled or typed into.
    ///
    /// This times that real call count against a single call, so the
    /// multiplier is a measured number rather than an assumption on either
    /// side of this card's argument.
    #[test]
    fn librarys_repeated_grouped_calls_cost_as_many_full_passes_as_it_makes() {
        use std::time::Instant;

        let songs = library_of(300);
        let filters = Filters::default();
        let group_by = GroupBy::FirstLetter;

        let once = {
            let start = Instant::now();
            let groups = grouped(songs.clone(), group_by, SortField::Title, SortDir::Asc, &filters);
            (start.elapsed(), groups.len())
        };
        let group_count = once.1;
        // The main `for`, the alphabet rail, and one `songs_in_group` call
        // per mounted group — every group, since nothing starts collapsed.
        let calls_per_render = 2 + group_count;

        let start = Instant::now();
        for _ in 0..calls_per_render {
            std::hint::black_box(grouped(songs.clone(), group_by, SortField::Title, SortDir::Asc, &filters));
        }
        let repeated = start.elapsed();

        eprintln!(
            "single grouped() call: {:?}; library.rs's actual {calls_per_render} calls per render \
             (First letter, {group_count} groups, nothing collapsed): {repeated:?} — {:.1}x a single call",
            once.0,
            repeated.as_secs_f64() / once.0.as_secs_f64().max(1e-9)
        );
        assert!(
            repeated.as_millis() < 200,
            "library.rs's real per-render grouped() cost took {repeated:?}, expected well under 200ms"
        );
    }

    // ── helpers ─────────────────────────────────────────────────────────

    fn durated(id: u32, seconds: u32) -> Song {
        let mut s = song(id, &format!("Song {id}"), "A");
        s.duration = Some(seconds);
        s
    }

    fn titles(songs: &[Song]) -> Vec<String> {
        songs.iter().map(|s| s.title.clone()).collect()
    }
}

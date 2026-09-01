//! Values the app computes rather than stores.
//!
//! Group buckets, sort order, cumulative setlist times, total runtime and the
//! "Before you start" prep facts are all derived on read. Keeping them here as
//! plain functions — no signals, no stores — means the rules the screens are
//! thin over can be tested without a window.

use rinch_tabler_icons::TablerIcon;

use crate::model::{Confidence, Day, Setlist, Song, fmt_duration};
use crate::store::{Group, GroupBy, SortDir, SortField};

/// How many rows a group shows before "Show N more".
pub const GROUP_PREVIEW: usize = 6;

/// Songs matching a search query, over title, artist and tags. An empty query
/// matches everything.
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
pub fn grouped(
    songs: Vec<Song>,
    query: &str,
    group_by: GroupBy,
    sort_field: SortField,
    sort_dir: SortDir,
) -> Vec<Group> {
    let mut groups = group_songs(filter_songs(songs, query), group_by);
    for group in &mut groups {
        sort_songs(&mut group.songs, sort_field, sort_dir);
    }
    groups
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
pub fn prep_facts(songs: &[Song]) -> String {
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

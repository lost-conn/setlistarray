//! Values the app computes rather than stores.
//!
//! Group buckets, sort order, cumulative setlist times, total runtime and the
//! "Before you start" prep facts are all derived on read. Keeping them here as
//! plain functions — no signals, no stores — means the rules the screens are
//! thin over can be tested without a window.

use crate::model::{Confidence, Setlist, Song, fmt_duration};
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
pub fn direction_arrow(field: SortField, dir: SortDir) -> &'static str {
    let downward = matches!(field, SortField::LastPlayed | SortField::DateAdded);
    match (dir, downward) {
        (SortDir::Asc, false) | (SortDir::Desc, true) => "↑",
        (SortDir::Asc, true) | (SortDir::Desc, false) => "↓",
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
        // "A → Z" reads up the alphabet; "newest first" reads down the calendar.
        assert_eq!(direction_arrow(SortField::Artist, SortDir::Asc), "↑");
        assert_eq!(direction_arrow(SortField::Artist, SortDir::Desc), "↓");
        assert_eq!(direction_arrow(SortField::LastPlayed, SortDir::Asc), "↓");
        assert_eq!(direction_arrow(SortField::LastPlayed, SortDir::Desc), "↑");
        assert_eq!(direction_arrow(SortField::DateAdded, SortDir::Asc), "↓");
        assert_eq!(direction_arrow(SortField::Tempo, SortDir::Asc), "↑");
    }

    #[test]
    fn every_sort_field_has_words_and_an_arrow_in_both_directions() {
        for field in SortField::ALL {
            for dir in [SortDir::Asc, SortDir::Desc] {
                assert!(!field.direction_label(dir).is_empty(), "{field:?}");
                assert!(["↑", "↓"].contains(&direction_arrow(field, dir)), "{field:?}");
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

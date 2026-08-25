//! Values the app computes rather than stores.
//!
//! Group buckets, sort order, cumulative setlist times, total runtime and the
//! "Before you start" prep facts are all derived on read. Keeping them here as
//! plain functions — no signals, no stores — means the rules the screens are
//! thin over can be tested without a window.

use crate::model::{Confidence, Song};
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

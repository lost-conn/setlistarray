//! Demo content so the screens have something honest to render. Replace with
//! the on-device store (SQLite + an attachments directory) when persistence
//! lands.

use crate::model::{Attachment, AttachmentKind, Confidence, Day, Setlist, Song};

fn song(
    id: u32,
    title: &str,
    artist: &str,
    confidence: Option<Confidence>,
    key: Option<&str>,
    tempo: Option<u16>,
    duration: Option<u32>,
    attachment: Option<u32>,
) -> Song {
    let mut s = Song::new(id, title, artist);
    s.confidence = confidence;
    s.key = key.map(str::to_string);
    s.tempo = tempo;
    s.duration = duration;
    if let Some(a) = attachment {
        s.attachments.push(a);
        s.primary_attachment = Some(a);
    }
    s
}

pub fn songs() -> Vec<Song> {
    let mut list = vec![
        song(1, "Carolina", "M. Ward", Some(Confidence::Solid), Some("G"), Some(96), Some(224), Some(101)),
        song(2, "Angel From Montgomery", "John Prine", Some(Confidence::Solid), Some("D"), Some(88), Some(242), Some(102)),
        song(3, "Blackbird", "The Beatles", Some(Confidence::Solid), Some("G"), Some(94), Some(138), Some(103)),
        song(4, "The Weight", "The Band", Some(Confidence::Solid), Some("A"), Some(76), Some(275), None),
        song(5, "Landslide", "Fleetwood Mac", Some(Confidence::Solid), Some("Eb"), None, Some(199), Some(104)),
        song(6, "Wagon Wheel", "Old Crow Medicine Show", Some(Confidence::Rusty), Some("A"), Some(148), Some(232), Some(105)),
        song(7, "Wish You Were Here", "Pink Floyd", Some(Confidence::Rusty), Some("G"), Some(60), Some(334), None),
        song(8, "Harvest Moon", "Neil Young", Some(Confidence::Rusty), Some("D"), Some(120), Some(302), Some(106)),
        song(9, "Nightswimming", "R.E.M.", Some(Confidence::Learning), Some("C"), Some(112), None, None),
        song(10, "Pink Moon", "Nick Drake", Some(Confidence::Learning), Some("C"), None, Some(122), Some(107)),
        song(11, "Skinny Love", "Bon Iver", Some(Confidence::Learning), None, None, None, None),
        song(12, "Fake Plastic Trees", "Radiohead", None, Some("A"), Some(72), Some(290), None),
        song(13, "Big Yellow Taxi", "Joni Mitchell", None, None, None, None, None),
        song(14, "Ripple", "Grateful Dead", None, Some("G"), Some(96), Some(266), Some(108)),
    ];

    // A few played dates so the "played March" meta variant has something to
    // say and last-played sorting is meaningful.
    list[0].last_played = Some(Day::new(2026, 6, 2));
    list[1].last_played = Some(Day::new(2026, 3, 14));
    list[3].last_played = Some(Day::new(2026, 3, 2));
    list[5].last_played = Some(Day::new(2026, 1, 20));

    list[0].tuning = Some("Standard".into());
    list[2].capo = Some(2);
    list[4].tuning = Some("Open D".into());
    list[4].capo = Some(3);
    list[7].tuning = Some("Double drop D".into());

    list[0].tags = vec!["campfire".into()];
    list[2].tags = vec!["fingerstyle".into()];
    list[5].tags = vec!["campfire".into(), "crowd".into()];

    list
}

pub fn attachments() -> Vec<Attachment> {
    vec![
        Attachment {
            id: 101,
            kind: AttachmentKind::Pdf,
            title: "carolina-chords.pdf".into(),
            bytes_on_disk: 412_000,
            page_count: Some(2),
            source_url: None,
            captured_at: None,
            body: None,
        },
        Attachment {
            id: 102,
            kind: AttachmentKind::CapturedPage,
            title: "Angel From Montgomery — chords".into(),
            bytes_on_disk: 1_240_000,
            page_count: None,
            source_url: Some("https://tabs.example/angel-from-montgomery".into()),
            captured_at: Some(Day::new(2026, 5, 9)),
            body: Some("[D] I am an old woman, named after my mother…".into()),
        },
        Attachment {
            id: 103,
            kind: AttachmentKind::Pdf,
            title: "blackbird-tab.pdf".into(),
            bytes_on_disk: 690_000,
            page_count: Some(3),
            source_url: None,
            captured_at: None,
            body: None,
        },
        Attachment {
            id: 104,
            kind: AttachmentKind::Text,
            title: "Landslide — my version".into(),
            bytes_on_disk: 2_100,
            page_count: None,
            source_url: None,
            captured_at: None,
            body: Some("Capo 3. Eb shapes played as C.".into()),
        },
        Attachment {
            id: 105,
            kind: AttachmentKind::CapturedPage,
            title: "Wagon Wheel — chords".into(),
            bytes_on_disk: 980_000,
            page_count: None,
            source_url: Some("https://tabs.example/wagon-wheel".into()),
            captured_at: Some(Day::new(2026, 2, 2)),
            body: None,
        },
        Attachment {
            id: 106,
            kind: AttachmentKind::Pdf,
            title: "harvest-moon.pdf".into(),
            bytes_on_disk: 331_000,
            page_count: Some(1),
            source_url: None,
            captured_at: None,
            body: None,
        },
        Attachment {
            id: 107,
            kind: AttachmentKind::Text,
            title: "Pink Moon — lyrics".into(),
            bytes_on_disk: 1_400,
            page_count: None,
            source_url: None,
            captured_at: None,
            body: None,
        },
        Attachment {
            id: 108,
            kind: AttachmentKind::Pdf,
            title: "ripple.pdf".into(),
            bytes_on_disk: 205_000,
            page_count: Some(2),
            source_url: None,
            captured_at: None,
            body: None,
        },
    ]
}

pub fn setlists() -> Vec<Setlist> {
    vec![
        Setlist {
            id: 1,
            name: "Porch, Saturday".into(),
            song_ids: vec![1, 2, 3, 5, 6],
            last_played: Some(Day::new(2026, 8, 14)),
        },
        Setlist {
            id: 2,
            name: "Quiet set".into(),
            song_ids: vec![10, 7, 9],
            last_played: None,
        },
    ]
}

//! Demo content, for screenshots and tests.
//!
//! Not on the startup path any more: a fresh install opens an empty library.
//! `--seed` writes this into the library, but only into an empty one, so
//! running it twice does not give you two of everything.
//!
//! The ids here are the demo's own. [`install`] does not keep them — the
//! database mints ids, and object ids are the domain ids, so the demo is
//! remapped on the way in exactly as any real content would be.
//!
//! ## Why three of the demo songs are traditional, and why they carry real text
//!
//! Every attachment in here used to be metadata — a title, a page count, a
//! byte size — with nothing behind it, because `--seed` builds its library
//! with no directory under any attachment (see [`install`] and
//! `AttachmentsStore::directory`). That is harmless for a sort test and it was
//! fine while the demo existed only to fill a list. It stopped being fine when
//! card S1 went to photograph the app for a store listing: performance mode,
//! the full-screen viewer and search-inside-a-chart are three of the things
//! this app is *for*, and all three photographed as empty states, because
//! there was not one chart in the library with a line of text in it.
//!
//! So three songs here are typed charts that actually render. They are
//! traditional — "Wildwood Flower" (published 1860 as "I'll Twine 'Mid the
//! Ringlets"), "Shenandoah" and "Scarborough Fair", all long out of copyright
//! anywhere this app is downloadable — and that is the whole of why those
//! three and not three songs somebody would rather see. A screenshot on a
//! public store page is a publication: putting a verse of a song still in
//! copyright on it is a licensing problem with a picture of it attached, and
//! the same is true of the seed generally, which is why attachment 102's body
//! no longer quotes the John Prine line it used to.
//!
//! The charts are written the way a chord chart is written — chords on the
//! line above the syllable they fall on, monospaced, columns load-bearing —
//! and every line of all three is inside 40 columns. That is not a rounding
//! of "short enough"; it is the budget `chart_surface::STAGE_CHART_PX`'s own
//! comment sets out, the width at which 18.5 px type still fits a 393 px
//! phone without a lyric running off the right of the stand.

use std::collections::HashMap;

use crate::db::DbResult;
use crate::db::repo::Repo;
use crate::model::{
    Attachment, AttachmentId, AttachmentKind, Confidence, Day, Setlist, Song, SongId,
};

/// The typed charts the three traditional songs carry, verbatim.
///
/// Written out here rather than assembled from parts because a chord chart is
/// a *picture* made of characters: the only thing that puts `G7` over "raven"
/// is that it sits in column 6 of the line above it, and the moment this is
/// built by a formatter somewhere that alignment becomes something a refactor
/// can silently move. `white-space: pre` and a monospaced face carry it from
/// here to the screen unchanged (`screens::chart_surface`), so what is typed
/// in this file is exactly what a reader sees on a music stand.
///
/// Flush against the left margin for the same reason. Rust has no indented
/// string literal, so any indentation written for the benefit of this file
/// would be indentation drawn on the phone.
const WILDWOOD_FLOWER_CHART: &str = "\
Wildwood Flower
Traditional - key of C, capo 2

Verse 1

C
I'll twine 'mid the ringlets
      G7          C
of my raven black hair
C
The lilies so pale and the
G7       C
roses so fair
C
The myrtle so bright with an
F       C
emerald hue
             G7
And the pale aronatus with
               C
eyes of bright blue

Verse 2

C
I'll dance and I'll sing and
   G7            C
my life shall be gay
C
I'll charm every heart in the
G7         C
crowd I survey
C
Though my heart now is breaking
   F           C
he never shall know
        G7
How his name makes me tremble
   C
my pale cheeks to glow";

const SHENANDOAH_CHART: &str = "\
Shenandoah
Traditional - key of D, free time

Verse 1

D                G       D
Oh Shenandoah, I long to see you
 D                A
Away, you rolling river
D                G       D
Oh Shenandoah, I long to see you
 Bm       A      D
Away, I'm bound away
           G       A  D
'Cross the wide Missouri

Verse 2

D                G         D
Oh Shenandoah, I love your daughter
 D                A
Away, you rolling river
D                      G       D
For her I'd cross your roaming water
 Bm       A      D
Away, I'm bound away
           G       A  D
'Cross the wide Missouri";

const SCARBOROUGH_FAIR_CHART: &str = "\
Scarborough Fair
Traditional - key of Am, 3/4

Verse 1

Am               G           Am
Are you going to Scarborough Fair?
Am       C         G        Am
Parsley, sage, rosemary and thyme
  Am           C       G     Am
Remember me to one who lives there
Am       G                  Am
She once was a true love of mine

Verse 2

Am                    G       Am
Tell her to make me a cambric shirt
Am       C         G        Am
Parsley, sage, rosemary and thyme
    Am              G     Am
Without no seam nor needlework
Am          G                 Am
Then she'll be a true love of mine";

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
        // The three with a chart behind them. "Traditional" sorts after every
        // artist above it, which is deliberate rather than incidental: the
        // library's default grouping is by confidence and its default sort is
        // by artist ascending (`db::prefs`), so these land at the foot of
        // whichever group they are in and the first row of the first group —
        // the one `scripts/screenshot.sh` photographs — does not move.
        song(15, "Wildwood Flower", "Traditional", Some(Confidence::Solid), Some("C"), Some(112), Some(168), Some(109)),
        song(16, "Shenandoah", "Traditional", Some(Confidence::Solid), Some("D"), None, Some(205), Some(110)),
        song(17, "Scarborough Fair", "Traditional", Some(Confidence::Rusty), Some("Am"), Some(84), Some(191), Some(111)),
    ];

    // A few played dates so the "played March" meta variant has something to
    // say and last-played sorting is meaningful.
    list[0].last_played = Some(Day::new(2026, 6, 2));
    list[1].last_played = Some(Day::new(2026, 3, 14));
    list[3].last_played = Some(Day::new(2026, 3, 2));
    list[5].last_played = Some(Day::new(2026, 1, 20));
    list[14].last_played = Some(Day::new(2026, 7, 26));
    list[15].last_played = Some(Day::new(2026, 7, 26));

    list[0].tuning = Some("Standard".into());
    // The capo the Wildwood Flower chart's own second line says to put on.
    // Two places have to agree about a fact like this — the song's fields and
    // the chart's text — and the one a reader trusts is whichever they looked
    // at last, so they had better say the same thing.
    list[14].capo = Some(2);
    list[2].capo = Some(2);
    list[4].tuning = Some("Open D".into());
    list[4].capo = Some(3);
    list[7].tuning = Some("Double drop D".into());

    list[0].tags = vec!["campfire".into()];
    list[2].tags = vec!["fingerstyle".into()];
    list[5].tags = vec!["campfire".into(), "crowd".into()];
    list[14].tags = vec!["campfire".into(), "traditional".into()];
    list[15].tags = vec!["traditional".into()];
    list[16].tags = vec!["fingerstyle".into(), "traditional".into()];

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
            rechecked_at: None,
        },
        Attachment {
            id: 102,
            kind: AttachmentKind::CapturedPage,
            title: "Angel From Montgomery — chords".into(),
            bytes_on_disk: 1_240_000,
            page_count: None,
            source_url: Some("https://tabs.example/angel-from-montgomery".into()),
            captured_at: Some(Day::new(2026, 5, 9)),
            // What a capture's extracted text looks like without being a
            // verse of a song that is still in copyright. Card S1 replaced a
            // quoted line here for the reason its own header gives: this body
            // is searchable, it is drawn on song detail and in the viewer, and
            // both of those end up in a screenshot on a public store page.
            // A chart's *chords* are facts about a tune and its shape is a
            // fact about an arrangement; its words are somebody's.
            body: Some(
                "Verse, chorus, verse, chorus, solo over the verse, last chorus.\n\
                 D  -  F#m  -  G  -  D  through the verse; G  -  D  -  A on the turn.\n\
                 Capo 2 if you want it in the singer's key."
                    .into(),
            ),
            // `--seed` builds an in-memory library with no directory behind
            // any attachment — see `AttachmentsStore::directory` — so E6's
            // re-check has nothing on disk to read a mode off of and never
            // fires here regardless of this field. `None` is still the right
            // seed: it is what "never checked" actually looks like.
            rechecked_at: None,
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
            rechecked_at: None,
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
            rechecked_at: None,
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
            rechecked_at: None,
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
            rechecked_at: None,
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
            rechecked_at: None,
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
            rechecked_at: None,
        },
        // The three that are not just a row. `bytes_on_disk` is the chart's
        // own length rather than a plausible-looking number, because for a
        // typed chart that *is* what is on disk — `chart_editor` writes the
        // text and records its byte count — and a demo that says 2.1 kB over
        // 600 bytes of text is a demo teaching the storage line to lie.
        Attachment {
            id: 109,
            kind: AttachmentKind::Text,
            title: "Wildwood Flower — chords".into(),
            bytes_on_disk: WILDWOOD_FLOWER_CHART.len() as u64,
            page_count: None,
            source_url: None,
            captured_at: None,
            body: Some(WILDWOOD_FLOWER_CHART.into()),
            rechecked_at: None,
        },
        Attachment {
            id: 110,
            kind: AttachmentKind::Text,
            title: "Shenandoah — chords".into(),
            bytes_on_disk: SHENANDOAH_CHART.len() as u64,
            page_count: None,
            source_url: None,
            captured_at: None,
            body: Some(SHENANDOAH_CHART.into()),
            rechecked_at: None,
        },
        Attachment {
            id: 111,
            kind: AttachmentKind::Text,
            title: "Scarborough Fair — chords".into(),
            bytes_on_disk: SCARBOROUGH_FAIR_CHART.len() as u64,
            page_count: None,
            source_url: None,
            captured_at: None,
            body: Some(SCARBOROUGH_FAIR_CHART.into()),
            rechecked_at: None,
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
        // A set that opens on a chart. The two above it start on songs whose
        // primary attachment is a PDF or a capture with no bytes behind it —
        // true of every attachment in this demo before card S1 and still true
        // of most of them — so performance mode entered from either of them
        // begins on "This PDF could not be drawn." That is an honest picture
        // of an unfinished library and a poor picture of the feature, and the
        // fix is not to pretend the PDFs are there but to give the demo one
        // set whose first song genuinely has something to read off.
        Setlist {
            id: 3,
            name: "Campfire set".into(),
            song_ids: vec![15, 16, 17, 1, 6],
            last_played: Some(Day::new(2026, 7, 26)),
        },
    ]
}

/// Write the demo library through the repository, remapping every id.
///
/// Songs first, because an attachment is created against the song that owns it;
/// then the primary-attachment pointer, which cannot be known until the
/// attachment has an id; then the setlists, whose membership is remapped the
/// same way.
pub fn install(repo: &Repo) -> DbResult<()> {
    let demo_songs = songs();
    let demo_attachments: HashMap<AttachmentId, Attachment> =
        attachments().into_iter().map(|a| (a.id, a)).collect();

    let mut song_ids: HashMap<SongId, SongId> = HashMap::new();
    for song in &demo_songs {
        let mut stored = song.clone();
        stored.attachments.clear();
        stored.primary_attachment = None;
        let id = repo.create_song(&stored)? as SongId;
        song_ids.insert(song.id, id);
    }

    for song in &demo_songs {
        let owner = song_ids[&song.id];
        let mut attached: Vec<AttachmentId> = Vec::new();
        let mut primary = None;
        for old in &song.attachments {
            let Some(attachment) = demo_attachments.get(old) else {
                continue;
            };
            let id = repo.create_attachment(owner, attachment)? as AttachmentId;
            attached.push(id);
            if song.primary_attachment == Some(*old) {
                primary = Some(id);
            }
        }
        if attached.is_empty() {
            continue;
        }
        let mut stored = song.clone();
        stored.id = owner;
        stored.attachments = attached;
        stored.primary_attachment = primary;
        repo.save_song(&stored)?;
    }

    for setlist in setlists() {
        let mut stored = setlist.clone();
        stored.id = 0;
        stored.song_ids.clear();
        let id = repo.create_setlist(&stored)? as crate::model::SetlistId;
        let members: Vec<SongId> = setlist
            .song_ids
            .iter()
            .filter_map(|old| song_ids.get(old).copied())
            .collect();
        repo.set_members(id, &members)?;
    }

    Ok(())
}

//! Between the domain types and rhypedb objects.
//!
//! One rule runs through all of it: an unset field is *absent*, never an empty
//! string or a zero. The UI is built on "if it isn't there, don't render it",
//! and that has to survive a round trip through storage.

use rhypedb_engine::object::{FieldMap, Object, Value};

use crate::model::{
    Attachment, AttachmentId, AttachmentKind, Confidence, Day, Setlist, SetlistId, Song, SongId,
};

// ── field helpers ───────────────────────────────────────────────────────────

fn put(fields: &mut FieldMap, name: &str, value: Value) {
    fields.insert(name.to_string(), value);
}

fn put_some(fields: &mut FieldMap, name: &str, value: Option<Value>) {
    if let Some(value) = value {
        fields.insert(name.to_string(), value);
    }
}

fn string(fields: &FieldMap, name: &str) -> Option<String> {
    match fields.get(name) {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn u32(fields: &FieldMap, name: &str) -> Option<u32> {
    match fields.get(name) {
        Some(Value::U32(v)) => Some(*v),
        Some(Value::U64(v)) => u32::try_from(*v).ok(),
        _ => None,
    }
}

fn u64(fields: &FieldMap, name: &str) -> Option<u64> {
    match fields.get(name) {
        Some(Value::U64(v)) => Some(*v),
        Some(Value::U32(v)) => Some(*v as u64),
        _ => None,
    }
}

fn day(fields: &FieldMap, name: &str) -> Option<Day> {
    match fields.get(name) {
        Some(Value::DateTime(ms)) => Some(day_from_millis(*ms)),
        _ => None,
    }
}

// ── Day <-> epoch millis ────────────────────────────────────────────────────
//
// rhypedb stores DateTime as epoch milliseconds. A `Day` is a calendar date
// with no zone, so it maps to midnight UTC. Howard Hinnant's civil-date
// algorithms, which are exact for the range any music library will ever hold.

const MILLIS_PER_DAY: i64 = 86_400_000;

pub fn millis_from_day(day: Day) -> i64 {
    days_from_civil(day.year, day.month as i32, day.day as i32) * MILLIS_PER_DAY
}

pub fn day_from_millis(ms: i64) -> Day {
    // Floor-divide so pre-epoch dates land on the right day.
    let days = ms.div_euclid(MILLIS_PER_DAY);
    let (year, month, dom) = civil_from_days(days);
    Day::new(year, month as u8, dom as u8)
}

fn days_from_civil(y: i32, m: i32, d: i32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i32, i32, i32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    ((if m <= 2 { y + 1 } else { y }) as i32, m as i32, d as i32)
}

// ── Confidence, tags, attachment kind ───────────────────────────────────────

fn confidence_name(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Solid => "Solid",
        Confidence::Rusty => "Rusty",
        Confidence::Learning => "Learning",
    }
}

fn confidence_from(name: &str) -> Option<Confidence> {
    match name {
        "Solid" => Some(Confidence::Solid),
        "Rusty" => Some(Confidence::Rusty),
        "Learning" => Some(Confidence::Learning),
        _ => None,
    }
}

fn kind_name(kind: AttachmentKind) -> &'static str {
    match kind {
        AttachmentKind::Pdf => "Pdf",
        AttachmentKind::CapturedPage => "CapturedPage",
        AttachmentKind::Text => "Text",
    }
}

fn kind_from(name: &str) -> AttachmentKind {
    match name {
        "CapturedPage" => AttachmentKind::CapturedPage,
        "Text" => AttachmentKind::Text,
        _ => AttachmentKind::Pdf,
    }
}

/// Tags are stored as one comma-separated string: a song has a handful, and no
/// query filters on them in the database.
fn join_tags(tags: &[String]) -> Option<Value> {
    let joined: Vec<&str> = tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
    (!joined.is_empty()).then(|| Value::String(joined.join(",")))
}

fn split_tags(fields: &FieldMap) -> Vec<String> {
    match string(fields, "tags") {
        Some(s) => s
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        None => Vec::new(),
    }
}

// ── Song ────────────────────────────────────────────────────────────────────

pub fn song_fields(song: &Song) -> FieldMap {
    let mut fields = FieldMap::new();
    put(&mut fields, "title", Value::String(song.title.clone()));
    put(&mut fields, "artist", Value::String(song.artist.clone()));
    put_some(&mut fields, "song_key", song.key.clone().map(Value::String));
    put_some(&mut fields, "tempo", song.tempo.map(|t| Value::U32(t as u32)));
    put_some(&mut fields, "tuning", song.tuning.clone().map(Value::String));
    put_some(&mut fields, "capo", song.capo.map(|c| Value::U32(c as u32)));
    put_some(&mut fields, "duration", song.duration.map(Value::U32));
    put_some(&mut fields, "tags", join_tags(&song.tags));
    put_some(
        &mut fields,
        "confidence",
        song.confidence
            .map(|c| Value::String(confidence_name(c).to_string())),
    );
    put_some(&mut fields, "notes", song.notes.clone().map(Value::String));
    put_some(
        &mut fields,
        "last_played",
        song.last_played.map(|d| Value::DateTime(millis_from_day(d))),
    );
    put(
        &mut fields,
        "created_at",
        Value::DateTime(song.created_at as i64),
    );
    put_some(
        &mut fields,
        "primary_attachment",
        song.primary_attachment.map(|id| Value::U64(id as u64)),
    );
    fields
}

/// Every optional song field, by the name it has in the schema. `song_fields`
/// writes only the ones a song has; an update has to say something about the
/// rest, or clearing a key on screen leaves the old key on disk.
const OPTIONAL_SONG_FIELDS: [&str; 10] = [
    "song_key",
    "tempo",
    "tuning",
    "capo",
    "duration",
    "tags",
    "confidence",
    "notes",
    "last_played",
    "primary_attachment",
];

/// The field map for a *rewrite*: what the song has, plus an explicit `Null`
/// for everything it no longer has. `update` merges, so an absent key would
/// preserve the old value rather than clear it — the one place where "unset is
/// absent" needs saying out loud.
pub fn song_updates(song: &Song) -> FieldMap {
    let mut fields = song_fields(song);
    for name in OPTIONAL_SONG_FIELDS {
        fields.entry(name.to_string()).or_insert(Value::Null);
    }
    fields
}

/// Rebuild a song. `attachments` comes from the relationship, which the caller
/// reads separately — an object carries its scalars, not its links.
pub fn song_from(object: &Object, attachments: Vec<AttachmentId>) -> Song {
    let f = &object.fields;
    Song {
        id: object.id as SongId,
        title: string(f, "title").unwrap_or_default(),
        artist: string(f, "artist").unwrap_or_default(),
        key: string(f, "song_key"),
        tempo: u32(f, "tempo").map(|t| t as u16),
        tuning: string(f, "tuning"),
        capo: u32(f, "capo").map(|c| c as u8),
        duration: u32(f, "duration"),
        tags: split_tags(f),
        confidence: string(f, "confidence").and_then(|c| confidence_from(&c)),
        notes: string(f, "notes"),
        last_played: day(f, "last_played"),
        created_at: match f.get("created_at") {
            Some(Value::DateTime(ms)) => *ms as u64,
            _ => 0,
        },
        primary_attachment: u64(f, "primary_attachment").map(|id| id as AttachmentId),
        attachments,
    }
}

// ── Attachment ──────────────────────────────────────────────────────────────

pub fn attachment_fields(attachment: &Attachment) -> FieldMap {
    let mut fields = FieldMap::new();
    put(
        &mut fields,
        "kind",
        Value::String(kind_name(attachment.kind).to_string()),
    );
    put(&mut fields, "title", Value::String(attachment.title.clone()));
    put(
        &mut fields,
        "bytes_on_disk",
        Value::U64(attachment.bytes_on_disk),
    );
    put_some(&mut fields, "page_count", attachment.page_count.map(Value::U32));
    put_some(
        &mut fields,
        "source_url",
        attachment.source_url.clone().map(Value::String),
    );
    put_some(
        &mut fields,
        "captured_at",
        attachment
            .captured_at
            .map(|d| Value::DateTime(millis_from_day(d))),
    );
    put_some(&mut fields, "body", attachment.body.clone().map(Value::String));
    fields
}

pub fn attachment_from(object: &Object) -> Attachment {
    let f = &object.fields;
    Attachment {
        id: object.id as AttachmentId,
        kind: string(f, "kind").map(|k| kind_from(&k)).unwrap_or(AttachmentKind::Pdf),
        title: string(f, "title").unwrap_or_default(),
        bytes_on_disk: u64(f, "bytes_on_disk").unwrap_or(0),
        page_count: u32(f, "page_count"),
        source_url: string(f, "source_url"),
        captured_at: day(f, "captured_at"),
        body: string(f, "body"),
    }
}

// ── Setlist ─────────────────────────────────────────────────────────────────

pub fn setlist_fields(setlist: &Setlist) -> FieldMap {
    let mut fields = FieldMap::new();
    put(&mut fields, "name", Value::String(setlist.name.clone()));
    put_some(
        &mut fields,
        "last_played",
        setlist
            .last_played
            .map(|d| Value::DateTime(millis_from_day(d))),
    );
    fields
}

/// The setlist counterpart to [`song_updates`]: a rewrite says what the setlist
/// no longer has, so clearing a played date clears it on disk.
pub fn setlist_updates(setlist: &Setlist) -> FieldMap {
    let mut fields = setlist_fields(setlist);
    fields
        .entry("last_played".to_string())
        .or_insert(Value::Null);
    fields
}

/// `song_ids` comes from the membership relationship, ordered by its
/// `position` edge field.
pub fn setlist_from(object: &Object, song_ids: Vec<SongId>) -> Setlist {
    let f = &object.fields;
    Setlist {
        id: object.id as SetlistId,
        name: string(f, "name").unwrap_or_default(),
        song_ids,
        last_played: day(f, "last_played"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object_of(type_name: &str, id: u64, fields: FieldMap) -> Object {
        Object {
            type_name: type_name.to_string(),
            id,
            fields,
            raw_fields: None,
        }
    }

    #[test]
    fn a_day_survives_the_round_trip() {
        for day in [
            Day::new(2026, 8, 25),
            Day::new(2026, 1, 1),
            Day::new(2026, 12, 31),
            Day::new(2024, 2, 29), // leap day
            Day::new(1969, 7, 20), // before the epoch
            Day::new(2000, 3, 1),  // the century-leap-year edge
        ] {
            assert_eq!(day_from_millis(millis_from_day(day)), day, "{day:?}");
        }
    }

    #[test]
    fn the_epoch_is_where_it_should_be() {
        assert_eq!(millis_from_day(Day::new(1970, 1, 1)), 0);
        assert_eq!(day_from_millis(0), Day::new(1970, 1, 1));
    }

    #[test]
    fn a_bare_song_stores_only_what_it_has() {
        let song = Song::new(1, "Skinny Love", "Bon Iver");
        let fields = song_fields(&song);

        assert!(fields.contains_key("title"));
        assert!(fields.contains_key("artist"));
        assert!(fields.contains_key("created_at"));
        for absent in [
            "song_key", "tempo", "tuning", "capo", "duration", "tags", "confidence", "notes",
            "last_played", "primary_attachment",
        ] {
            assert!(!fields.contains_key(absent), "{absent} should be absent");
        }
    }

    #[test]
    fn a_filled_song_survives_the_round_trip() {
        let mut song = Song::new(7, "Landslide", "Fleetwood Mac");
        song.key = Some("Eb".into());
        song.tempo = Some(96);
        song.tuning = Some("Open D".into());
        song.capo = Some(3);
        song.duration = Some(199);
        song.tags = vec!["campfire".into(), "crowd".into()];
        song.confidence = Some(Confidence::Solid);
        song.notes = Some("Capo 3, Eb shapes as C.".into());
        song.last_played = Some(Day::new(2026, 6, 2));
        song.primary_attachment = Some(104);
        song.attachments = vec![104];

        let object = object_of("Song", 7, song_fields(&song));
        let back = song_from(&object, vec![104]);
        assert_eq!(back, song);
    }

    #[test]
    fn an_unrated_song_comes_back_unrated_not_defaulted() {
        let song = Song::new(3, "Fake Plastic Trees", "Radiohead");
        let object = object_of("Song", 3, song_fields(&song));
        let back = song_from(&object, Vec::new());
        assert_eq!(back.confidence, None);
        assert_eq!(back.key, None);
        assert!(back.tags.is_empty());
    }

    #[test]
    fn tags_are_trimmed_and_empty_ones_dropped() {
        let mut song = Song::new(1, "One", "A");
        song.tags = vec!["  campfire ".into(), "".into(), "crowd".into()];
        let fields = song_fields(&song);
        assert_eq!(
            fields.get("tags"),
            Some(&Value::String("campfire,crowd".into()))
        );
        assert_eq!(split_tags(&fields), vec!["campfire", "crowd"]);
    }

    #[test]
    fn an_attachment_survives_the_round_trip() {
        let attachment = Attachment {
            id: 102,
            kind: AttachmentKind::CapturedPage,
            title: "Angel From Montgomery — chords".into(),
            bytes_on_disk: 1_240_000,
            page_count: None,
            source_url: Some("https://tabs.example/angel".into()),
            captured_at: Some(Day::new(2026, 5, 9)),
            body: Some("[D] I am an old woman…".into()),
        };
        let object = object_of("Attachment", 102, attachment_fields(&attachment));
        assert_eq!(attachment_from(&object), attachment);
    }

    #[test]
    fn every_attachment_kind_names_itself_reversibly() {
        for kind in [
            AttachmentKind::Pdf,
            AttachmentKind::CapturedPage,
            AttachmentKind::Text,
        ] {
            assert_eq!(kind_from(kind_name(kind)), kind);
        }
    }

    #[test]
    fn an_update_clears_the_fields_a_song_no_longer_has() {
        let mut song = Song::new(7, "Landslide", "Fleetwood Mac");
        song.key = Some("Eb".into());
        let updates = song_updates(&song);

        assert_eq!(updates.get("song_key"), Some(&Value::String("Eb".into())));
        for cleared in ["tempo", "tuning", "capo", "duration", "tags", "confidence",
                        "notes", "last_played", "primary_attachment"] {
            assert_eq!(updates.get(cleared), Some(&Value::Null), "{cleared}");
        }
    }

    #[test]
    fn a_cleared_field_reads_back_as_absent_not_as_a_blank() {
        let song = Song::new(7, "Landslide", "Fleetwood Mac");
        let object = object_of("Song", 7, song_updates(&song));
        let back = song_from(&object, Vec::new());
        assert_eq!(back.key, None);
        assert_eq!(back.tempo, None);
        assert_eq!(back.confidence, None);
        assert!(back.tags.is_empty());
    }

    #[test]
    fn a_setlist_survives_the_round_trip() {
        let setlist = Setlist {
            id: 1,
            name: "Porch, Saturday".into(),
            song_ids: vec![3, 1, 2],
            last_played: Some(Day::new(2026, 8, 14)),
        };
        let object = object_of("Setlist", 1, setlist_fields(&setlist));
        assert_eq!(setlist_from(&object, vec![3, 1, 2]), setlist);
    }
}

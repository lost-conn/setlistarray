//! Domain types. Every field but `id`, `title`, `artist` and `created_at` is
//! optional, and an unfilled field is simply absent from the UI — never a
//! placeholder dash.

pub type SongId = u32;
pub type SetlistId = u32;
pub type AttachmentId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    Solid,
    Rusty,
    Learning,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Confidence::Solid => "Solid",
            Confidence::Rusty => "Rusty",
            Confidence::Learning => "Learning",
        }
    }

    /// Filled dots out of three, and which colour token fills them.
    pub fn dots(self) -> u8 {
        match self {
            Confidence::Solid => 3,
            Confidence::Rusty => 2,
            Confidence::Learning => 1,
        }
    }

    /// Solid uses the accent; anything shakier uses the dimmed accent.
    pub fn dot_color(self) -> &'static str {
        match self {
            Confidence::Solid => "var(--sla-accent)",
            _ => "var(--sla-accent-dim)",
        }
    }
}

/// A calendar day with no time zone and no dependency. Enough for
/// "played Jun 2" and for sorting by recency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Day {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Day {
    pub const fn new(year: i32, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    fn month_name(self) -> &'static str {
        const NAMES: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        NAMES[(self.month.clamp(1, 12) - 1) as usize]
    }

    /// "Jun 2" — the form used on song detail and setlist rows.
    pub fn short(self) -> String {
        format!("{} {}", self.month_name(), self.day)
    }

    /// "March" — used in a library row when the month alone says enough.
    pub fn month_long(self) -> &'static str {
        const NAMES: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        NAMES[(self.month.clamp(1, 12) - 1) as usize]
    }

    /// Today, read from the system clock (UTC — the calendar day, not the
    /// wall clock, is all "mark played today" needs). No date-library
    /// dependency, matching the rest of `Day`: just Howard Hinnant's
    /// `civil_from_days` over seconds-since-epoch.
    pub fn today() -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = (secs / 86_400) as i64;
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month: month as u8,
            day: day as u8,
        }
    }
}

/// Days-since-the-Unix-epoch to a proleptic-Gregorian (year, month, day).
/// <http://howardhinnant.github.io/date_algorithms.html#civil_from_days>
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m, d)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentKind {
    Pdf,
    CapturedPage,
    Text,
}

impl AttachmentKind {
    /// The 9px uppercase label on a library row thumb.
    pub fn badge(self) -> &'static str {
        match self {
            AttachmentKind::Pdf => "PDF",
            AttachmentKind::CapturedPage => "WEB",
            AttachmentKind::Text => "TXT",
        }
    }

    /// The qualifier after an attachment name in a collapsed row.
    pub fn descriptor(self) -> &'static str {
        match self {
            AttachmentKind::Pdf => "pdf",
            AttachmentKind::CapturedPage => "saved page",
            AttachmentKind::Text => "typed",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attachment {
    pub id: AttachmentId,
    pub kind: AttachmentKind,
    pub title: String,
    pub bytes_on_disk: u64,
    pub page_count: Option<u32>,
    pub source_url: Option<String>,
    pub captured_at: Option<Day>,
    /// Extracted text, for TXT attachments and for search inside captures.
    pub body: Option<String>,
}

// Default exists so `Song` can be a component prop: the rsx macro builds
// props with `..Default::default()`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Song {
    pub id: SongId,
    pub title: String,
    pub artist: String,
    pub key: Option<String>,
    pub tempo: Option<u16>,
    pub tuning: Option<String>,
    pub capo: Option<u8>,
    /// Seconds.
    pub duration: Option<u32>,
    pub tags: Vec<String>,
    pub confidence: Option<Confidence>,
    pub notes: Option<String>,
    pub last_played: Option<Day>,
    pub created_at: u64,
    pub attachments: Vec<AttachmentId>,
    pub primary_attachment: Option<AttachmentId>,
}

impl Song {
    pub fn new(id: SongId, title: impl Into<String>, artist: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            artist: artist.into(),
            key: None,
            tempo: None,
            tuning: None,
            capo: None,
            duration: None,
            tags: Vec::new(),
            confidence: None,
            notes: None,
            last_played: None,
            created_at: id as u64,
            attachments: Vec::new(),
            primary_attachment: None,
        }
    }

    /// `Artist · key · tempo`, skipping anything unfilled. Never renders an
    /// empty slot.
    pub fn meta_line(&self) -> String {
        let mut parts = vec![self.artist.clone()];
        if let Some(k) = &self.key {
            parts.push(k.clone());
        }
        if let Some(t) = self.tempo {
            parts.push(format!("{t} bpm"));
        }
        parts.join(" · ")
    }

    /// The variant used when last-played is the more informative fact.
    pub fn meta_line_played(&self) -> String {
        match self.last_played {
            Some(d) => format!("{} · played {}", self.artist, d.month_long()),
            None => self.meta_line(),
        }
    }

    pub fn has_chart(&self) -> bool {
        !self.attachments.is_empty()
    }
}

/// `3:44` — the form used down the right edge of a setlist.
pub fn fmt_duration(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Setlist {
    pub id: SetlistId,
    pub name: String,
    /// Order is the vector order. Membership is by reference — removing a song
    /// from a setlist never touches the song.
    pub song_ids: Vec<SongId>,
    pub last_played: Option<Day>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_as_minutes_and_seconds() {
        assert_eq!(fmt_duration(0), "0:00");
        assert_eq!(fmt_duration(9), "0:09");
        assert_eq!(fmt_duration(224), "3:44");
        assert_eq!(fmt_duration(3600), "60:00");
    }

    #[test]
    fn a_meta_line_holds_only_the_fields_that_exist() {
        let mut song = Song::new(1, "Carolina", "M. Ward");
        assert_eq!(song.meta_line(), "M. Ward");

        song.key = Some("G".into());
        assert_eq!(song.meta_line(), "M. Ward · G");

        song.tempo = Some(96);
        assert_eq!(song.meta_line(), "M. Ward · G · 96 bpm");
    }

    #[test]
    fn a_tempo_with_no_key_still_reads_cleanly() {
        let mut song = Song::new(1, "Carolina", "M. Ward");
        song.tempo = Some(96);
        assert_eq!(song.meta_line(), "M. Ward · 96 bpm");
    }

    #[test]
    fn the_played_variant_falls_back_when_nothing_was_played() {
        let mut song = Song::new(1, "Carolina", "M. Ward");
        song.key = Some("G".into());
        assert_eq!(song.meta_line_played(), song.meta_line());

        song.last_played = Some(Day::new(2026, 3, 14));
        assert_eq!(song.meta_line_played(), "M. Ward · played March");
    }

    #[test]
    fn days_render_short_and_long() {
        let day = Day::new(2026, 6, 2);
        assert_eq!(day.short(), "Jun 2");
        assert_eq!(day.month_long(), "June");
    }

    #[test]
    fn days_order_by_calendar() {
        assert!(Day::new(2026, 8, 14) > Day::new(2026, 3, 2));
        assert!(Day::new(2026, 3, 2) > Day::new(2025, 12, 31));
    }

    #[test]
    fn civil_from_days_matches_known_epoch_offsets() {
        // Day 0 is the Unix epoch itself.
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 10,957 days later is the well-known y2k reference point
        // (946684800 / 86400 == 10957).
        assert_eq!(civil_from_days(10_957), (2000, 1, 1));
        // A date the far side of a leap day, to exercise the leap-year math.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(19_783), (2024, 3, 1));
    }

    #[test]
    fn today_reads_a_plausible_calendar_day() {
        // Not pinned to a specific date (that would just re-implement the
        // clock) — just a sanity check that it lands in this decade and on a
        // real day of a real month.
        let day = Day::today();
        assert!(day.year >= 2024 && day.year < 2100);
        assert!((1..=12).contains(&day.month));
        assert!((1..=31).contains(&day.day));
    }

    #[test]
    fn confidence_fills_dots_and_dims_anything_shaky() {
        assert_eq!(Confidence::Solid.dots(), 3);
        assert_eq!(Confidence::Rusty.dots(), 2);
        assert_eq!(Confidence::Learning.dots(), 1);
        assert_eq!(Confidence::Solid.dot_color(), "var(--sla-accent)");
        assert_eq!(Confidence::Rusty.dot_color(), "var(--sla-accent-dim)");
        assert_eq!(Confidence::Learning.dot_color(), "var(--sla-accent-dim)");
    }

    #[test]
    fn a_song_has_a_chart_only_once_something_is_attached() {
        let mut song = Song::new(1, "Carolina", "M. Ward");
        assert!(!song.has_chart());
        song.attachments.push(101);
        assert!(song.has_chart());
    }

    #[test]
    fn attachment_kinds_carry_their_badge_and_descriptor() {
        assert_eq!(AttachmentKind::Pdf.badge(), "PDF");
        assert_eq!(AttachmentKind::CapturedPage.badge(), "WEB");
        assert_eq!(AttachmentKind::Text.badge(), "TXT");
        assert_eq!(AttachmentKind::CapturedPage.descriptor(), "saved page");
        assert_eq!(AttachmentKind::Text.descriptor(), "typed");
    }
}

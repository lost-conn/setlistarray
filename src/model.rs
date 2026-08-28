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

    /// The calendar day `n` days after 1970-01-01 — the inverse of
    /// [`days_since_epoch`](Self::days_since_epoch).
    pub fn from_days_since_epoch(n: i64) -> Self {
        let (year, month, day) = civil_from_days(n);
        Self {
            year,
            month: month as u8,
            day: day as u8,
        }
    }

    /// Days since 1970-01-01, so that two `Day`s can be subtracted.
    ///
    /// `Day` deliberately has no time zone and no dependency, and ordering was
    /// all it needed until something wanted a *window* — the picker's "Recent"
    /// chip asks "within the last 90 days", which ordering alone cannot
    /// answer. This is the inverse of [`civil_from_days`], from the same
    /// source, so the two round-trip.
    pub fn days_since_epoch(self) -> i64 {
        days_from_civil(
            self.year,
            self.month.clamp(1, 12) as u32,
            self.day.max(1) as u32,
        )
    }
}

/// A proleptic-Gregorian (year, month, day) to days-since-the-Unix-epoch.
/// <http://howardhinnant.github.io/date_algorithms.html#days_from_civil>
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    // The algorithm counts years from March, so January and February belong to
    // the year before.
    let y = y as i64 - i64::from(m <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let mp = u64::from(if m > 2 { m - 3 } else { m + 9 }); // [0, 11]
    let doy = (153 * mp + 2) / 5 + u64::from(d) - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe as i64 - 719_468
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

    // ── Attachments ─────────────────────────────────────────────────────
    //
    // The three rules the handoff states about charts are rules about a
    // *song*, so they live on `Song` rather than in a store: they need no
    // database, no signal and no window, and there is exactly one copy of
    // each. The store's job is to write the result down.
    //
    //   1. The first chart added becomes the primary one.
    //   2. Expanding a collapsed chart does not make it primary — that is
    //      view state, and nothing here can be reached from it.
    //   3. A song with charts always has a primary one; a song with none
    //      never does.
    //
    // Rule 3 is the awkward one, because the card that asked for rules 1 and
    // 2 does not say what happens when the primary chart is *removed*.
    // [`settle_primary`](Self::settle_primary) is the answer, and every
    // mutation below goes through it.

    /// The chart the card shows, or `None` when the song has none.
    ///
    /// Read through here rather than off `primary_attachment` directly. The
    /// field can disagree with `attachments` — a library restored from a
    /// backup, a row that failed to save, a future migration — and an empty
    /// card sitting above three collapsed rows is a worse answer than the
    /// oldest chart. The fallback is the same rule that chose the primary in
    /// the first place, so a read never disagrees with a write.
    pub fn primary(&self) -> Option<AttachmentId> {
        self.primary_attachment
            .filter(|id| self.attachments.contains(id))
            .or_else(|| self.attachments.first().copied())
    }

    /// Attach a chart. The first one becomes primary; every one after it
    /// joins the collapsed rows under the card.
    ///
    /// Attaching the same id twice is a no-op rather than a second row — the
    /// list is a set that happens to be ordered, and the order is the order
    /// they were added.
    pub fn attach(&mut self, id: AttachmentId) {
        if self.attachments.contains(&id) {
            return;
        }
        self.attachments.push(id);
        self.settle_primary();
    }

    /// Remove a chart. `false` if the song never had it.
    ///
    /// **When the primary chart is removed, the oldest chart still attached
    /// takes its place.** The card does not say what should happen, so:
    /// leaving `primary_attachment` pointing at something that is gone is out
    /// (rule 3), and clearing it while charts remain is out for the same
    /// reason — it would leave a song showing an empty card above a list of
    /// its own charts. Between the two remaining candidates, "the oldest one
    /// left" is rule 1 applied a second time, and it is the one the user can
    /// predict: the row directly under the card is the row that moves into
    /// it.
    pub fn detach(&mut self, id: AttachmentId) -> bool {
        let before = self.attachments.len();
        self.attachments.retain(|a| *a != id);
        if self.attachments.len() == before {
            return false;
        }
        self.settle_primary();
        true
    }

    /// Promote a chart the song already has. `false` — and nothing changed —
    /// for one it does not, because a primary pointer into another song's
    /// library is the state rule 3 exists to prevent.
    pub fn set_primary(&mut self, id: AttachmentId) -> bool {
        if !self.attachments.contains(&id) {
            return false;
        }
        self.primary_attachment = Some(id);
        true
    }

    /// Rule 3, enforced. Keeps a primary that is still valid, promotes the
    /// oldest chart when it is not, and clears it when there are no charts
    /// left. Every mutation of `attachments` ends here, which is what makes
    /// "attachments but no primary" unreachable rather than merely unlikely.
    fn settle_primary(&mut self) {
        self.primary_attachment = self.primary();
    }
}

/// `3:44` — the form used down the right edge of a setlist.
pub fn fmt_duration(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// `412 KB` — what an attachment costs, in the words the handoff uses.
///
/// The wireframe's storage row (`5c`) reads `248 MB ›` and the attachment rows
/// this feeds are the same register, so the unit is decimal and the label is
/// the short one: nobody looking at a chart wants to read `1.4 MiB`, and the
/// distinction is not worth the character it costs on a phone row.
///
/// One decimal place from a megabyte up, none below — and a trailing `.0`
/// dropped, because the row the handoff drew reads `248 MB` and not
/// `248.0 MB`. A chart is rarely under a hundred kilobytes and rarely over ten
/// megabytes, so `412 KB` and `1.4 MB` are the two shapes this actually
/// produces; the rest exist so that no size can render as an empty string or a
/// wall of digits.
pub fn fmt_bytes(bytes: u64) -> String {
    const KB: u64 = 1_000;
    const MB: u64 = 1_000_000;
    const GB: u64 = 1_000_000_000;
    match bytes {
        b if b < KB => format!("{b} B"),
        // Rounded up, so a file that exists never reads as `0 KB`.
        b if b < MB => format!("{} KB", b.div_ceil(KB)),
        b if b < GB => format!("{} MB", one_decimal(b as f64 / MB as f64)),
        b => format!("{} GB", one_decimal(b as f64 / GB as f64)),
    }
}

/// `1.4`, `248`, `3.2`. The whole number keeps no decimal point.
fn one_decimal(value: f64) -> String {
    let text = format!("{value:.1}");
    text.strip_suffix(".0").unwrap_or(&text).to_string()
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
    fn a_size_reads_as_the_handoffs_storage_row_does() {
        assert_eq!(fmt_bytes(0), "0 B");
        assert_eq!(fmt_bytes(999), "999 B");
        assert_eq!(fmt_bytes(1_000), "1 KB");
        assert_eq!(fmt_bytes(1_001), "2 KB");
        assert_eq!(fmt_bytes(411_500), "412 KB");
        assert_eq!(fmt_bytes(1_400_000), "1.4 MB");
        // Exactly the string the handoff's storage row (`5c`) shows.
        assert_eq!(fmt_bytes(248_000_000), "248 MB");
        assert_eq!(fmt_bytes(3_200_000_000), "3.2 GB");
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
    fn days_from_civil_inverts_civil_from_days() {
        // The two halves of Hinnant's pair have to agree, or a date window
        // silently measures the wrong number of days.
        for days in [-25_000i64, -1, 0, 1, 10_957, 19_782, 19_783, 20_693, 60_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(
                days_from_civil(y, m, d),
                days,
                "round trip failed for {y}-{m:02}-{d:02}"
            );
        }
    }

    #[test]
    fn two_days_subtract_to_the_gap_between_them() {
        let june = Day::new(2026, 6, 2).days_since_epoch();
        let august = Day::new(2026, 8, 26).days_since_epoch();
        assert_eq!(august - june, 85);
        // Across a leap day, and across a year boundary.
        assert_eq!(
            Day::new(2024, 3, 1).days_since_epoch() - Day::new(2024, 2, 28).days_since_epoch(),
            2
        );
        assert_eq!(
            Day::new(2026, 1, 1).days_since_epoch() - Day::new(2025, 1, 1).days_since_epoch(),
            365
        );
    }

    #[test]
    fn the_epoch_is_day_zero() {
        assert_eq!(Day::new(1970, 1, 1).days_since_epoch(), 0);
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

    // ── The attachment rules (card D1) ──────────────────────────────────

    fn with_charts(ids: &[AttachmentId]) -> Song {
        let mut song = Song::new(1, "Bron-Yr-Aur Stomp", "Led Zeppelin");
        for id in ids {
            song.attach(*id);
        }
        song
    }

    #[test]
    fn the_first_chart_attached_becomes_the_primary_one() {
        let mut song = Song::new(1, "Carolina", "M. Ward");
        assert_eq!(song.primary(), None, "a song with no chart has no primary");

        song.attach(101);
        assert_eq!(song.primary_attachment, Some(101));

        // And nothing after it takes the place.
        song.attach(102);
        song.attach(103);
        assert_eq!(song.primary_attachment, Some(101));
        assert_eq!(song.attachments, vec![101, 102, 103]);
    }

    #[test]
    fn attaching_the_same_chart_twice_does_not_list_it_twice() {
        let mut song = with_charts(&[101, 102]);
        song.attach(101);
        assert_eq!(song.attachments, vec![101, 102]);
        assert_eq!(song.primary_attachment, Some(101));
    }

    #[test]
    fn removing_the_primary_chart_promotes_the_oldest_one_left() {
        let mut song = with_charts(&[101, 102, 103]);
        assert!(song.detach(101));
        assert_eq!(
            song.primary_attachment,
            Some(102),
            "the row directly under the card moves into it"
        );
        assert_eq!(song.attachments, vec![102, 103]);
    }

    #[test]
    fn removing_a_chart_that_is_not_primary_leaves_the_primary_alone() {
        let mut song = with_charts(&[101, 102, 103]);
        assert!(song.detach(103));
        assert_eq!(song.primary_attachment, Some(101));
        assert_eq!(song.attachments, vec![101, 102]);
    }

    #[test]
    fn removing_a_promoted_chart_promotes_again() {
        // The rule has to survive being applied twice: this is the sequence
        // that would leave a dangling pointer if `settle_primary` only ran on
        // the first removal.
        let mut song = with_charts(&[101, 102, 103]);
        song.detach(101);
        song.detach(102);
        assert_eq!(song.primary_attachment, Some(103));
        assert_eq!(song.primary(), Some(103));
    }

    #[test]
    fn removing_the_last_chart_leaves_no_primary() {
        let mut song = with_charts(&[101]);
        assert!(song.detach(101));
        assert!(song.attachments.is_empty());
        assert_eq!(song.primary_attachment, None, "and not a dangling id");
        assert_eq!(song.primary(), None);
        assert!(!song.has_chart());
    }

    #[test]
    fn removing_a_chart_the_song_never_had_changes_nothing() {
        let mut song = with_charts(&[101, 102]);
        let before = song.clone();
        assert!(!song.detach(999));
        assert_eq!(song, before);
    }

    #[test]
    fn set_primary_promotes_a_chart_the_song_already_has() {
        let mut song = with_charts(&[101, 102, 103]);
        assert!(song.set_primary(103));
        assert_eq!(song.primary(), Some(103));
        // The list order is the order they were added and does not move.
        assert_eq!(song.attachments, vec![101, 102, 103]);
    }

    #[test]
    fn set_primary_refuses_a_chart_that_is_not_attached() {
        let mut song = with_charts(&[101, 102]);
        assert!(!song.set_primary(999));
        assert_eq!(song.primary(), Some(101), "and did not point at nothing");
    }

    #[test]
    fn a_song_with_charts_always_reads_a_primary_even_if_the_field_is_wrong() {
        // A library edited behind the app's back: the stored pointer names a
        // chart this song does not have. The card renders the oldest one
        // rather than nothing at all.
        let mut song = with_charts(&[101, 102]);
        song.primary_attachment = Some(999);
        assert_eq!(song.primary(), Some(101));

        // And the next mutation writes the correct answer down.
        song.attach(103);
        assert_eq!(song.primary_attachment, Some(101));
    }

    #[test]
    fn a_song_with_charts_and_no_primary_recovers_one() {
        let mut song = with_charts(&[101, 102]);
        song.primary_attachment = None;
        assert_eq!(song.primary(), Some(101));
        song.detach(102);
        assert_eq!(song.primary_attachment, Some(101));
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

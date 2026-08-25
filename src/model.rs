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

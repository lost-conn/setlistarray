//! View state and settings, as one row.
//!
//! **Why a row and not a blob, and not an object per setting.** These values
//! are singular: there is exactly one library view and exactly one set of
//! settings, and nothing ever queries, relates or counts them. So a library of
//! objects — a `Setting` type with a key and a value — would be a lie about
//! their shape, and the card is right to reject it.
//!
//! The other candidate was one serialised blob: a single string field, or a
//! file next to the database. I picked a typed row instead, for three reasons.
//! A blob needs a format, and hand-rolling one (or taking a serde dependency
//! this app otherwise does not have) is a second serialisation layer to keep
//! honest alongside `convert.rs`. A row rides the durability, the open-failure
//! handling and — when card I1 lands — the backup that the rest of the library
//! already gets, where a loose file in `<data>/` would quietly be left out. And
//! rhypedb reconciles an added field on open, so tomorrow's setting is a line
//! in `schema.rhype` rather than a version tag and a parser branch.
//!
//! The one place it is not typed is the collapsed and expanded group lists.
//! Those are variable-length lists of user-facing labels, and the schema has no
//! list-of-scalar. They are joined with newlines rather than commas because a
//! group label is an artist, a tag or a tuning — all of which may contain a
//! comma, and none of which can contain a newline.

use rhypedb_engine::object::{FieldMap, Value};

use crate::capture::CaptureMode;
use crate::model::Confidence;
use crate::store::{
    AccentChoice, Density, DefaultTuning, Filters, GroupBy, PerformanceTheme, SortDir, SortField,
    ThemeChoice,
};

/// Everything `LibraryViewStore` and `SettingsStore` remember between launches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub query: String,
    pub group_by: GroupBy,
    pub sort_field: SortField,
    pub sort_dir: SortDir,
    pub density: Density,
    pub collapsed: Vec<String>,
    pub expanded: Vec<String>,
    /// The library's active filters (card G3) — see `crate::store::Filters`.
    pub filters: Filters,
    /// Light, Dark, or Follow system — card K8. See [`ThemeChoice`], and see
    /// [`from_fields`] for what a row written before this card becomes.
    pub theme: ThemeChoice,
    pub accent: AccentChoice,
    pub performance_theme: PerformanceTheme,
    pub keep_awake: bool,
    pub recheck_saved_pages: bool,
    /// Card H5 — the tuning `song_form` prefills a **new** song with. It is
    /// read exactly once, at the moment a song is created; changing it never
    /// touches a song that already exists. See [`crate::store::DefaultTuning`]
    /// for why the value is one of a fixed seven rather than free text.
    pub default_tuning: DefaultTuning,
    pub capture_mode: CaptureMode,
    /// When the last export finished, milliseconds since the epoch — card
    /// I1's export writes this the moment its zip is safely handed off, and
    /// card I3 is the screen that will read it. Built now because a row on
    /// this struct costs nothing today and would otherwise be I3's own
    /// migration to add later; the row I3 draws for it is not.
    pub last_export_at: Option<i64>,
}

impl Default for Preferences {
    /// A first run, and the fallback for any field that fails to read back.
    /// These are the same defaults the stores were built with.
    fn default() -> Self {
        Self {
            query: String::new(),
            group_by: GroupBy::Confidence,
            sort_field: SortField::Artist,
            sort_dir: SortDir::Asc,
            density: Density::Comfortable,
            collapsed: Vec::new(),
            expanded: Vec::new(),
            filters: Filters::default(),
            theme: ThemeChoice::default(),
            accent: AccentChoice::FromSystem,
            performance_theme: PerformanceTheme::FollowApp,
            keep_awake: true,
            recheck_saved_pages: false,
            default_tuning: DefaultTuning::default(),
            // Reader text, and `CaptureMode`'s own docs carry the measurement
            // that says so. `CaptureMode::default()` rather than the variant
            // spelled out, so the app's default and the engine's cannot drift.
            capture_mode: CaptureMode::default(),
            last_export_at: None,
        }
    }
}

const SEPARATOR: char = '\n';

fn join(labels: &[String]) -> String {
    labels.join("\n")
}

fn split(text: &str) -> Vec<String> {
    text.split(SEPARATOR)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

fn string(fields: &FieldMap, name: &str) -> Option<String> {
    match fields.get(name) {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

fn boolean(fields: &FieldMap, name: &str, fallback: bool) -> bool {
    match fields.get(name) {
        Some(Value::Bool(b)) => *b,
        _ => fallback,
    }
}

/// `None` covers both "never exported" and "the row is corrupt" the same way
/// every other optional field on this struct does — there is nothing a wrong
/// type here could mean other than absent.
fn datetime(fields: &FieldMap, name: &str) -> Option<i64> {
    match fields.get(name) {
        Some(Value::DateTime(ms)) => Some(*ms),
        _ => None,
    }
}

/// `system`, or the index into `theme::ACCENTS`.
fn accent_name(accent: AccentChoice) -> String {
    match accent {
        AccentChoice::FromSystem => "system".to_string(),
        AccentChoice::Named(i) => i.to_string(),
    }
}

fn accent_from(name: &str) -> AccentChoice {
    match name.parse::<usize>() {
        Ok(i) => AccentChoice::Named(i),
        Err(_) => AccentChoice::FromSystem,
    }
}

/// The theme choice, and the one field on this row that has to read two
/// shapes — card K8.
///
/// Before K8 the app had a two-state dark mode and stored it as
/// `dark_mode: Bool`. It has three states now (`ThemeChoice`), which a `Bool`
/// cannot hold, so the value moved to a new `theme: String` field and the old
/// one stayed in `schema.rhype` to be read exactly here and nowhere else.
///
/// ## What an existing install becomes, and why
///
/// **A stored `true` becomes `Dark`; a stored `false` becomes `Light`. Neither
/// becomes `FollowSystem`.** The temptation is to map `false` onto the new
/// default — "they never turned dark mode on, so they have no opinion" — and
/// it is wrong, because a stored `false` is not the absence of an answer. The
/// old row was written by `set_dark`, which had exactly one caller: the switch
/// on the Settings screen. A `false` in that field means somebody looked at a
/// switch and left it off, or turned it off, and mapping that to "follow the
/// system" would let a sunset turn their app dark on the strength of a
/// preference they had already expressed to the contrary. An upgrade must not
/// change how the app looks; that rule decides both arms.
///
/// **The absence of the field is what means `FollowSystem`**, and that falls
/// out of the same rule rather than being a second decision:
/// [`Preferences::default`] is the fallback for a row that has neither field,
/// which is a fresh install, and a fresh install has nobody's opinion to
/// preserve. `ThemeChoice::default`'s own comment argues why following the
/// system is the right thing to do with nobody's opinion.
///
/// The new field wins whenever it is present, so a library that has been
/// opened once by this version never consults the old one again — including
/// the case where somebody sets Follow system, which writes `"FollowSystem"`
/// over a `dark_mode` that is still sitting there saying `false`.
fn theme_from(fields: &FieldMap, fallback: ThemeChoice) -> ThemeChoice {
    if let Some(name) = string(fields, "theme") {
        if let Some(choice) = ThemeChoice::from_name(&name) {
            return choice;
        }
        // A `theme` this build does not recognise — a row from a future
        // version with a fourth option — falls through to the legacy bool
        // rather than straight to the default, because the bool is still the
        // more specific thing this install knows about the user.
    }
    match fields.get("dark_mode") {
        Some(Value::Bool(true)) => ThemeChoice::Dark,
        Some(Value::Bool(false)) => ThemeChoice::Light,
        _ => fallback,
    }
}

/// One selected confidence chip's stored name, `Unrated` included — the same
/// four words `crate::derive::group_songs`'s confidence buckets already use,
/// so a filter chip and a group header never name the same state two ways.
fn confidence_slot_name(confidence: Option<Confidence>) -> &'static str {
    match confidence {
        Some(Confidence::Solid) => "Solid",
        Some(Confidence::Rusty) => "Rusty",
        Some(Confidence::Learning) => "Learning",
        None => "Unrated",
    }
}

/// The inverse of [`confidence_slot_name`]. `None` here means "not one of the
/// four words" — a line corrupted or written by a future version — and the
/// caller drops it rather than guessing, the same as [`GroupBy::from_name`]
/// returning `None` for a stray value.
fn confidence_slot_from(name: &str) -> Option<Option<Confidence>> {
    match name {
        "Solid" => Some(Some(Confidence::Solid)),
        "Rusty" => Some(Some(Confidence::Rusty)),
        "Learning" => Some(Some(Confidence::Learning)),
        "Unrated" => Some(None),
        _ => None,
    }
}

pub fn to_fields(preferences: &Preferences) -> FieldMap {
    let mut fields = FieldMap::new();
    let mut put = |name: &str, value: Value| {
        fields.insert(name.to_string(), value);
    };
    put("query", Value::String(preferences.query.clone()));
    put("group_by", Value::String(preferences.group_by.name().into()));
    put(
        "sort_field",
        Value::String(preferences.sort_field.name().into()),
    );
    put("sort_dir", Value::String(preferences.sort_dir.name().into()));
    put("density", Value::String(preferences.density.name().into()));
    put("collapsed", Value::String(join(&preferences.collapsed)));
    put("expanded", Value::String(join(&preferences.expanded)));
    let confidence_names: Vec<String> = preferences
        .filters
        .confidences
        .iter()
        .map(|c| confidence_slot_name(*c).to_string())
        .collect();
    put("filter_confidences", Value::String(join(&confidence_names)));
    put("filter_tags", Value::String(join(&preferences.filters.tags)));
    put(
        "filter_tunings",
        Value::String(join(&preferences.filters.tunings)),
    );
    put(
        "filter_has_chart",
        Value::Bool(preferences.filters.has_chart),
    );
    put("theme", Value::String(preferences.theme.name().into()));
    put("accent", Value::String(accent_name(preferences.accent)));
    put(
        "performance_theme",
        Value::String(preferences.performance_theme.name().into()),
    );
    put("keep_awake", Value::Bool(preferences.keep_awake));
    put(
        "recheck_saved_pages",
        Value::Bool(preferences.recheck_saved_pages),
    );
    put(
        "default_tuning",
        Value::String(preferences.default_tuning.name().into()),
    );
    put(
        "capture_mode",
        Value::String(preferences.capture_mode.name().into()),
    );
    put_some(
        &mut fields,
        "last_export_at",
        preferences.last_export_at.map(Value::DateTime),
    );
    fields
}

/// The `Some`-or-absent counterpart to `put` — `to_fields` never writes a
/// `Null` for `last_export_at` on a library that has never exported, the
/// same way `song_fields` leaves an unset optional out entirely rather than
/// writing a placeholder.
fn put_some(fields: &mut FieldMap, name: &str, value: Option<Value>) {
    if let Some(value) = value {
        fields.insert(name.to_string(), value);
    }
}

/// Anything unreadable falls back to its default rather than failing the read:
/// a setting that cannot be understood is worth losing, a library is not.
pub fn from_fields(fields: &FieldMap) -> Preferences {
    let fallback = Preferences::default();
    Preferences {
        query: string(fields, "query").unwrap_or(fallback.query),
        group_by: string(fields, "group_by")
            .and_then(|n| GroupBy::from_name(&n))
            .unwrap_or(fallback.group_by),
        sort_field: string(fields, "sort_field")
            .and_then(|n| SortField::from_name(&n))
            .unwrap_or(fallback.sort_field),
        sort_dir: string(fields, "sort_dir")
            .and_then(|n| SortDir::from_name(&n))
            .unwrap_or(fallback.sort_dir),
        density: string(fields, "density")
            .and_then(|n| Density::from_name(&n))
            .unwrap_or(fallback.density),
        collapsed: string(fields, "collapsed")
            .map(|s| split(&s))
            .unwrap_or(fallback.collapsed),
        expanded: string(fields, "expanded")
            .map(|s| split(&s))
            .unwrap_or(fallback.expanded),
        filters: Filters {
            confidences: string(fields, "filter_confidences")
                .map(|s| split(&s).iter().filter_map(|n| confidence_slot_from(n)).collect())
                .unwrap_or(fallback.filters.confidences),
            tags: string(fields, "filter_tags")
                .map(|s| split(&s))
                .unwrap_or(fallback.filters.tags),
            tunings: string(fields, "filter_tunings")
                .map(|s| split(&s))
                .unwrap_or(fallback.filters.tunings),
            has_chart: boolean(fields, "filter_has_chart", fallback.filters.has_chart),
        },
        theme: theme_from(fields, fallback.theme),
        accent: string(fields, "accent")
            .map(|n| accent_from(&n))
            .unwrap_or(fallback.accent),
        performance_theme: string(fields, "performance_theme")
            .and_then(|n| PerformanceTheme::from_name(&n))
            .unwrap_or(fallback.performance_theme),
        keep_awake: boolean(fields, "keep_awake", fallback.keep_awake),
        recheck_saved_pages: boolean(
            fields,
            "recheck_saved_pages",
            fallback.recheck_saved_pages,
        ),
        // Absent — the field a database written before card H5 does not
        // have — falls back to `DefaultTuning::default()` the same way every
        // other field on this row does for a pre-existing library. That
        // fallback is what `an_empty_row_reads_back_as_the_defaults` below
        // pins down for the whole struct, this field included; G3's own
        // commit relied on the identical property when `filter_*` was added
        // to this row, and it still holds.
        default_tuning: string(fields, "default_tuning")
            .and_then(|n| DefaultTuning::from_name(&n))
            .unwrap_or(fallback.default_tuning),
        capture_mode: string(fields, "capture_mode")
            .and_then(|n| CaptureMode::from_name(&n))
            .unwrap_or(fallback.capture_mode),
        last_export_at: datetime(fields, "last_export_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled() -> Preferences {
        Preferences {
            query: "wheel".into(),
            group_by: GroupBy::Tuning,
            sort_field: SortField::LastPlayed,
            sort_dir: SortDir::Desc,
            density: Density::Compact,
            collapsed: vec!["Learning".into(), "Old Crow, Medicine Show".into()],
            expanded: vec!["Solid".into()],
            filters: Filters {
                confidences: vec![Some(Confidence::Solid), None],
                tags: vec!["campfire".into(), "Dylan, sort of".into()],
                tunings: vec!["Drop D".into()],
                has_chart: true,
            },
            // The non-default one, for the same reason `capture_mode` below
            // picks its non-default value.
            theme: ThemeChoice::Dark,
            accent: AccentChoice::Named(2),
            performance_theme: PerformanceTheme::AlwaysDark,
            keep_awake: false,
            recheck_saved_pages: true,
            // The non-default one, for the same reason `capture_mode` below
            // picks its non-default value.
            default_tuning: DefaultTuning::Dadgad,
            // The non-default one, so the round trip proves it is written and
            // read rather than falling back to the same answer twice.
            capture_mode: CaptureMode::FullPage,
            // The non-default one too — `Some`, not the `None` a library
            // that has never exported reads back as.
            last_export_at: Some(1_756_770_000_000),
        }
    }

    #[test]
    fn preferences_survive_the_round_trip() {
        assert_eq!(from_fields(&to_fields(&filled())), filled());
    }

    #[test]
    fn a_group_label_containing_a_comma_survives() {
        let back = from_fields(&to_fields(&filled()));
        assert_eq!(back.collapsed[1], "Old Crow, Medicine Show");
    }

    #[test]
    fn an_empty_row_reads_back_as_the_defaults() {
        assert_eq!(from_fields(&FieldMap::new()), Preferences::default());
    }

    /// Card K8's migration, both values, and the thing it must not do.
    ///
    /// A `FieldMap` with a `dark_mode` and no `theme` is exactly what a
    /// library written by any build before this card holds, and the promise
    /// is that opening it does not change how the app looks. The third
    /// assertion is the one that matters most and is the easiest to get
    /// wrong: `false` is a preference somebody expressed, not the absence of
    /// one, so it must not quietly become "follow the system" and start
    /// tracking sunset.
    #[test]
    fn a_row_from_before_the_theme_choice_existed_keeps_the_look_it_had() {
        let mut on = FieldMap::new();
        on.insert("dark_mode".into(), Value::Bool(true));
        assert_eq!(from_fields(&on).theme, ThemeChoice::Dark);

        let mut off = FieldMap::new();
        off.insert("dark_mode".into(), Value::Bool(false));
        assert_eq!(from_fields(&off).theme, ThemeChoice::Light);
        assert_ne!(
            from_fields(&off).theme,
            ThemeChoice::FollowSystem,
            "an install that had dark mode switched off must not start following the system"
        );
    }

    /// The other half: no `dark_mode` either, which is a fresh install and the
    /// only case with nobody's opinion to preserve.
    #[test]
    fn a_library_with_no_theme_field_at_all_follows_the_system() {
        assert_eq!(from_fields(&FieldMap::new()).theme, ThemeChoice::FollowSystem);
    }

    /// Once this build has written a `theme`, the legacy bool is dead — even
    /// while it is still sitting in the row saying the opposite, which is
    /// exactly the state an upgraded library is in the moment somebody taps
    /// "Follow system".
    #[test]
    fn the_new_field_wins_over_the_legacy_bool_that_is_still_beside_it() {
        let mut fields = FieldMap::new();
        fields.insert("dark_mode".into(), Value::Bool(false));
        fields.insert("theme".into(), Value::String("FollowSystem".into()));
        assert_eq!(from_fields(&fields).theme, ThemeChoice::FollowSystem);

        fields.insert("theme".into(), Value::String("Dark".into()));
        assert_eq!(from_fields(&fields).theme, ThemeChoice::Dark);
    }

    /// A `theme` from a future version with a fourth option falls back to what
    /// this build *does* know about the user — the legacy bool — rather than
    /// jumping straight to the shipped default, which would throw away a
    /// preference for no reason.
    #[test]
    fn an_unreadable_theme_name_falls_back_to_the_legacy_bool_before_the_default() {
        let mut fields = FieldMap::new();
        fields.insert("dark_mode".into(), Value::Bool(true));
        fields.insert("theme".into(), Value::String("Sepia".into()));
        assert_eq!(from_fields(&fields).theme, ThemeChoice::Dark);
    }

    /// And all three of the new field's names survive the round trip they are
    /// stored through, which the `filled()` fixture above only exercises for
    /// whichever one it happens to hold.
    #[test]
    fn every_theme_choice_survives_the_round_trip_through_the_row() {
        for choice in ThemeChoice::ALL {
            let preferences = Preferences {
                theme: choice,
                ..Default::default()
            };
            assert_eq!(from_fields(&to_fields(&preferences)).theme, choice);
        }
    }

    #[test]
    fn a_nonsense_value_falls_back_rather_than_failing() {
        let mut fields = to_fields(&filled());
        fields.insert("group_by".into(), Value::String("Kazoo".into()));
        fields.insert("accent".into(), Value::String("puce".into()));
        fields.insert("default_tuning".into(), Value::String("Nashville".into()));
        let back = from_fields(&fields);
        assert_eq!(back.group_by, GroupBy::Confidence);
        assert_eq!(back.accent, AccentChoice::FromSystem);
        assert_eq!(back.default_tuning, DefaultTuning::Standard);
        // Everything else is untouched.
        assert_eq!(back.density, Density::Compact);
    }

    /// The property card H5 leans on: a database written before this card —
    /// which is exactly what a `FieldMap` missing `default_tuning` models —
    /// still opens, reading the preference back as the shipped default
    /// (`DefaultTuning::Standard`) rather than failing to open at all. Also
    /// covered by `an_empty_row_reads_back_as_the_defaults` above, which pins
    /// the whole struct; this test names the one field the card asked about.
    #[test]
    fn a_row_from_before_default_tuning_existed_opens_and_reads_standard() {
        let fields = FieldMap::new();
        assert_eq!(from_fields(&fields).default_tuning, DefaultTuning::Standard);
    }

    /// The same property, for card I1's field: a library nobody has ever
    /// exported from — including every library written before this card —
    /// has no `last_export_at` key at all, and reads back `None` rather than
    /// `Some(0)` or a parse error.
    #[test]
    fn a_library_that_has_never_exported_reads_last_export_at_as_none() {
        assert_eq!(from_fields(&FieldMap::new()).last_export_at, None);
    }

    #[test]
    fn a_recorded_export_time_survives_the_round_trip() {
        let mut preferences = Preferences::default();
        preferences.last_export_at = Some(1_756_770_000_000);
        assert_eq!(
            from_fields(&to_fields(&preferences)).last_export_at,
            Some(1_756_770_000_000)
        );
    }

    #[test]
    fn no_collapsed_groups_reads_back_as_none_rather_than_one_empty_label() {
        let preferences = Preferences {
            collapsed: Vec::new(),
            ..Default::default()
        };
        assert!(from_fields(&to_fields(&preferences)).collapsed.is_empty());
    }

    #[test]
    fn a_filter_tag_containing_a_comma_survives() {
        let back = from_fields(&to_fields(&filled()));
        assert_eq!(back.filters.tags[1], "Dylan, sort of");
    }

    #[test]
    fn unrated_survives_the_round_trip_alongside_a_named_confidence() {
        let back = from_fields(&to_fields(&filled()));
        assert_eq!(
            back.filters.confidences,
            vec![Some(Confidence::Solid), None]
        );
    }

    #[test]
    fn a_corrupted_confidence_slot_is_dropped_rather_than_guessed_at() {
        let mut fields = to_fields(&filled());
        fields.insert(
            "filter_confidences".into(),
            Value::String("Solid\nKazoo\nUnrated".into()),
        );
        let back = from_fields(&fields);
        assert_eq!(
            back.filters.confidences,
            vec![Some(Confidence::Solid), None]
        );
    }

    #[test]
    fn no_filters_selected_reads_back_as_the_defaults() {
        let preferences = Preferences {
            filters: Filters::default(),
            ..Default::default()
        };
        assert_eq!(
            from_fields(&to_fields(&preferences)).filters,
            Filters::default()
        );
    }
}

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
    pub dark_mode: bool,
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
            dark_mode: false,
            accent: AccentChoice::FromSystem,
            performance_theme: PerformanceTheme::FollowApp,
            keep_awake: true,
            recheck_saved_pages: false,
            default_tuning: DefaultTuning::default(),
            // Reader text, and `CaptureMode`'s own docs carry the measurement
            // that says so. `CaptureMode::default()` rather than the variant
            // spelled out, so the app's default and the engine's cannot drift.
            capture_mode: CaptureMode::default(),
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
    put("dark_mode", Value::Bool(preferences.dark_mode));
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
    fields
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
        dark_mode: boolean(fields, "dark_mode", fallback.dark_mode),
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
            dark_mode: true,
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

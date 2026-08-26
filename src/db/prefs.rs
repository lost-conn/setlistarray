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

use crate::store::{AccentChoice, Density, GroupBy, PerformanceTheme, SortDir, SortField};

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
    pub dark_mode: bool,
    pub accent: AccentChoice,
    pub performance_theme: PerformanceTheme,
    pub keep_awake: bool,
    pub recheck_saved_pages: bool,
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
            dark_mode: false,
            accent: AccentChoice::FromSystem,
            performance_theme: PerformanceTheme::FollowApp,
            keep_awake: true,
            recheck_saved_pages: false,
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
            dark_mode: true,
            accent: AccentChoice::Named(2),
            performance_theme: PerformanceTheme::AlwaysDark,
            keep_awake: false,
            recheck_saved_pages: true,
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
        let back = from_fields(&fields);
        assert_eq!(back.group_by, GroupBy::Confidence);
        assert_eq!(back.accent, AccentChoice::FromSystem);
        // Everything else is untouched.
        assert_eq!(back.density, Density::Compact);
    }

    #[test]
    fn no_collapsed_groups_reads_back_as_none_rather_than_one_empty_label() {
        let preferences = Preferences {
            collapsed: Vec::new(),
            ..Default::default()
        };
        assert!(from_fields(&to_fields(&preferences)).collapsed.is_empty());
    }
}

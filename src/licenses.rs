//! What SetListArray is licensed under, and what everything it is built from
//! is licensed under — the data behind `screens::licenses`.
//!
//! Two obligations meet here, and they point in opposite directions.
//!
//! **The app's own.** SetListArray is GPL-3.0-or-later (`LICENSE`, and the
//! README's License section). GPLv3 asks a program with an interactive user
//! interface to display "Appropriate Legal Notices": a copyright notice, a
//! statement that there is no warranty, that the work may be conveyed under
//! the license, and how to view a copy of it. [`COPYRIGHT`], [`NOTICE`] and
//! [`GPL_TEXT`] are those, and the screen shows all three. [`SOURCE`] is there
//! because a license that turns on the source being available ought to say
//! where it is.
//!
//! **Everyone else's.** MIT, BSD and Apache each require that their copyright
//! notice and permission text go with every copy of the software, binary
//! copies included, and an APK is a binary copy of a few hundred crates. Until
//! this module existed it carried none of their notices. [`NOTICES`] is all of
//! them, generated rather than written — see `scripts/make-notices.py` for how,
//! and for why the result is Rust source in `licenses/third_party.rs` rather
//! than a text file beside the fonts.

mod third_party;

pub use third_party::NOTICES;

/// One license text, and every crate that ships exactly that text.
///
/// "Exactly" up to whitespace: the generator groups on the text with its
/// whitespace collapsed, so two copies of the Apache license wrapped at
/// different widths are one notice, while two MIT licenses naming different
/// copyright holders are two — which is the difference that matters, since the
/// copyright line is the part of an MIT notice that has to be reproduced.
#[derive(Debug, PartialEq, Eq)]
pub struct Notice {
    /// The license's name as cargo-about reports it, e.g. "MIT License".
    pub license: &'static str,
    /// Its SPDX identifier, e.g. `MIT`.
    pub spdx: &'static str,
    /// `name version` of every crate shipping this text, sorted.
    pub crates: &'static [&'static str],
    /// The text as those crates ship it, line breaks and all.
    pub text: &'static str,
}

/// What `Cargo.toml` says this crate is under, as the row in Settings shows it.
pub const APP_LICENSE: &str = "GPL-3.0-or-later";

/// The copyright notice GPLv3's "Appropriate Legal Notices" begins with. The
/// name is the one this repository's commits are made under.
pub const COPYRIGHT: &str = "Copyright © 2026 Lost Connection";

/// The FSF's own wording for a GPLv3-or-later program, the same paragraph the
/// README opens its License section with: what may be done with it, under
/// which license, and that it comes with no warranty.
pub const NOTICE: &str = "SetListArray is free software: you can redistribute it and/or modify it \
    under the terms of the GNU General Public License as published by the Free Software \
    Foundation, either version 3 of the License, or (at your option) any later version. It is \
    distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the \
    implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.";

/// Where the source is, without a scheme: the app has no browser to hand a
/// link to, so this is something to read and type, not to tap.
pub const SOURCE: &str = "github.com/lost-conn/setlistarray";

/// The license itself, from the same `LICENSE` file at the repository root
/// that GitHub and `cargo` read.
pub const GPL_TEXT: &str = include_str!("../LICENSE");

/// A license text as paragraphs a phone can wrap.
///
/// Every license file in the tree is hard-wrapped at around 80 columns,
/// because it was written for a terminal. Put on a 360dp-wide screen as it is,
/// each of those lines either runs off the side or breaks a second time part
/// way along, and a page of legal text becomes a page of ragged fragments. So
/// a blank line is kept as the paragraph break it is, and the line breaks
/// inside a paragraph are not: their lines are joined with single spaces and
/// left to the layout to wrap. Nothing is dropped — every word of the original
/// is in the output, in order — and that is the property the test holds.
pub fn paragraphs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(line);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// The line under a closed notice: who ships it, by name, the first three and
/// a count of the rest. Versions are left for the open row — `png 0.17.16` and
/// `png 0.18.0` are one crate to somebody scanning a list, so a name is only
/// counted once.
pub fn used_by_summary(crates: &[&str]) -> String {
    let mut names: Vec<&str> = crates
        .iter()
        .map(|c| c.split(' ').next().unwrap_or(c))
        .collect();
    names.dedup();
    match names.len() {
        0..=3 => names.join(", "),
        n => format!("{} and {} more", names[..3].join(", "), n - 3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `name version` for every `[[package]]` in `Cargo.lock`, sorted — the
    /// same list `make-notices.py` records, read the same way, by hand rather
    /// than with a TOML parser this crate does not otherwise need.
    fn locked_packages() -> Vec<String> {
        let mut out = Vec::new();
        let mut name = None;
        for line in include_str!("../Cargo.lock").lines() {
            if line == "[[package]]" {
                name = None;
            } else if let Some(n) = line.strip_prefix("name = \"") {
                name = Some(n.trim_end_matches('"'));
            } else if let Some(v) = line.strip_prefix("version = \"") {
                if let Some(n) = name.take() {
                    out.push(format!("{n} {}", v.trim_end_matches('"')));
                }
            }
        }
        out.sort();
        out
    }

    /// The notices are generated, and a generated file goes stale the moment
    /// what it was generated from moves. `Cargo.lock` is that thing: nobody
    /// adds, removes or bumps a dependency without it changing. So the
    /// generator records the lockfile it read, and this holds the record
    /// against the real one — the failure it prevents is a new crate shipping
    /// in the APK with its notice missing from the screen, which nothing else
    /// would ever mention.
    #[test]
    fn the_notices_were_generated_from_this_cargo_lock() {
        let locked = locked_packages();
        assert!(
            locked.len() > 300,
            "read only {} packages out of Cargo.lock; the reader above has stopped matching \
             its format, and this test's silence would mean nothing",
            locked.len()
        );
        let recorded: Vec<String> = third_party::GENERATED_FROM
            .iter()
            .map(|p| p.to_string())
            .collect();
        if locked != recorded {
            let added: Vec<&String> = locked.iter().filter(|p| !recorded.contains(p)).collect();
            let gone: Vec<&String> = recorded.iter().filter(|p| !locked.contains(p)).collect();
            panic!(
                "Cargo.lock has changed since src/licenses/third_party.rs was generated \
                 (new: {added:?}; gone: {gone:?}). Re-run `python3 scripts/make-notices.py` \
                 so the Licenses screen names every crate the app is built from."
            );
        }
    }

    /// SetListArray's license is the screen's first section; listed again
    /// among the third-party notices, it would read as a dependency of itself.
    #[test]
    fn the_app_is_not_one_of_its_own_third_party_notices() {
        for notice in NOTICES {
            assert!(
                !notice.crates.iter().any(|c| c.starts_with("setlistarray ")),
                "SetListArray is listed under {} as a third-party crate",
                notice.license
            );
        }
    }

    #[test]
    fn every_notice_says_who_ships_it_and_what_it_says() {
        assert!(NOTICES.len() > 50, "only {} notices; the generator has lost most of them", NOTICES.len());
        for notice in NOTICES {
            assert!(!notice.crates.is_empty(), "{} has no crate shipping it", notice.license);
            assert!(!notice.text.trim().is_empty(), "{} has no text", notice.license);
        }
    }

    /// The three places that say what this app is under have to agree:
    /// `Cargo.toml`, the `LICENSE` file the screen shows, and the row in
    /// Settings. A relicense that updated one and not the others would put
    /// the wrong text in front of the person the notice is for.
    #[test]
    fn the_license_shown_is_the_license_the_crate_declares() {
        assert!(
            include_str!("../Cargo.toml").contains(&format!("license = \"{APP_LICENSE}\"")),
            "Cargo.toml's license is not {APP_LICENSE}"
        );
        assert!(
            GPL_TEXT.contains("GNU GENERAL PUBLIC LICENSE")
                && GPL_TEXT.contains("Version 3, 29 June 2007"),
            "LICENSE is not the GPLv3 text"
        );
    }

    #[test]
    fn paragraphs_keep_every_word_and_only_the_blank_line_breaks() {
        let text = "  Copyright (c) 2026 Somebody\n\nPermission is hereby granted,\nfree of \
                    charge, to any person\n   \nTHE SOFTWARE IS PROVIDED \"AS IS\"\n";
        assert_eq!(
            paragraphs(text),
            vec![
                "Copyright (c) 2026 Somebody",
                "Permission is hereby granted, free of charge, to any person",
                "THE SOFTWARE IS PROVIDED \"AS IS\"",
            ]
        );
        for notice in NOTICES.iter().chain([&Notice { license: "", spdx: "", crates: &[], text: GPL_TEXT }]) {
            let before: Vec<&str> = notice.text.split_whitespace().collect();
            let after: Vec<String> = paragraphs(notice.text);
            let after: Vec<&str> = after.iter().flat_map(|p| p.split_whitespace()).collect();
            assert_eq!(before, after, "reflowing {} lost or moved a word", notice.license);
        }
    }

    #[test]
    fn a_long_list_of_crates_is_summarised_by_name() {
        assert_eq!(used_by_summary(&["a 1", "b 2"]), "a, b");
        assert_eq!(used_by_summary(&["png 0.17.16", "png 0.18.0", "zip 8.0.0"]), "png, zip");
        assert_eq!(used_by_summary(&["a 1", "b 1", "c 1", "d 1", "e 1"]), "a, b, c and 2 more");
    }
}

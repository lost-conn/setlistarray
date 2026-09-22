//! What holds the landing page to the app it is advertising.
//!
//! `site/` is HTML and CSS, and nothing in a Rust build has any reason to look
//! at it — which is exactly why these tests exist. The page repeats things
//! that are defined elsewhere in this repository: the theme's colours, the
//! shape the screenshot compositor emits, the names of the font subsets, the
//! ids of the screens themselves. Every one of those is a pair of files in two
//! languages joined only by a string, and this repository's habit with that
//! shape of coupling is to make a mismatch fail the build rather than wait for
//! somebody to notice it on a page nobody reopens.
//!
//! It is the same job `the_listing_has_a_caption_for_every_shot_and_no_others`
//! does for the Play listing, and the reason is the same one that card learned:
//! a rename on one side is invisible until the result is in front of the
//! public.
//!
//! Everything here is `include_str!`, so the tests read the files as the
//! repository has them and never depend on a working directory.

use crate::theme::{DARK_NEUTRALS, LIGHT_NEUTRALS, RUST};

const STYLE: &str = include_str!("../site/style.css");
const INDEX: &str = include_str!("../site/index.html");
const FRAME_PY: &str = include_str!("../scripts/store-frame.py");
const FONTS_PY: &str = include_str!("../scripts/site-fonts.py");
const SHOTS_JSON: &str = include_str!("../store/shots.json");

/// The `--name: value` pairs in a block of CSS declarations, custom properties
/// only.
///
/// Both sides of the colour comparison are parsed with this rather than one
/// being matched against the other as a substring, because `theme.rs` writes
/// `--sla-paper: #FBF7F0;` with one space and a stylesheet is entitled to any
/// amount of whitespace it likes. Comparing parsed pairs means the test is
/// about the values and not about the formatting.
fn declarations(block: &str) -> Vec<(String, String)> {
    block
        .split(';')
        .filter_map(|decl| decl.split_once(':'))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .filter(|(name, _)| name.starts_with("--"))
        .collect()
}

/// The body of the first `:root{…}` block at or after `from`.
fn root_block(css: &str, from: usize) -> &str {
    let open = css[from..].find(":root{").expect("site/style.css declares a :root block") + from;
    let start = open + ":root{".len();
    let end = css[start..].find('}').expect("the :root block is closed") + start;
    &css[start..end]
}

/// A quoted attribute's values, for every occurrence of `attr` in `html`.
fn attribute_values<'a>(html: &'a str, attr: &str) -> Vec<&'a str> {
    let needle = format!("{attr}=\"");
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find(&needle) {
        let value = &rest[at + needle.len()..];
        let end = value.find('"').expect("an attribute's quote is closed");
        out.push(&value[..end]);
        rest = &value[end..];
    }
    out
}

/// True if a subresource reference would leave this origin.
///
/// A scheme, or a protocol-relative `//host/path`. `data:` would not leave the
/// origin, but the page has none and one appearing would be worth a second
/// look anyway, so it is not special-cased.
fn goes_off_origin(value: &str) -> bool {
    value.contains("://") || value.starts_with("//")
}

/// The value of a `NAME = "…"` assignment in one of the site's Python scripts.
fn python_string(source: &str, name: &str) -> String {
    let at = source
        .find(&format!("\n{name} = \""))
        .unwrap_or_else(|| panic!("{name} is assigned a string literal at the top level"));
    let value = &source[at + format!("\n{name} = \"").len()..];
    value[..value.find('"').expect("the literal is closed")].to_string()
}

#[test]
fn the_site_css_repeats_the_theme_tokens_exactly() {
    // `tokens()` in theme.rs composes the neutrals and the accent into one
    // declaration block at runtime. The page cannot call it — it is a
    // stylesheet — so it copies the values, and this is what stops the copy
    // rotting. Only the accent this app ships as its default is checked, and
    // that is deliberate: the app lets a phone pick its own accent from the
    // wallpaper, a web page has nobody to ask, and the store pictures are
    // pinned to Rust for the same reason.
    let dark_at = STYLE
        .find("@media (prefers-color-scheme: dark)")
        .expect("site/style.css has a dark-mode block");

    for (mode, block, neutrals, accent) in [
        (
            "light",
            root_block(STYLE, 0),
            LIGHT_NEUTRALS,
            [
                ("--sla-accent", RUST.base),
                ("--sla-accent-tint", RUST.tint),
                ("--sla-accent-on-tint", RUST.on_tint),
                ("--sla-on-accent", RUST.on_accent),
                ("--sla-accent-dim", RUST.dim),
            ],
        ),
        (
            "dark",
            root_block(STYLE, dark_at),
            DARK_NEUTRALS,
            [
                ("--sla-accent", RUST.base_dark),
                ("--sla-accent-tint", RUST.tint_dark),
                ("--sla-accent-on-tint", RUST.on_tint_dark),
                ("--sla-on-accent", RUST.on_accent_dark),
                ("--sla-accent-dim", RUST.dim_dark),
            ],
        ),
    ] {
        let from_theme = declarations(neutrals);
        let shared: Vec<(String, String)> = declarations(block)
            .into_iter()
            .filter(|(name, _)| name.starts_with("--sla-"))
            .collect();

        assert!(
            shared.len() >= 6,
            "site/style.css's {mode} :root declares only {} of the app's tokens. Either the \
             page stopped sharing theme.rs's vocabulary — the point of naming them --sla-* — \
             or this test is reading the wrong block.",
            shared.len()
        );

        for (name, value) in shared {
            let expected = from_theme
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
                .or_else(|| {
                    accent.iter().find(|(n, _)| *n == name).map(|(_, v)| (*v).to_string())
                });

            let Some(expected) = expected else {
                panic!(
                    "site/style.css declares {name} in its {mode} block and src/theme.rs has no \
                     such token. A page token that only looks like an app token is worse than \
                     one that does not: it reads as shared and is not."
                );
            };

            assert_eq!(
                value.to_ascii_uppercase(),
                expected.to_ascii_uppercase(),
                "{name} is {value} in site/style.css's {mode} block and {expected} in \
                 src/theme.rs. These are one colour written in two files; move both."
            );
        }
    }
}

#[test]
fn the_site_asks_nothing_of_anybody_else() {
    // The last line of the receipt on the page says it makes no third-party
    // requests, and the fonts are subset and self-hosted rather than fetched
    // from Google precisely so that it can. A claim on a page is worth what
    // the thing enforcing it is worth, and this is the thing enforcing it.
    //
    // Links are not requests. An <a href> to the repository or to Play is the
    // reader choosing to go somewhere, which is the opposite of the page
    // quietly telling a third party that they visited it — so only
    // subresources are checked: anything the browser fetches without being
    // asked.
    let mut offenders: Vec<String> = Vec::new();

    for value in attribute_values(INDEX, "src") {
        if goes_off_origin(value) {
            offenders.push(format!("site/index.html  src=\"{value}\""));
        }
    }

    for tag in INDEX.split("<link").skip(1) {
        let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
        for value in attribute_values(tag, "href") {
            if goes_off_origin(value) {
                offenders.push(format!("site/index.html  <link href=\"{value}\">"));
            }
        }
    }

    let mut rest = STYLE;
    while let Some(at) = rest.find("url(") {
        let value = &rest[at + "url(".len()..];
        let end = value.find(')').expect("a url() is closed");
        let url = value[..end].trim_matches(['\'', '"']);
        if goes_off_origin(url) {
            offenders.push(format!("site/style.css  url({url})"));
        }
        rest = &value[end..];
    }

    assert!(
        offenders.is_empty(),
        "the landing page would fetch something from another origin, and it prints \"no \
         third-party requests\" at the bottom of its own receipt:\n  {}\n\
         Serve the file from site/ instead — that is what scripts/site-fonts.py exists for.",
        offenders.join("\n  ")
    );
}

#[test]
fn the_site_reserves_the_shape_the_compositor_emits() {
    // site/style.css cannot measure a picture it will never be shipped: the
    // plates are built by the deploy and never committed, so the stylesheet
    // declares the aspect ratio and the browser holds the space open. If the
    // compositor's idea of that shape and the stylesheet's ever part company,
    // every section below the fold jumps when an image lands — a fault that is
    // invisible on the machine that built the page, because there the image is
    // already in cache.
    let at = FRAME_PY
        .find("SITE_PLATE_ASPECT = (")
        .expect("scripts/store-frame.py declares SITE_PLATE_ASPECT");
    let tuple = &FRAME_PY[at + "SITE_PLATE_ASPECT = (".len()..];
    let tuple = &tuple[..tuple.find(')').expect("the tuple is closed")];
    let (width, height) = tuple.split_once(',').expect("SITE_PLATE_ASPECT is a pair");
    let (width, height) = (width.trim(), height.trim());

    let declared = format!("aspect-ratio:{width}/{height}");
    assert!(
        STYLE.contains(&declared),
        "scripts/store-frame.py emits plates at {width}x{height} and site/style.css does not \
         declare `{declared}` on `.plate img`. The compositor refuses a capture that is not \
         that shape, so the two numbers are the same decision written twice."
    );
}

#[test]
fn the_site_serves_the_slices_the_subsetter_cuts() {
    // Each face is cut into a `base` and an `ext` slice and declared against a
    // matching `unicode-range`. Get the range wrong on the stylesheet side and
    // the failure is not an error — it is a visitor silently downloading the
    // 129 KB of accented glyphs this page does not use, or, in the other
    // direction, an em dash coming down as tofu.
    for constant in ["EXT_RANGE", "BASE_RANGE"] {
        let range = python_string(FONTS_PY, constant);
        assert!(
            STYLE.contains(&format!("unicode-range:{range}")),
            "scripts/site-fonts.py cuts {constant} as `{range}` and site/style.css does not \
             declare `unicode-range:{range}`. The slice and the range that selects it are one \
             decision in two files."
        );
    }

    // And every file the subsetter writes is a file the stylesheet asks for.
    // The stems live in FACES as the second element of each tuple; reading
    // them out of the source is uglier than importing them and is the only way
    // a Rust test can see a Python list.
    for line in FONTS_PY.lines() {
        let line = line.trim();
        if !line.starts_with('(') || !line.contains(".ttf\", \"") {
            continue;
        }
        let stem = line.split(".ttf\", \"").nth(1).and_then(|r| r.split('"').next());
        let stem = stem.expect("a FACES row names the stem its slices are written under");
        for slice in ["base", "ext"] {
            let file = format!("fonts/{stem}-{slice}.woff2");
            assert!(
                STYLE.contains(&file),
                "scripts/site-fonts.py writes {file} and no @font-face in site/style.css \
                 loads it. An unreferenced subset is bytes on the server nothing will fetch."
            );
        }
    }
}

#[test]
fn every_picture_the_page_shows_is_a_shot_the_tour_photographs() {
    // The page names its pictures `img/<shot id>.webp`, and those ids are the
    // ones in `src/shots.rs`'s tour by way of `store/shots.json`. Rename a
    // screen on either side and the page keeps a hole where a screenshot was —
    // and keeps it silently, because the deploy that builds the pictures has
    // no idea which of them anybody references.
    let listing: serde_json::Value =
        serde_json::from_str(SHOTS_JSON).expect("store/shots.json parses as JSON");
    let photographed = listing
        .get("shots")
        .and_then(serde_json::Value::as_object)
        .expect("store/shots.json holds its shots under a `shots` object");

    let mut shown = Vec::new();
    for value in attribute_values(INDEX, "src") {
        if let Some(name) = value.strip_prefix("img/").and_then(|n| n.strip_suffix(".webp")) {
            shown.push(name.to_string());
        }
    }

    assert!(
        !shown.is_empty(),
        "site/index.html shows no img/<id>.webp at all; either the page stopped using the \
         tour's screenshots or this test is looking for the wrong filenames"
    );

    let missing: Vec<&String> = shown.iter().filter(|id| !photographed.contains_key(*id)).collect();
    assert!(
        missing.is_empty(),
        "site/index.html asks for pictures of screens the tour never photographs: {missing:?}\n\
         The names come from `shots` in store/shots.json, which is itself held to \
         src/shots.rs's table by the_listing_has_a_caption_for_every_shot_and_no_others."
    );
}

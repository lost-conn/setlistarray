# Handoff: SetListArray — Android app (Rinch)

## Overview

SetListArray is an offline-first Android app for hobbyist musicians to keep track of songs they know how to play, attach charts to them (PDFs, offline-captured webpages, or typed text), and organize them into setlists. Target framework: **[Rinch](https://github.com/joeleaver/rinch)** (Rust GUI, HTML/CSS layout via Stylo + Taffy, signals-based reactivity, Mantine-inspired component library, CSS-variable theme system).

Core premise from the product owner: *"I'm always forgetting what songs I know how to play, and would like a centralized way to keep track."* Everything works with no network connection. No account, no sync, nothing uploaded.

## About the Design Files

The files in this bundle are **design references created in HTML** — prototypes showing intended look, layout, and behavior. They are **not production code to copy**. The task is to recreate these designs in Rinch using its own `rsx!` markup, component library, and theme system.

Rinch takes HTML/CSS-shaped input, so much of the layout translates fairly directly (flexbox, gaps, border-radius, colors). But structure the implementation as Rinch components with signals and stores — do not transliterate the HTML.

Two files:

- `SetListArray Hi-fi.dc.html` — **the authority on visuals.** Library, song detail, setlist detail, in light and dark, plus the accent-token panel.
- `SetListArray Wireframes.dc.html` — **the authority on flow and on screens not yet in hi-fi.** Turn 2 (ids `2a`–`2e`) is the agreed direction; turn 1 (`1a`–`1r`) is earlier exploration kept for reference. Where turn 1 and turn 2 disagree, turn 2 wins.

## Fidelity

**Mixed, and it matters which is which:**

- **High-fidelity** — Library, Song detail, Setlist detail. Final colors, typography, spacing. Recreate these precisely.
- **Low-fidelity (wireframe)** — Add/edit song, Attachment viewer, Add-webpage capture, Setlists list, Add-to-setlist sheet, Performance view, Sort/group sheet, Settings, First run. Structure and flow are decided; apply the hi-fi token set to style them. Where a wireframe and the hi-fi language conflict, follow the hi-fi language.

---

## Design Tokens

### Type

Two families, both on Google Fonts.

| Role | Family | Usage |
| --- | --- | --- |
| Display / song titles | **Newsreader** (serif, weights 400/500/600) | Song titles, setlist names, screen titles |
| UI / everything functional | **Karla** (weights 400/500/600/700) | Labels, metadata, buttons, chips, nav |

Type scale as used:

| Token | Value | Where |
| --- | --- | --- |
| screen-title | Newsreader 400, 34px, line-height 1.0, letter-spacing −0.01em | "Songs" |
| detail-title | Newsreader 500, 32px / 1.12, −0.015em | Song detail title |
| setlist-title | Newsreader 500, 30px / 1.12, −0.015em | Setlist detail title |
| row-title | Newsreader 500, 18px / 1.25 | Song name in any list row |
| body | Karla 400/500, 15px | Attachment rows, buttons |
| meta | Karla 400, 13px / 1.4 | Artist · key · tempo lines |
| meta-small | Karla 400/500, 12px | Counts, cumulative times, captions |
| label-caps | Karla 500, 12px, letter-spacing 0.16em, uppercase | Group headers ("Solid", "Rusty") |
| section-caps | Karla 600, 12px, letter-spacing 0.10em, uppercase | "Before you start" |
| status-bar | Karla 600, 13px | Clock |
| button | Karla 600, 15–16px | Primary buttons |
| chip | Karla 500, 13px | Filter/sort chips, metadata chips |
| nav-label | Karla 500/600, 11px, letter-spacing 0.04em | Bottom nav |

### Color — light

| Token | Hex | Notes |
| --- | --- | --- |
| paper (surface) | `#FBF7F0` | Screen background |
| card | `#FFFFFF` | Elevated attachment preview |
| fill | `#F1E9DC` | Search field, chips, metadata chips, icon buttons |
| fill-2 | `#F6F0E6` | Empty attachment thumb |
| hairline | `#E7DFD4` | Section dividers, nav top border |
| hairline-soft | `#EFE8DD` | Between list rows |
| swatch-mid | `#C4B8A9` | Ramp only, not text |
| muted | `#6E645A` | **All secondary/tertiary text.** 4.6:1 on paper |
| ink-2 | `#4A423B` | Chip text, emphasis inside muted rows |
| ink | `#1C1917` | Primary text, dark buttons |
| tab-bar-text | `#E4DACB` | Skeleton bars in previews (non-text) |

### Color — dark

| Token | Hex | Notes |
| --- | --- | --- |
| paper | `#181512` | Warm, not neutral black |
| card | `#211C18` | Attachment preview |
| fill | `#241F1A` | Search field, chips, thumbs |
| hairline | `#2C2620` | Section dividers |
| hairline-soft | `#241F1A` | Between rows |
| muted | `#9B9188` | All secondary/tertiary text; 5.9:1 |
| ink-2 | `#D6CCC1` | Chip text |
| ink | `#F5EFE6` | Primary text, light buttons |
| skeleton | `#2E2822` / `#42392F` | Preview bars |

The neutrals are fixed in both modes. They carry the identity. Mode follows the Android system setting.

### Accent — one token, user- or system-supplied

Accent appears in exactly five places: **group label, list link ("Show 38 more"), FAB, active nav item, primary button.** Nothing else is tinted.

Resolution order:

1. User-picked accent, if set in Settings.
2. Otherwise Material You primary extracted from the wallpaper, **darkened until it clears 4.5:1 against paper**.
3. Otherwise Rust (default).

Derived per accent:

- `accent` — light mode base. Default Rust `#B54724`.
- `accent-dark` — dark mode variant, lightened for contrast. Default `#E8845C`.
- `accent-tint` — accent at ~12% over paper, for selected metadata chips. Default light `#F7E6DE`, dark `#3A2318`.
- `accent-on-tint` — readable accent-family text on the tint. Default light `#8E3419`, dark `#F0A483`.
- `on-accent` — always paper (`#FBF7F0` light, `#1A1310` dark for the lighter accent).
- `accent-dim` — accent desaturated for "rusty" confidence dots. Default light `#D8B4A2`, dark `#8B5540`.

Alternate accents shown as proof the base holds: Pine `#2F6F4E`, Indigo `#3D5A9E`, Plum `#7A4C86`.

**Contrast rule the design commits to:** every text token clears 4.5:1 against its own background. Muted is `#6E645A` on paper and `#9B9188` on dark for exactly this reason — do not lighten either.

### Spacing, radius, elevation

- Screen horizontal padding: **22px**.
- Vertical rhythm inside lists: rows are `padding: 11px 0` with a 1px hairline-soft bottom border.
- Gaps: 6–7px between chips, 13px between a row thumb and its text, 10px between footer buttons.
- Radius: 10px (thumbnails) · 12px (info panels) · 14px (search field, secondary buttons) · 16px (card, primary Play button) · 20px (FAB) · 999px (filter/sort chips) · 34px (device frame corner, reference only).
- Shadows: card `0 2px 10px -4px rgba(28,25,23,.14), 0 0 0 1px rgba(28,25,23,.06)`; FAB `0 8px 18px -4px <accent at 50%>`. In dark, drop the tint shadow and use a 1px `rgba(255,255,255,.05)` hairline instead.
- Touch targets: icon buttons 40×40, FAB 60×60, primary buttons 48–52 tall. Nothing below 44.

### Icons

Rinch ships 5,000+ Tabler Icons with a type-safe enum API — **use those**, do not port the inline SVGs from the prototype. Icons needed: search, settings (gear), plus, chevron-left, chevron-down, chevron-right, pencil, dots-vertical, play, maximize/expand, alert-circle, music note (nav), list (nav), drag handle (grip-vertical), checkbox, sun (keep-awake).

Icon sizing: 16–19px inside 40×40 buttons; 21px in nav; 24px in FAB. Stroke weight 1.8–1.9 for outline icons.

---

## Screens

### 1. Library (Songs tab) — HI-FI

The app's home. Answers "what can I actually play?"

**Structure, top to bottom:**

1. Status bar area, 44px.
2. Title block, `padding: 6px 22px 14px`, flex row aligned to baseline: left is "Songs" (screen-title) with a sub-line beneath — `300 in your book · <accent, 600>41 solid</accent>` (meta). Right is a 40×40 circular gear button, `background: fill`, opening Settings. **Settings has no nav item; it lives in this gear and in the Setlists header gear.**
3. Search field: `fill`, radius 14, `padding: 11px 14px`, search icon + placeholder "Search title, artist, tag…" in muted 15px.
4. Chip row, wrapping, 7px gaps, all radius 999:
   - `Group: Confidence` — active, `background: ink`, `color: paper`.
   - `Artist ↑` — active sort, `background: fill`, `color: ink-2`. Label shows the current field plus direction arrow.
   - `Filter` — `background: fill`, `color: muted`.
   - `≣` — density toggle, same treatment, opens/toggles compact rows.
5. Grouped list, scrolling. Per group: a header row — label-caps group name (the **first/best group is accent-colored**, subsequent groups muted), the count in meta-small muted, then a flex-1 1px hairline. Groups in order: **Solid · Rusty · Learning · Unrated**. Each group collapses.
6. Song rows: 38×38 radius-10 thumb (`fill` background, 9px 600 uppercase muted label `PDF` / `WEB` / `TXT`; if the song has no attachment, `fill-2` with a 1px dashed `#DDD3C4` border and no label) · title (row-title) with a meta line under it (`Artist · key · tempo`, or `Artist · played March` when last-played is more informative) · a confidence indicator on the right: three 6px dots, filled `accent` for solid, `accent-dim` for rusty, `hairline` for empty.
7. Per-group truncation: `Show 38 more` in accent 500 13px.
8. FAB: 60×60, radius 20, `background: accent`, plus icon in `on-accent`, at `right: 20px; bottom: 16px`. Creates a song.
9. Bottom nav: 2 equal columns, 1px hairline top border, `padding: 10px 0 22px`. Icon 21px above an 11px label. Active is accent (icon stroke + label, weight 600); inactive is muted (weight 500). Tabs: **Songs · Setlists**. Only two.

**Metadata shown per row:** only fields that exist. All song metadata except artist is optional — never render an empty slot or a placeholder dash.

**Compact density (wireframe `2b`):** single-line rows, title left / `Artist · key` right in meta-small, `padding: 5px 0`, group headers shrink to 14px. Roughly doubles rows per screen for a 300-song library. Set in Settings → Library density; the `≣` chip toggles it inline. When grouping is set to "First letter", an alphabet scrubber rail appears on the right edge (see wireframe `1c`).

### 2. Song detail — HI-FI

**Structure:**

1. Header: 40×40 back (chevron-left), spacer, 40×40 edit (pencil), 40×40 overflow (dots-vertical). `padding: 2px 18px 8px`.
2. Title block: detail-title, then artist in 15px muted, 5px below.
3. Metadata chips, wrapping, 7px gaps, radius 8, `padding: 6px 11px`, Karla 500 13px. Filled fields only. The **key chip** uses `accent-tint` background with `accent-on-tint` text; every other chip is `fill` with `ink-2`. Order as authored: Key · tuning · capo · bpm · duration · tags.
4. Status line above a hairline top border: confidence word in 600 `ink-2`, three dots, then `· Played Jun 2 · 2 setlists` in muted 13px. The setlist count is a link into the add-to-setlist sheet.
5. **Primary attachment card:** `card` background, radius 16, card shadow, `padding: 16px 18px`, flex-1 so it takes remaining height. Header row: filename in 600 15px, `primary · 2 pages` in muted 12px, spacer, expand icon in accent. Below it a preview of the content. Footer: "Tap to open full screen" in muted 12px. Opens the Attachment viewer.
6. **Other attachments** as collapsed rows: 15px 500 name + `· saved page` / `· typed`, chevron-down in muted, hairline-soft dividers. Expanding one inlines its content; it does not become primary. Long-press or the ⋮ menu sets primary.
7. `+ Add attachment` row in accent 600.
8. Footer bar, hairline top border, `padding: 12px 22px 24px`: primary **Add to setlist** button (`background: ink`, `color: paper`, radius 14, 14px padding, flex-1) plus a 52px-wide `fill` square button with a play glyph that starts performance mode on this one song.

**Overflow (⋮) menu** — wireframe `2d`, order matters: **Add to setlist…** (first, highlighted) · Set confidence · Mark played today · Duplicate · Delete song (destructive). The same menu is the long-press menu on a library row.

### 3. Setlist detail — HI-FI

**Structure:**

1. Header: back · spacer · edit · overflow.
2. Title block: setlist-title, then `5 songs · 18:31 · played Aug 14` in muted 13px.
3. Song rows, `padding: 12px 0`, hairline-soft dividers, aligned to baseline:
   - Position number, accent 600 13px, fixed 15px width.
   - Title (row-title) + meta line (`Artist · key · capo`).
   - Right column, right-aligned: song duration in 500 13px, and **cumulative start time** beneath it in muted 12px (0:00, 3:44, 7:46, 10:04, 14:39). This is the running clock — a musician reads down it to see where they'll be at any point.
   - A song with no chart gets an inline warning pill under its meta: `accent-tint` background, radius 6, `padding: 3px 8px`, alert-circle icon + "No chart attached" in `accent-on-tint` 600 11.5px.
4. Action row: `+ Add songs` in accent 600 14px, `Reorder` in muted, 22px apart.
5. **"Before you start" panel:** `fill` background, radius 12, `padding: 13px 15px`. section-caps label, then body copy in 13.5px/1.5 `ink-2` — aggregated prep facts: which tunings the set needs, how many songs want a capo, and that everything is available offline. Derived, not authored.
6. Footer: full-width **Play set** button — `background: accent`, radius 16, 16px padding, play icon + 16px 600 label in `on-accent`, accent shadow.

Reordering is drag-by-handle; swipe-left removes. Songs appear in a setlist by reference — deleting from a setlist never deletes the song.

### 4. Setlists tab — WIREFRAME (`1m`)

Card per setlist: name (row-title), `5 songs · 18:31 · played Aug 14` in meta, and a truncated song list (`Carolina · Angel From Mont… · Blackbird · +2`). A play glyph on each card starts performance mode directly. The most recent set gets a heavier border. FAB creates a setlist. Long-press a card: duplicate, rename, delete. Header carries the same gear as the Songs tab.

### 5. Add / edit song — WIREFRAME (`1j`)

Full-screen form. `✕` · "New song" · **Save** in accent.

- **Title** and **Artist** are the only visible fields. Artist input autocompletes against existing artists and shows a `use "<typed>"` escape hatch.
- Everything else sits behind a collapsed **More details** disclosure with the sub-line "key · tempo · tuning · capo · duration · tags · confidence · notes — all optional".
- **Attachments** section, three rows: Pick a PDF · Save a webpage offline · Type lyrics / chords.
- Footer note: "Saved on this device. Nothing leaves the phone."

Adding a song must be possible in a few seconds: type a title, hit Save.

### 6. Attachment viewer — WIREFRAME (`1k`)

Full-screen, dark chrome regardless of theme (`#1a1a1a` in the wireframe — use dark-mode `paper`/`ink`). Top bar: back, filename + song name beneath, expand, overflow. Content area on a neutral backdrop. Bottom bar: page prev/next with `1 / 2`, then zoom out / zoom in / rotate. Chrome auto-hides; tap restores it.

### 7. Add webpage (offline capture) — WIREFRAME (`1l`)

The distinguishing feature. Paste a URL, watch it come down, keep it forever.

- URL field.
- Progress panel with a checklist: **Fetched page** ✓ · **Stripped ads & scripts** ✓ · **Downloading images (3 of 7)** in progress, a determinate accent progress bar, and `1.2 MB so far · will work with no signal`.
- A preview of the captured result.
- `Save as: Reader text ▾` — reader-extracted text vs. full page fidelity.
- Footer: Cancel · **Attach**.

Failure states to design during build: fetch failed, partial capture, paywalled/JS-only page.

### 8. Add to setlist sheet — WIREFRAME (`2e`)

Bottom sheet, ~57% height, radius 18 top corners, grab handle. Title "Add to setlist" with the song name beneath. Search field. Checkbox list of setlists with `9 songs · 32:04` sub-lines; a setlist that already contains the song shows a disabled check and "already in this set". `+ New setlist…` in accent at the end. Footer: "added at the end of each" + **Add** button.

Reachable from song detail ⋮, from the footer button, and from a library row long-press. Multi-select — one song into several setlists at once.

### 9. Setlist editing — WIREFRAME (`1i`, chosen)

Editing happens **on the setlist detail screen**; a bottom-sheet picker slides over it so the set stays visible behind. The sheet has search, filter chips (All / Solid / Recent / Tag), a checkbox list, a running "2 picked" count, and an **Add 2 songs** button. This was chosen over a two-step wizard (`1g`) and an inline search-at-top pattern (`1h`).

### 10. Performance view — WIREFRAME (`1o`)

Playing a set, phone on a stand.

- Thin top bar: `2 / 5` position, centered song title with `G · capo 2 · 96 bpm` beneath, close.
- Full-bleed chart fills everything else. Larger type than the detail preview.
- Swipe left/right between songs; edge chevrons hint at it (the previous-song chevron dims at the ends).
- Bottom bar: `up next Blackbird`, a **keep awake** toggle, a `set` button opening the running order.
- A 5-segment progress strip at the very bottom: played segments solid ink, current segment accent, upcoming muted.

**Theme:** the product owner chose **user choice** — performance mode defaults to following the app theme, with a Settings option to force it dark. Design both.

### 11. Sort & group sheet — WIREFRAME (`2c`)

Bottom sheet, ~68% height.

- **Group by** chip row: Confidence (default) · First letter · Artist · Tag · Tuning · None.
- **Sort within group** list: every metadata field as a row — Artist, Title, Confidence, Last played, Date added, Key, Tempo, Duration, Capo, Tuning. Each row shows a human-readable direction ("A → Z", "newest first", "slow → fast") and a direction arrow. The active row is tinted. **Tapping the active row reverses it**; tapping another row selects it.
- Fields almost nobody has filled in are greyed with a count ("3 songs have this") but remain selectable.
- Footer: "Songs missing a field sort last" + Done.

Sorting by artist was an explicit requirement; every field is sortable in both directions.

### 12. Settings — WIREFRAME (`1q`)

Grouped rows, no account section.

- **Storage** — Attachments on device (248 MB) · Saved webpages (37) · Re-check saved pages (off).
- **Backup** — Export library (.zip) · Import from file · Last export.
- **Defaults** — Default tuning · Library sort · **Library density (comfortable / compact)** · Keep screen awake while playing (on) · **Accent color** · **Performance mode theme** · Dark mode follows system.
- Footer: "SetListArray · works with no connection. Nothing is uploaded anywhere."

### 13. First run — WIREFRAME (`1r`)

One job on screen. Wordmark, an illustration slot, the question **"What's a song you know how to play?"**, sub-line "Add it now. Charts, keys and setlists can come later — or never.", a single title field, an **Add song** button, and a secondary "or import a backup file". Footer: "Everything stays on this phone. No account, no signal needed." Bottom nav visible but dimmed.

---

## Interactions & Behavior

| Trigger | Result |
| --- | --- |
| Tap bottom nav | Switch Songs ⇄ Setlists. Each tab keeps its own scroll position and filter state. |
| Tap library row | Song detail. |
| Long-press library row | Same overflow menu as song detail ⋮, "Add to setlist…" first. |
| Tap group header | Collapse/expand that group. Collapse state persists. |
| Tap `Group:` chip or `≣` | Sort & group sheet / density toggle. |
| Tap search | Search & filter screen (wireframe `1p`): live results grouped into **Songs**, **Setlists**, and **Inside attachments** (full-text over typed text and captured pages), with the matched substring highlighted. |
| Tap primary attachment card | Attachment viewer. |
| Tap a collapsed attachment row | Expands inline. |
| Tap "Add to setlist" | Bottom sheet `2e`. |
| Tap "Play set" | Performance view at song 1. |
| Swipe in performance view | Next/previous song. |
| Swipe-left a setlist row | Remove from set (song untouched). |
| Drag a setlist row handle | Reorder. |
| Pick a PDF / URL / type text | Attachment flows; the first attachment added becomes primary automatically. |

Motion: sheets slide up with a standard Android ease-out, ~200–250ms. Group collapse animates height. Nothing else needs animation — this app is read, not watched.

## State

Suggested Rinch stores (the README's `todo-app` and `Rorumall` examples show the pattern):

- `SongsStore` — `Vec<Song>`; add/edit/delete, set confidence, mark played. `Song { id, title, artist, key?, tempo?, tuning?, capo?, duration?, tags: Vec<String>, confidence: Option<Confidence>, notes?, last_played?, created_at, attachments: Vec<AttachmentId>, primary_attachment: Option<AttachmentId> }`. Every field but `id`, `title`, `artist`, `created_at` is optional.
- `SetlistsStore` — `Setlist { id, name, song_ids: Vec<SongId>, last_played? }`. Order is the vector order. Membership is by reference.
- `AttachmentsStore` — `Attachment { id, kind: Pdf | CapturedPage | Text, title, bytes_on_disk, page_count?, source_url?, captured_at?, body? }`.
- `LibraryViewStore` — search query, group-by, sort field + direction, active filters, density, collapsed-group set. Persisted.
- `SettingsStore` — accent (user pick or `FromSystem`), theme mode, performance theme, default tuning, keep-awake.
- `PlaybackStore` — active setlist, current index, keep-awake flag.

Derived values are `Memo`s, not stored: group buckets, cumulative setlist times, total runtime, the "Before you start" prep facts, "In N setlists".

Persistence is local files/SQLite on device. Webpage capture writes HTML + assets into an attachment directory. Export/import is a zip of the database plus that directory.

## Assets

None to hand over. The prototype uses inline SVG placeholders for icons — replace with Tabler Icons from `rinch-tabler-icons`. Fonts are Newsreader and Karla from Google Fonts; bundle both for offline use rather than fetching at runtime. The first-run illustration slot is an unfilled placeholder — no artwork exists yet.

## Files

- `SetListArray Hi-fi.dc.html` — hi-fi: library, song detail, setlist detail (light + dark) and the accent panel.
- `SetListArray Wireframes.dc.html` — flow wireframes. Turn 2 (`2a`–`2e`) is current; turn 1 (`1a`–`1r`) is prior exploration.
- `Hifi-A-Gigbook.dc.html` — the chosen direction as first presented.
- `Hifi-B-Stage.dc.html`, `Hifi-C-Utility.dc.html` — rejected directions, included only as context for why the chosen one won.

Open any of them in a browser to view.

## Decisions already made — don't relitigate

- Two bottom-nav tabs only. Settings is a gear in each tab's header.
- Confidence grouping is the default library view, not A–Z.
- Compact density is a setting, not a separate screen.
- All song metadata except artist is optional, and unfilled fields are simply absent from the UI.
- Songs missing the active sort field sort to the bottom of their group rather than disappearing.
- Warm neutrals are fixed; only the accent varies.
- Every text color clears 4.5:1 on its background.

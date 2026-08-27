# Bundled fonts, and what their licences ask for

These three families are compiled into the binary on both platforms (see
`FONTS` in `src/lib.rs`), so they are *redistributed* with every build — an APK
included. That is what makes this directory necessary rather than tidy: two of
the three licences require their text to travel with the font.

| Font | Licence | Text |
| --- | --- | --- |
| Newsreader (roman + italic) | SIL Open Font License 1.1 | [OFL-1.1.txt](OFL-1.1.txt) |
| Karla | SIL Open Font License 1.1 | [OFL-1.1.txt](OFL-1.1.txt) |
| DejaVu Sans Mono | Bitstream Vera + public-domain changes | [DejaVu-copyright.txt](DejaVu-copyright.txt) |

Newsreader and Karla come from `github.com/google/fonts/raw/main/ofl`, which is
where `scripts/install-fonts.sh` fetches them; everything under that tree is
OFL 1.1. DejaVu Sans Mono was taken from this machine's
`fonts-dejavu-core` package, and `DejaVu-copyright.txt` is that package's own
copyright file, carried over unedited.

**What each asks of this app.** The OFL wants its text distributed with the
font, forbids selling the font on its own, and reserves the right to require a
rename if the font is modified — none of which this app does or wants to do.
Bitstream Vera is the same shape: keep the copyright and permission notice with
the Font Software, do not sell the fonts by themselves, and do not use
"Bitstream" or "Vera" in a modified font's name. The fonts here are unmodified.

**If a font is added, changed or dropped**, update this table and put its
licence text beside the others in the same commit. A font that ships without
its licence is a licence breach, not an oversight — and it is invisible, which
is exactly why it needs a note rather than a memory.

`scripts/install-fonts.sh` installs Newsreader and Karla into the *user's*
fontconfig as a convenience for desktop development. That is a local install,
not redistribution, and it is not what this directory is about. Since the fonts
are compiled in, that script is no longer required to run the app.

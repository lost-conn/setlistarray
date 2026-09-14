#!/usr/bin/env python3
"""Put the store page in `store/metadata/` onto Google Play, in a single edit.

    scripts/play-listing.py --dry-run
    PLAY_SERVICE_ACCOUNT_JSON="$(cat key.json)" scripts/play-listing.py

`play.yml` ships the app; this ships the page in front of it. The words come out
of `store/metadata/en-US/` — `title.txt`, `short_description.txt`,
`full_description.txt`, the same three files
`the_play_listing_copy_fits_inside_the_limits_play_enforces` in `src/lib.rs`
holds to Play's limits — and the pictures out of
`store/metadata/en-US/images/`, which `scripts/store-frame.py` renders and which
`.gitignore` deliberately does not keep, because they are a pure function of the
app and rebuilding them is a minute's work.

The title goes too, not only the descriptions. It could have been left to the
console on the argument that a name changes once a decade, and that is exactly
why it should be here: a field nobody ever edits is a field nobody remembers is
editable, and the point of keeping the listing in git is that the repository is
the whole answer to "what does the store page say". Half an answer is worse than
none, because it is the half you stop checking.

────────────────────────────────────────────────────────────────────────────
What an "edit" is, and why the whole run is one transaction
────────────────────────────────────────────────────────────────────────────

Nothing in the Play Developer API writes to a published listing directly. You
open an **edit** — `edits.insert`, which hands back an id — make every change
against that id, and then `edits.commit` applies the whole set at once. An edit
that is never committed changes nothing at all; it simply expires. So the
sequence below is one transaction in the ordinary sense, and a run that dies
between the descriptions and the screenshots leaves the store page exactly as it
was rather than half-rewritten. That property is the reason to talk to this API
directly instead of firing three independent "update this field" calls at it.

Two consequences are worth stating because both shaped this file.

The first is that **everything checkable is checked before `edits.insert`**. The
validation below reads every file, counts every field and measures every PNG
while no edit exists, so a description that is four characters too long fails
with an edit that was never opened instead of one that has to be abandoned. This
is the same argument `store-frame.py --check` makes about rendering, made one
stage later: a rejection is cheapest at the point where nothing has happened yet.

The second is that **only one edit may be open on a package at a time**. Play
refuses `edits.insert` while another edit is outstanding, and "outstanding"
includes the one `play.yml`'s upload action is holding while it pushes an eleven
megabyte bundle. That is why the publish job in `.github/workflows/listing.yml`
shares `play.yml`'s `concurrency: group: play` rather than having a group of its
own: the two workflows have nothing else in common and no reason to queue, right
up until they are both talking to the same package, at which point the second
one to arrive fails for a reason that reads like a bug in this script.

────────────────────────────────────────────────────────────────────────────
`deleteall` before every upload, and the run that taught us
────────────────────────────────────────────────────────────────────────────

Play's images are **a set you replace, not a list you append to**, and the API
does not pretend otherwise: `edits.images.upload` adds an image to the language's
collection for that type, and nothing about uploading `1_library.png` a second
time replaces the first one. Play stores images by its own id, not by the
filename they arrived under, so the second run's seven screenshots land *beside*
the first run's seven. The listing then has fourteen, Play publishes the first
eight of them in its own order, and the store page ends up showing a mixture of
two runs — which looks, to anybody opening it, like a compositing bug in
`store-frame.py` rather than a missing call here. Nothing fails. Nothing is
logged. The only symptom is a store page with the wrong pictures on it.

So every image type is emptied with `edits.images.deleteall` immediately before
its uploads, inside the same edit, which means the replacement is atomic like
everything else: if the run dies after the `deleteall` the commit never happens
and the live listing still has its old screenshots.

────────────────────────────────────────────────────────────────────────────
Why this is Python over the REST API and not fastlane `supply`
────────────────────────────────────────────────────────────────────────────

`supply` does this exact job, reads this exact directory layout — the
`metadata/en-US/…` and `images/phoneScreenshots/…` names here are its
conventions, adopted on purpose so that this decision stays reversible — and
would be a shorter file than this one.

It is Ruby. Putting it on the runner means a Ruby toolchain, a Gemfile and a
bundler lock in a repository that has none of those, maintained by people who do
not write Ruby, for one call sequence. Against that, this repository's tooling is
already Python and has been since the icons: `make-icons.py`, `make-notices.py`,
`scripts/store-frame.py` and `scripts/icon_gradient.py` all run on a stock
interpreter with Pillow and NumPy, and the compositor that produces the very
images this uploads is one of them. `google-api-python-client` is one `pip
install` on a runner that is already installing Python packages, and the API it
wraps is the one documented thing here — `edits.insert`, `listings.update`,
`images.deleteall`, `images.upload`, `edits.commit` — so a person debugging a
403 is reading Google's own reference rather than a Ruby wrapper's translation
of it.

That import is deliberately *not* at the top of this file: `--dry-run` does every
piece of validation and prints everything it would send without authenticating,
and it has to keep working on a machine where the client library was never
installed and no credential exists. On this project's development laptop that is
not a hypothetical — it is the only mode that can be run at all, by the rule that
nothing here talks to Play from a developer's machine.
"""

import argparse
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# The same package name `play.yml` hands the upload action. Spelled here rather
# than read out of the workflow because a YAML file is not an interface; if the
# two ever disagree the symptom is an edit opened on an app that does not exist,
# which at least fails immediately and by name.
PACKAGE = "dev.lostconnection.setlistarray"
LOCALE = "en-US"
METADATA = ROOT / "store" / "metadata"

CREDENTIAL_ENV = "PLAY_SERVICE_ACCOUNT_JSON"
SCOPE = "https://www.googleapis.com/auth/androidpublisher"

# Play's limits on the three text fields, in UTF-16 code units — see
# `assert_listing_field` in `src/lib.rs` for why the unit matters and why
# neither bytes nor Rust `char`s are it. The numbers are here as well as there
# because this script can be pointed at a metadata directory that no `cargo
# test` ever saw, and because the check that runs is the one that counts.
FIELDS = (
    ("title", "title.txt", 30),
    ("shortDescription", "short_description.txt", 80),
    ("fullDescription", "full_description.txt", 4000),
)

# Play's limits on the three image classes, which are three different sets of
# rules and not one — `store-frame.py`'s header makes that argument at length.
#
# This is the second copy of these numbers in the repository and that is a
# deliberate trade rather than an oversight. `store-frame.py` checks them at
# render time, where Pillow and NumPy are already installed; this checks them at
# upload time, where the only dependency wanted is the API client, and it has to
# work on a directory that arrived as a workflow artifact rather than one this
# machine just rendered. Sharing them would mean importing a module that pulls
# NumPy onto a runner whose job is to make five HTTPS calls. If either copy
# moves, move both: they are describing the same external rule and there is no
# version of this where one of them is right and the other is not.
SHOT_MIN_SIDE, SHOT_MAX_SIDE = 320, 3840
SHOT_MAX_RATIO = 2.0
SHOT_MIN_COUNT, SHOT_MAX_COUNT = 2, 8
FEATURE_SIZE = (1024, 500)
ICON_SIZE = (512, 512)
MAX_BYTES = 8 * 1024 * 1024

RED = "\033[31m"
GREEN = "\033[32m"
DIM = "\033[2m"
RESET = "\033[0m"


def die(message):
    sys.exit(f"{RED}✗ {message}{RESET}")


def ok(message):
    print(f"{GREEN}✓{RESET} {message}")


# ---------------------------------------------------------------------------
# The words
# ---------------------------------------------------------------------------


def utf16_units(text):
    """How long Play thinks this string is.

    Play measures a field the way a Java `String` measures itself. `len()` in
    Python counts code points, which is `char` in Rust and wrong the same way:
    one emoji or one musical symbol is a single code point and two of Play's
    units. Encoding and halving is the boring, correct answer.
    """
    return len(text.encode("utf-16-le")) // 2


def load_copy(metadata_dir, locale):
    """The three text fields, read and held to everything Play cares about.

    The checks are `assert_listing_field`'s in `src/lib.rs`, repeated here for
    the same reason the image limits are: that test guards *this tree's* copy on
    a laptop, and this guards whatever copy is actually about to be sent, which
    on a runner is a checkout nobody has run `cargo test` against in this job.

    The trailing newline is stripped before sending. It is a property of a text
    file, not of the field — Play stores what it is handed verbatim, and a
    listing that ends in a blank line is a store page with a gap under its last
    paragraph that nobody can see in an editor.
    """
    copy = {}
    for key, filename, limit in FIELDS:
        path = metadata_dir / locale / filename
        if not path.is_file():
            die(f"{path} is missing; Play will not take a listing without a {key}")

        text = path.read_text(encoding="utf-8")
        if not text.strip():
            die(f"{path} is empty. Play does not refuse an empty field, it publishes one")

        ragged = [n for n, line in enumerate(text.splitlines(), start=1) if line.rstrip() != line]
        if ragged:
            die(f"{path} has trailing whitespace on line(s) {ragged}, and Play stores it verbatim")

        field = text.strip()
        units = utf16_units(field)
        if units > limit:
            die(
                f"{path} is {units} UTF-16 code units and Play's limit for {key} is {limit}. "
                f"That is the count Play makes; this text is {len(field.encode('utf-8'))} bytes "
                f"and {len(field)} code points, and neither number decides anything"
            )
        copy[key] = field
    return copy


# ---------------------------------------------------------------------------
# The pictures
# ---------------------------------------------------------------------------


def png_size(path):
    """A PNG's dimensions out of its own IHDR, and a refusal if it is not one.

    Read by hand rather than by opening it, exactly as `store-frame.py` does and
    for the same two reasons: "is this actually a PNG" is one of the things being
    asked, and a decoder would happily answer it for a JPEG somebody renamed —
    and, here, because doing it this way is what keeps this script's dependencies
    down to the API client.
    """
    with path.open("rb") as handle:
        header = handle.read(24)
    if header[:8] != b"\x89PNG\r\n\x1a\n":
        die(f"{path} is not a PNG; Play takes PNG or JPEG for these, and not a renamed one")
    return int.from_bytes(header[16:20], "big"), int.from_bytes(header[20:24], "big")


def check_bytes(path):
    size = path.stat().st_size
    if size > MAX_BYTES:
        die(f"{path.name} is {size / 1024 / 1024:.1f} MB; Play's limit for one image is 8 MB")


def load_images(images_dir):
    """Every picture this run would send, keyed by the `imageType` Play calls it.

    The order matters and is the directory's: `store-frame.py` numbers the
    screenshots `1_library.png` … `7_performance.png` so that the order of
    `store/shots.json` is the order of the store page, and Play shows them in the
    order they are uploaded. Sorting by filename is what turns that numbering
    into the promise it looks like.
    """
    shots_dir = images_dir / "phoneScreenshots"
    if not shots_dir.is_dir():
        die(
            f"{shots_dir} does not exist. The finished images are not in git — run "
            f"`scripts/store-frame.py .shots {images_dir}` first, or download them from the "
            "Listing workflow's compose job"
        )

    shots = sorted(shots_dir.glob("*.png"))
    if not SHOT_MIN_COUNT <= len(shots) <= SHOT_MAX_COUNT:
        die(
            f"{len(shots)} phone screenshot(s) in {shots_dir}; Play takes between "
            f"{SHOT_MIN_COUNT} and {SHOT_MAX_COUNT} and publishes none of them if there are more"
        )

    for path in shots:
        width, height = png_size(path)
        short, long = min(width, height), max(width, height)
        if short < SHOT_MIN_SIDE or long > SHOT_MAX_SIDE:
            die(
                f"{path.name} is {width}x{height}; Play wants every side between "
                f"{SHOT_MIN_SIDE} and {SHOT_MAX_SIDE}px"
            )
        if long > short * SHOT_MAX_RATIO:
            die(
                f"{path.name} is {width}x{height}, a ratio of {long / short:.3f}:1; Play refuses "
                f"a phone screenshot longer than {SHOT_MAX_RATIO:g}:1"
            )
        check_bytes(path)

    images = {"phoneScreenshots": shots}
    for image_type, filename, want in (
        ("featureGraphic", "featureGraphic.png", FEATURE_SIZE),
        ("icon", "icon.png", ICON_SIZE),
    ):
        path = images_dir / filename
        if not path.is_file():
            die(f"{path} is missing; Play requires it and refuses the listing without it")
        size = png_size(path)
        if size != want:
            die(f"{filename} is {size[0]}x{size[1]}; Play requires exactly {want[0]}x{want[1]}")
        check_bytes(path)
        images[image_type] = [path]

    return images


# ---------------------------------------------------------------------------
# Saying what would happen
# ---------------------------------------------------------------------------


def describe(package, locale, copy, images, changes_not_sent_for_review):
    """Print the whole run, in the order it would happen, and send nothing.

    Deliberately a transcript of the call sequence rather than a summary of the
    outcome. The thing a person needs to check before letting this near a live
    store page is not "did it find seven pictures" — `--check` already said that
    — it is "is it about to overwrite the right app's listing with the right
    words", and the package name and the first line of each field answer that in
    one screen.
    """
    print(f"{DIM}package {RESET}{package}{DIM}, language {RESET}{locale}")
    print(f"{DIM}--> edits.insert{RESET}")

    print(f"{DIM}--> edits.listings.update{RESET}")
    for key, _, limit in FIELDS:
        field = copy[key]
        units = utf16_units(field)
        first = field.splitlines()[0]
        if len(first) > 68:
            first = first[:67] + "…"
        lines = len(field.splitlines())
        tail = f", {lines} lines" if lines > 1 else ""
        print(f"      {key:<16} {units:>4}/{limit} units{tail}")
        print(f"      {DIM}{' ' * 16} “{first}”{RESET}")

    total = 0
    for image_type, paths in images.items():
        print(f"{DIM}--> edits.images.deleteall {RESET}{image_type}")
        for path in paths:
            width, height = png_size(path)
            kib = path.stat().st_size / 1024
            print(
                f"{DIM}--> edits.images.upload   {RESET} {image_type}/{path.name}  "
                f"{width}x{height}  {kib:.0f} KiB"
            )
            total += 1

    review = "not sent for review" if changes_not_sent_for_review else "sent for review"
    print(f"{DIM}--> edits.commit{RESET} ({review})")
    ok(f"dry run: 3 text fields and {total} image(s) validated, nothing sent, no edit opened")


# ---------------------------------------------------------------------------
# Doing it
# ---------------------------------------------------------------------------


def credential_info():
    """The service account, out of the environment and never off the disk.

    An environment variable rather than a path because that is how `play.yml`
    already receives this exact key: GitHub hands a secret to a step as a string,
    and a step that has to write it to a file first is a step that leaves a
    private key in the workspace for whatever runs next. The name is `play.yml`'s
    too, so that one entry in the `play` environment serves both workflows.

    Stdlib only, and called before anything is imported from the API client, so
    that running this on a machine with neither — which is every machine this is
    developed on — says which environment variable is missing and what to pass
    instead of it, rather than a `ModuleNotFoundError` about a library that is
    only the second thing wrong.
    """
    raw = os.environ.get(CREDENTIAL_ENV, "").strip()
    if not raw:
        die(
            f"{CREDENTIAL_ENV} is unset or empty. It wants the service account's JSON key as a "
            "string, the way the play environment holds it for play.yml. To see what this run "
            "would do without one, pass --dry-run"
        )
    try:
        info = json.loads(raw)
    except json.JSONDecodeError as error:
        die(f"{CREDENTIAL_ENV} is not JSON ({error}); it wants the key file's contents, not a path")
    return info


def publish(package, locale, copy, images, changes_not_sent_for_review):
    """Open one edit, write everything into it, commit it.

    The `try` is not error handling in the usual sense — nothing here can be
    recovered from — it is there to abandon the edit. An edit left open blocks
    the next `edits.insert` on this package until Play expires it, which would
    turn one failed run into a second failed run that fails for an unrelated and
    much more confusing reason. Abandoning is best-effort by the same logic: if
    even that call fails there is nothing further to be done, and saying so is
    more use than replacing the original traceback with the cleanup's.
    """
    info = credential_info()

    from google.oauth2 import service_account
    from googleapiclient.discovery import build
    from googleapiclient.http import MediaFileUpload

    credentials = service_account.Credentials.from_service_account_info(info, scopes=[SCOPE])
    service = build("androidpublisher", "v3", credentials=credentials, cache_discovery=False)
    edits = service.edits()

    edit_id = edits.insert(packageName=package, body={}).execute()["id"]
    print(f"--> edit {edit_id} is open on {package}")
    try:
        # `listings.update` replaces the whole listing resource for this
        # language, so the fields not named here are cleared rather than left
        # alone — `video`, in practice, which this listing does not have and
        # which therefore has nowhere else it could be set from. That is the
        # intended behaviour: the repository is the listing, including the parts
        # of it that are empty.
        edits.listings().update(
            packageName=package, editId=edit_id, language=locale, body=copy
        ).execute()
        ok(f"listings.update: {', '.join(key for key, _, _ in FIELDS)}")

        for image_type, paths in images.items():
            edits.images().deleteall(
                packageName=package, editId=edit_id, language=locale, imageType=image_type
            ).execute()
            for path in paths:
                edits.images().upload(
                    packageName=package,
                    editId=edit_id,
                    language=locale,
                    imageType=image_type,
                    media_body=MediaFileUpload(str(path), mimetype="image/png", resumable=False),
                ).execute()
            ok(f"{image_type}: emptied, then {len(paths)} uploaded")

        edits.commit(
            packageName=package,
            editId=edit_id,
            changesNotSentForReview=changes_not_sent_for_review,
        ).execute()
    except BaseException:
        try:
            edits.delete(packageName=package, editId=edit_id).execute()
            print(f"--> edit {edit_id} abandoned; the live listing is untouched")
        except Exception as cleanup:
            print(
                f"{RED}! edit {edit_id} could not be abandoned ({cleanup}); it blocks the next "
                f"run until Play expires it{RESET}",
                file=sys.stderr,
            )
        raise

    ok(f"committed; {package}'s {locale} listing is now what store/metadata/{locale}/ says")


def main():
    parser = argparse.ArgumentParser(
        description="Send store/metadata/ to the Google Play listing, in one edit.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="validate everything and print what would be sent, without authenticating",
    )
    parser.add_argument("--package", default=PACKAGE, help=f"Play package name (default {PACKAGE})")
    parser.add_argument("--locale", default=LOCALE, help=f"listing language (default {LOCALE})")
    parser.add_argument(
        "--metadata",
        default=str(METADATA),
        help="the fastlane-layout metadata directory (default store/metadata)",
    )
    parser.add_argument(
        "--images",
        default=None,
        help="where the finished pictures are (default <metadata>/<locale>/images)",
    )
    # The listing half of `play.yml`'s `status: draft`, and needed for the same
    # reason: while Play has never reviewed the app it is a *draft app*, and a
    # commit that asks to send its changes for review is refused outright with a
    # message naming this parameter. It defaults off because that is the state
    # the app spends the rest of its life in, and an upload that quietly skipped
    # review would be a worse default than one that fails once and says why.
    parser.add_argument(
        "--changes-not-sent-for-review",
        action="store_true",
        help="commit without submitting for review; required while the app is still a draft app",
    )
    args = parser.parse_args()

    metadata_dir = Path(args.metadata)
    images_dir = Path(args.images) if args.images else metadata_dir / args.locale / "images"

    # Every one of these runs before anything below it can open an edit. That
    # ordering is the point; see this file's header.
    copy = load_copy(metadata_dir, args.locale)
    images = load_images(images_dir)

    if args.dry_run:
        describe(args.package, args.locale, copy, images, args.changes_not_sent_for_review)
        return
    publish(args.package, args.locale, copy, images, args.changes_not_sent_for_review)


if __name__ == "__main__":
    main()

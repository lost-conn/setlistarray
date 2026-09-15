# Releasing to Play

How a commit becomes a build on Google Play, and how the store page itself is
built out of this repository rather than typed into a console. Moved out of the
README; nothing here has changed but where it lives.

---

`.github/workflows/play.yml` builds the bundle on a GitHub runner and uploads
it. Pushing a `v*` tag (matching `Cargo.toml`'s version, so `v0.1.0` today)
releases to the internal testing track; running the workflow by hand (Actions →
Play → Run workflow) picks the track, or `none` to build only. Either way the
signed bundle is attached to the run.

The runner has no `../rinch-fixes` or `../rhypedb-main`, so it clones both
beside this repo at the commits in `.github/siblings.env` — rinch from the
`lost-conn/rinch` fork, which carries `local/both-fixes` until upstream catches
up. After building against a newer rinch here, push it and re-pin before
tagging:

```bash
git -C ../rinch-fixes push fork local/both-fixes
scripts/pin-siblings.sh           # records both HEADs; refuses one the remote lacks
scripts/pin-siblings.sh --check   # exit 1 if either sibling has moved past its pin
```

Once, by hand, before the workflow can upload anything:

1. Create the app in Play Console and upload its first bundle there — the API
   cannot create an app. A run with track `none` produces the signed bundle.
2. Create a Google Cloud service account, enable the Google Play Android
   Developer API in its project, invite the account in Play Console → Users and
   permissions, and download a JSON key for it. Give it **two** app
   permissions, not one: *Release apps to testing tracks*, which is what
   `play.yml` uploads a bundle with, and *Edit store listing, pricing &
   distribution*, which is what `listing.yml`'s publish job writes the store
   page with. They are separate checkboxes and the first does not imply the
   second — an account with release rights alone uploads bundles perfectly well
   and then 403s on every call `scripts/play-listing.py` makes, with a message
   about the account rather than about the permission it is missing. Nobody
   should have to learn that from the error.
3. Add the secrets below to the repository's `play` environment (Settings →
   Environments), which is also where to limit it to `master` and `v*` tags.

| Secret | What |
| --- | --- |
| `ANDROID_UPLOAD_KEYSTORE_BASE64` | The upload keystore, as `base64 -w0 upload.jks` |
| `ANDROID_UPLOAD_KEY_ALIAS` | Its alias; `upload` if unset |
| `ANDROID_UPLOAD_STORE_PASS` | The keystore's password |
| `ANDROID_UPLOAD_KEY_PASS` | The key's password, only if it differs |
| `PLAY_SERVICE_ACCOUNT_JSON` | The service account's JSON key |

Until Play has reviewed the app once it is a *draft app*, and the API accepts
only draft releases for one: run by hand with status `draft` until then. The
listing API has its own version of the same rule — a draft app cannot submit
changes for review, and `edits.commit` refuses an edit that asks to — so run
Listing with `changes_not_sent_for_review` on until then too, and turn both off
together once the app has been through review.

## The store listing

Everything Play shows *about* the app is built the way the app is: out of the
repository, by a workflow, with nothing typed into a console.
`.github/workflows/listing.yml` runs three jobs, in the order somebody would do
it by hand.

| Job | Runs on | What it does |
| --- | --- | --- |
| `capture` | a headless emulator | `scripts/store-shots.sh` walks the app through the screens in `src/shots.rs` and keeps one raw 1080x1920 frame each, plus the system-bar insets it measured |
| `compose` | a plain runner, no emulator, no secret | `scripts/store-frame.py` adds the gradient, the plate, the shadow and the captions, then holds all nine pictures to Play's shape limits |
| `publish` | only with `upload: true`, in the `play` environment | `scripts/play-listing.py` sends the title, both descriptions and every picture in one Play edit |

Actions → Listing → Run workflow, and **`upload` is false unless you set it**.
The ordinary run captures, composites, checks and attaches the finished pictures
to the run as an artifact — and sends nothing anywhere. That is the moment to
look at them, and it is the only moment anybody ever does. `upload: true`
re-captures and re-composites in the same run rather than fetching an earlier
run's artifact, on the argument that the pipeline is deterministic; the header of
`listing.yml` is honest about how much weight that argument is carrying.

The publish job shares `play.yml`'s `concurrency: group: play`, because Play
allows one open edit per package and a bundle upload holds one for minutes at a
time. Two runs that have nothing else to do with each other queue rather than
one of them failing with an error about an edit.

Where the words live, all of it reviewable in a diff:

* `store/metadata/en-US/title.txt`, `short_description.txt`,
  `full_description.txt` — the listing itself, held to Play's limits in UTF-16
  code units by `the_play_listing_copy_fits_inside_the_limits_play_enforces` in
  `src/lib.rs`.
* `store/metadata/en-US/changelogs/<version>.txt` — the "What's new" note,
  keyed by `Cargo.toml`'s version, which `play.yml` reads on the way to a
  release.
* `store/shots.json` — the caption under each screenshot and the layout it is
  composited with, keyed by the shot ids in `src/shots.rs`; a `cargo test`
  asserts those two sets match in both directions.

The finished images are the one part not in git — `.gitignore` says why — so
rebuild them before looking at anything locally:

```bash
scripts/store-shots.sh                                       # needs an emulator; see CLAUDE.md first
python3 scripts/store-frame.py .shots store/metadata/en-US/images
python3 scripts/store-frame.py --check store/metadata/en-US/images
python3 scripts/play-listing.py --dry-run                    # prints what it would send, sends nothing
```

`--dry-run` is the only mode of that last script to run from a laptop. The
credential lives in the `play` environment and the upload belongs to the
workflow, which is the only place it can be traced back to a run and a commit.

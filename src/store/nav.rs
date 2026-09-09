use std::rc::Rc;

use rinch::prelude::*;

use crate::model::{AttachmentId, SetlistId, SongId};

/// Two bottom-nav tabs only. Settings is a gear in each tab's header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Songs,
    Setlists,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Library,
    Setlists,
    SongDetail(SongId),
    SetlistDetail(SetlistId),
    Settings,
    /// The add/edit form (`1j`). One screen serves both: adding starts empty,
    /// editing arrives carrying the song it is about to overwrite.
    AddSong,
    EditSong(SongId),
    /// The typed lyrics/chords editor (D2) — the screen behind `1j`'s
    /// `TXT · Type lyrics / chords · ›` row, which the handoff never drew.
    /// One screen serves both jobs here too: `chart: None` writes a new
    /// `Text` attachment on Save, `Some(id)` reopens that one. The song is
    /// carried because a chart cannot exist without one — `attach` needs it,
    /// and ✕ goes back to it.
    TypeChart { song: SongId, chart: Option<AttachmentId> },
    /// Save a webpage offline (`1l`, card E2). Carries the song for the same
    /// reason `TypeChart` does: a capture cannot exist without one — it is
    /// `SongsStore::attach` that mints the row and the directory the page is
    /// written into — and ← has to land back on the song that opened it.
    ///
    /// It does *not* carry an attachment. A capture screen only ever produces
    /// a new chart; there is no "re-capture this one" way in, and if E6's
    /// re-check grows one it will be a different operation with a different
    /// question to answer (replace, or keep both?) rather than this route with
    /// an `Option` on it.
    CaptureWebpage { song: SongId },
    /// The full-screen attachment viewer (`1k`, card D5). Carries the song as
    /// well as the chart for two reasons, and neither is decoration: the top
    /// bar prints the song's name under the file's, and ← has to land back on
    /// the screen the viewer was opened from. A chart knows neither — the
    /// ownership arrow runs songs → attachments and never back (see
    /// `AttachmentsStore`'s header), so the only place that pairing exists is
    /// the route.
    ViewAttachment { song: SongId, attachment: AttachmentId },
    /// Search & filter (`1p`, card G1) — the screen behind the library's
    /// search field.
    ///
    /// **It carries nothing, and the query it is a screen for lives in
    /// [`LibraryViewStore::query`](crate::store::LibraryViewStore).** That is
    /// the one design decision this variant makes, and it is made against the
    /// obvious alternative — `Search { query: String }` — for two reasons that
    /// are both about this enum rather than about search.
    ///
    /// `Route` is `Copy`. Every variant above carries an id or two and nothing
    /// else, which is what lets `nav.route.get()` be read in half a dozen
    /// reactive closures a frame, matched on in `crate::app`, and passed by
    /// value to [`crate::derive::dark_chrome`] and [`crate::keep_awake::wanted`]
    /// without a clone anywhere. A `String` in here takes that away from every
    /// other variant too.
    ///
    /// And a query on the route makes every keystroke a navigation. The field
    /// updates as you type — that is the whole of what `1p` promises — so
    /// `Route::Search { query }` would mean `route.set(...)` per character,
    /// with every screen-level effect keyed on the route recomputing behind it,
    /// to move a value that no other screen has any business reading. Routes
    /// here answer *which screen*, and the answer does not change while you
    /// type.
    Search,
    /// Performance mode (`1o`, card F1) — the set being played, a song at a
    /// time. Carries the setlist and not the song, because *which* song is
    /// `PlaybackStore::index` and that has to survive being changed from
    /// inside the screen: F2's next/previous move through the set without
    /// leaving it, and a route carrying the song would make every step a
    /// navigation and every step a remount of the chart being read off.
    Performance(SetlistId),
}

impl Route {
    /// Whether this screen takes the whole window, bottom nav included.
    ///
    /// Two routes answer yes. The viewer (`1k`) is drawn edge to edge in dark
    /// chrome of its own, and a cream tab bar under it would be both the wrong
    /// colour and 60 px of a chart somebody is reading off a music stand.
    /// Performance mode (`1o`) is the same argument with the stakes raised:
    /// `1o` draws no bottom nav at all, the whole screen below its thin top bar
    /// is the chart, and the two tabs it would offer are both "leave the gig
    /// you are in the middle of" — which is what the ✕ in its own top bar is
    /// for, deliberately, and once.
    ///
    /// It is a method on `Route` rather than a flag the screens set, because
    /// the two things that have to obey it — the bottom nav and the strip
    /// behind the status bar — are drawn by `crate::app`, above the screen, and
    /// the route is the only thing they and the screen share.
    pub fn full_screen(self) -> bool {
        matches!(self, Route::ViewAttachment { .. } | Route::Performance(_))
    }

    /// Whether this screen is dark **whatever the app's theme is set to**.
    ///
    /// Split out of [`full_screen`](Self::full_screen) by card F1, because the
    /// moment a second route went full-screen the two questions stopped having
    /// the same answer. `crate::app` asks this one twice — for the background
    /// of the strip behind the status bar, and for whether to tell Android to
    /// draw its clock and gesture pill in dark glyphs — and both of those are
    /// about *colour*, not about how much of the window a screen takes.
    ///
    /// The viewer says yes: the handoff's README is explicit that its chrome is
    /// dark regardless of theme, and before this split a light-themed phone
    /// showed a cream strip above the viewer's black top bar.
    ///
    /// Performance mode says no, and that is the handoff's answer rather than a
    /// simplification: *"performance mode defaults to following the app theme,
    /// with a Settings option to force it dark"*. So `1o` in light mode is a
    /// cream screen with a cream strip above it, which is exactly right.
    ///
    /// **That day has come.** Card H1 built the Settings screen and with it the
    /// switch that forces `1o` dark, so this is no longer the whole question —
    /// it is the route half of [`crate::derive::dark_chrome`], which asks route
    /// *and* `SettingsStore::performance_theme` together. Everything that wants
    /// the real answer calls that; this stays exactly as narrow as it reads,
    /// "dark because of where you are, whatever anybody has set", and the viewer
    /// is still the only route that says yes to it.
    pub fn dark_chrome(self) -> bool {
        matches!(self, Route::ViewAttachment { .. })
    }
}

/// Which of the seven dismissable overlays a Back press found open, for the
/// benefit of the test that pins their order. Nothing on screen reads this —
/// it exists so that [`NavStore::press_back`] can be checked without a window,
/// which is the whole of card K15's rule applied to a key nobody can press on
/// a laptop.
///
/// The names are the sheets' own, not the signals' — `SongPicker` is
/// `picking_songs_for`, `RenameSetlist` is the inline field on a setlist card
/// rather than a sheet at all — because a failure message that says which
/// *sheet* stayed open is one a person can act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    Tuning,
    RunningOrder,
    SongPicker,
    AddToSetlist,
    Filters,
    SortAndGroup,
    RenameSetlist,
}

/// What a Back press did. Returned by [`NavStore::press_back`], and the only
/// reason that function has a return value at all.
///
/// **`NothingLeftToDoButExit` is a report, not an exit.** `press_back` closes
/// sheets and runs screens' back actions itself, but it will not end the
/// process, because a function that can end the process cannot be called from
/// a `cargo test` — the test binary would go down with the app, and the one
/// case worth pinning hardest (a tab root with nothing open) is exactly the
/// one that would do it. So the last step is left to the caller: the keyboard
/// interceptor in `crate::app` is the single place that turns this variant
/// into `close_current_window()`, and the tests below get to assert on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackPress {
    ClosedASheet(Sheet),
    RanTheScreensBackAction,
    NothingLeftToDoButExit,
}

/// A screen's own ← / ✕, parked where a key press can find it.
///
/// The `route` half is what makes this safe to leave lying around. A component
/// body runs once, at mount, and there is no matching hook that runs at
/// unmount which this app already uses — so a screen that registered its back
/// action and was then navigated away from would leave the closure behind,
/// pointing at signals its own disposed scope owns, and Back on the library
/// root would run song detail's `nav.back()` instead of exiting. Stamping the
/// route the registration was made from and checking it against the route on
/// screen turns that stale entry into a miss, which is precisely the "nothing
/// registered" case the roots want.
///
/// Doing it this way round rather than having the two roots register a `None`
/// of their own is deliberate: a screen that forgets to register gets a Back
/// that exits the app, which is wrong but loud and which
/// `crate::back_coverage`'s scan of the shipping source catches at
/// `cargo test` time. A screen that forgot to *clear* would leave the previous
/// screen's action armed under it, which is silent, and which no scan could
/// see.
#[derive(Clone)]
struct BackAction {
    route: Route,
    run: Rc<dyn Fn()>,
}

#[derive(Clone, Copy)]
pub struct NavStore {
    pub route: Signal<Route>,
    /// Each tab keeps its own scroll position and filter state; the route
    /// stack is per-tab for the same reason.
    pub tab: Signal<Tab>,
    /// The add-to-setlist bottom sheet, when open for a song.
    pub add_to_setlist_for: Signal<Option<SongId>>,
    /// The sort & group bottom sheet.
    pub sort_sheet_open: Signal<bool>,
    /// The filters bottom sheet (card G3) — the library's `Filter` chip.
    pub filter_sheet_open: Signal<bool>,
    /// The default-tuning picker (card H5) — Settings' `Default tuning` row.
    /// A sheet rather than inline chips: `crate::screens::settings`'s module
    /// header says why seven tunings do not fit the way two densities do.
    pub tuning_sheet_open: Signal<bool>,
    /// The setlist song picker (`1i`), when open over a setlist. The setlist
    /// detail screen stays mounted and visible behind it — that is the whole
    /// reason `1i` was chosen over the two-step wizard.
    pub picking_songs_for: Signal<Option<SetlistId>>,
    /// The running order (F3), when open over performance mode — the set being
    /// played, so the sheet can list it without asking which gig is on.
    ///
    /// It lives here with the other three for a reason particular to the screen
    /// that opens it. Performance mode is `Route::full_screen`, so it takes the
    /// whole window; a sheet nested inside `performance.rs` would be a child of
    /// that screen's own column and would slide up *inside* it rather than over
    /// it. Every sheet in this app is mounted in `crate::app` as a sibling
    /// after the route (see the note there), above everything, which is what
    /// makes them work over a full-screen route at all — and a sheet mounted
    /// out there can only be told to open by a signal both ends can see.
    pub running_order_for: Signal<Option<SetlistId>>,
    /// The setlist card currently in inline rename mode on the Setlists tab
    /// (opened from its long-press/right-click menu), and the text field's
    /// live draft. Screen-transient UI state, same as the two fields above —
    /// not worth a store of its own for one small pair of signals.
    pub renaming_setlist: Signal<Option<SetlistId>>,
    pub rename_draft: Signal<String>,
    /// What the screen currently on show does when its own ← or ✕ is tapped,
    /// so that the Android Back key can do the same thing rather than a second
    /// thing that drifts from it. Written by [`NavStore::register_back`] from a
    /// screen's component body; read by [`NavStore::press_back`] and by nothing
    /// else, ever reactively — see both for why.
    back_action: Signal<Option<BackAction>>,
}

impl NavStore {
    pub fn new() -> Self {
        Self {
            route: Signal::new(Route::Library),
            tab: Signal::new(Tab::Songs),
            add_to_setlist_for: Signal::new(None),
            sort_sheet_open: Signal::new(false),
            filter_sheet_open: Signal::new(false),
            tuning_sheet_open: Signal::new(false),
            picking_songs_for: Signal::new(None),
            running_order_for: Signal::new(None),
            renaming_setlist: Signal::new(None),
            rename_draft: Signal::new(String::new()),
            back_action: Signal::new(None),
        }
    }

    pub fn go(self, route: Route) {
        self.route.set(route);
    }

    pub fn select_tab(self, tab: Tab) {
        self.tab.set(tab);
        self.route.set(match tab {
            Tab::Songs => Route::Library,
            Tab::Setlists => Route::Setlists,
        });
    }

    /// Back always lands on the tab's root for now — deep stacks come with
    /// the screens that need them.
    pub fn back(self) {
        let tab = self.tab.get();
        self.route.set(match tab {
            Tab::Songs => Route::Library,
            Tab::Setlists => Route::Setlists,
        });
    }

    /// A screen saying what its own ← or ✕ does, so that the phone's Back key
    /// can do it too.
    ///
    /// Called from a component body, which is where it belongs and where card
    /// K54 just drew that line: the body is for the work that must happen once
    /// when a screen mounts, and displayed values belong in the `{move || …}`
    /// closures below it. This is work of exactly that shape — it happens once,
    /// it produces no pixels, and running it per redraw would rebuild an `Rc`
    /// every frame for a value nothing reads between key presses.
    ///
    /// **Hand it the screen's existing closure, not a copy of what that closure
    /// does.** That is the entire point of the mechanism. Before it, Back was
    /// dead (rinch maps `AK::Back` to `Escape` and swallows it), and the
    /// tempting fix — a `match` on the route inside the interceptor, spelling
    /// out where each screen goes — is a second copy of every screen's exit
    /// rule, sitting in a different file from the first. Two of those rules are
    /// not "go back": the chart editor's ✕ raises a confirm over unsaved typing,
    /// and a Back that skipped it would silently discard a verse somebody typed
    /// by hand into an app with no undo. Capture's ← is a third: its header
    /// arrow and its footer Cancel deliberately mean different things (see the
    /// long note at `crate::screens::capture`'s `back`), and only one of them is
    /// what Back means.
    ///
    /// The route is read untracked and stamped onto the registration; see
    /// [`BackAction`] for why it is there and why the roots are left to miss.
    pub fn register_back(self, action: impl Fn() + 'static) {
        let route = untracked(|| self.route.get());
        self.back_action.set(Some(BackAction { route, run: Rc::new(action) }));
    }

    /// The Android Back key, arriving as `Escape`. Returns what it did.
    ///
    /// Three steps, in this order:
    ///
    /// 1. **An open overlay closes, and nothing else happens.** A sheet is a
    ///    thing in front of the screen, and the first Back has to take the
    ///    front-most thing away — otherwise Back over the library's Sort sheet
    ///    would leave the app while a panel was still on the glass.
    /// 2. **Otherwise the screen's own ← or ✕ runs**, whatever that screen
    ///    decided it means — see [`register_back`](Self::register_back).
    /// 3. **Otherwise there is nowhere left to go**, which on a tab root is
    ///    true, and the caller ends the app. Android's own default Back at the
    ///    root of a task is `finish()`, so leaving is the behaviour a phone
    ///    already teaches; what this must not do is nothing, which is what it
    ///    did before this card.
    ///
    /// ## Why the overlays are checked in this order
    ///
    /// Six of the seven are the bottom sheets `crate::app` mounts as siblings
    /// after the route, and the order below is that mount order **reversed**.
    /// Later siblings paint over earlier ones, so reversed mount order is
    /// front-to-back: this closes whatever is actually on top. The seventh,
    /// `renaming_setlist`, is not a sheet at all — it is an inline field on a
    /// setlist card, under every one of the six — so it goes last.
    ///
    /// In the app as it stands the order cannot be observed, and that is worth
    /// saying rather than leaving as a happy accident: every sheet is opened by
    /// a control that is only reachable when no other sheet is up, because an
    /// open sheet's scrim covers the screen and eats the tap that would open a
    /// second one. So today at most one of these is ever `true`. The order is
    /// written down anyway, and pinned by a test, so that the day a sheet does
    /// open over a sheet the answer is a decision somebody made — close the top
    /// one — rather than whichever field happened to be listed first.
    ///
    /// ## Untracked, deliberately
    ///
    /// Every read below goes through [`untracked`]. This is not a render
    /// closure and it has no observer to subscribe with, so the wrapper changes
    /// nothing today; it is here so that it still changes nothing if this is
    /// ever called from somewhere that does have one. A subscription taken here
    /// would tie the whole of the Back decision — seven sheet signals and the
    /// registered action — to whatever effect happened to be running, and
    /// re-run it on every sheet that opens anywhere in the app.
    pub fn press_back(self) -> BackPress {
        untracked(|| {
            // Front to back; see the header. Each arm closes exactly one thing
            // and returns, because Back is one press and takes away one layer.
            if self.tuning_sheet_open.get() {
                self.tuning_sheet_open.set(false);
                return BackPress::ClosedASheet(Sheet::Tuning);
            }
            if self.running_order_for.get().is_some() {
                self.running_order_for.set(None);
                return BackPress::ClosedASheet(Sheet::RunningOrder);
            }
            if self.picking_songs_for.get().is_some() {
                self.picking_songs_for.set(None);
                return BackPress::ClosedASheet(Sheet::SongPicker);
            }
            if self.add_to_setlist_for.get().is_some() {
                self.add_to_setlist_for.set(None);
                return BackPress::ClosedASheet(Sheet::AddToSetlist);
            }
            if self.filter_sheet_open.get() {
                self.filter_sheet_open.set(false);
                return BackPress::ClosedASheet(Sheet::Filters);
            }
            if self.sort_sheet_open.get() {
                self.sort_sheet_open.set(false);
                return BackPress::ClosedASheet(Sheet::SortAndGroup);
            }
            if self.renaming_setlist.get().is_some() {
                // The card's own ✕ sets this and nothing else — the draft is
                // left where it is, and the next rename overwrites it — so
                // this abandons the edit the same way that ✕ does rather than
                // a tidier way of its own.
                self.renaming_setlist.set(None);
                return BackPress::ClosedASheet(Sheet::RenameSetlist);
            }

            // The `Rc` is cloned out of the signal before it is called, and
            // that is not tidiness. A registered action navigates, which writes
            // `route`, and running it from inside `Signal::with`'s borrow of
            // the signal store is the shape that produced "RefCell already
            // borrowed" for `crate::screens::capture`'s `attach` — see
            // `Flow::take_settled`, which exists for that reason and nothing
            // else.
            let route = self.route.get();
            let action = self.back_action.with(|held| {
                held.as_ref()
                    .filter(|held| held.route == route)
                    .map(|held| Rc::clone(&held.run))
            });
            match action {
                Some(run) => {
                    run();
                    BackPress::RanTheScreensBackAction
                }
                None => BackPress::NothingLeftToDoButExit,
            }
        })
    }
}

impl Default for NavStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    /// Every overlay `press_back` knows about, front to back, paired with the
    /// two things a test needs to do to one: open it, and ask whether it is
    /// still open. Written once here so that the order test below reads as the
    /// order and not as seven near-identical blocks — and so that a sheet added
    /// to `press_back` without a row here shows up as a count mismatch rather
    /// than as a silent gap.
    #[allow(clippy::type_complexity)]
    fn overlays() -> Vec<(Sheet, fn(NavStore), fn(NavStore) -> bool)> {
        vec![
            (
                Sheet::Tuning,
                |nav: NavStore| nav.tuning_sheet_open.set(true),
                |nav: NavStore| nav.tuning_sheet_open.get(),
            ),
            (
                Sheet::RunningOrder,
                |nav: NavStore| nav.running_order_for.set(Some(4)),
                |nav: NavStore| nav.running_order_for.get().is_some(),
            ),
            (
                Sheet::SongPicker,
                |nav: NavStore| nav.picking_songs_for.set(Some(4)),
                |nav: NavStore| nav.picking_songs_for.get().is_some(),
            ),
            (
                Sheet::AddToSetlist,
                |nav: NavStore| nav.add_to_setlist_for.set(Some(9)),
                |nav: NavStore| nav.add_to_setlist_for.get().is_some(),
            ),
            (
                Sheet::Filters,
                |nav: NavStore| nav.filter_sheet_open.set(true),
                |nav: NavStore| nav.filter_sheet_open.get(),
            ),
            (
                Sheet::SortAndGroup,
                |nav: NavStore| nav.sort_sheet_open.set(true),
                |nav: NavStore| nav.sort_sheet_open.get(),
            ),
            (
                Sheet::RenameSetlist,
                |nav: NavStore| nav.renaming_setlist.set(Some(2)),
                |nav: NavStore| nav.renaming_setlist.get().is_some(),
            ),
        ]
    }

    /// A screen that has registered a back action, and a counter that says how
    /// many times Back actually reached it. The `Rc<Cell>` is how the closure
    /// reports back across the `Rc<dyn Fn()>` the store keeps it behind.
    fn screen_with_a_back_action(nav: NavStore) -> Rc<Cell<usize>> {
        nav.route.set(Route::SongDetail(1));
        let ran = Rc::new(Cell::new(0usize));
        let counter = Rc::clone(&ran);
        nav.register_back(move || counter.set(counter.get() + 1));
        ran
    }

    /// A tab root with nothing in front of it has nowhere left to go, and says
    /// so rather than doing nothing — which is what the Back key did before
    /// this card, on every screen in the app.
    #[test]
    fn a_root_with_nothing_open_has_nothing_left_to_do_but_exit() {
        let nav = NavStore::new();
        assert_eq!(nav.press_back(), BackPress::NothingLeftToDoButExit);
    }

    /// The ordinary case: one press, one run of the screen's own ← — not none,
    /// and not two, which is the shape a `Back` wired to both an interceptor
    /// and a handler would have.
    #[test]
    fn a_registered_action_runs_once_per_press() {
        let nav = NavStore::new();
        let ran = screen_with_a_back_action(nav);

        assert_eq!(nav.press_back(), BackPress::RanTheScreensBackAction);
        assert_eq!(ran.get(), 1);

        assert_eq!(nav.press_back(), BackPress::RanTheScreensBackAction);
        assert_eq!(ran.get(), 2);
    }

    /// Every one of the seven, on its own, beats the screen underneath it. This
    /// is the rule that keeps Back from walking out of the app while a sheet is
    /// still on the glass — and the screen's action must not run *as well*,
    /// which is what the counter is checking.
    #[test]
    fn any_open_overlay_takes_the_press_before_the_screen_does() {
        for (sheet, open, still_open) in overlays() {
            let nav = NavStore::new();
            let ran = screen_with_a_back_action(nav);
            open(nav);

            assert_eq!(
                nav.press_back(),
                BackPress::ClosedASheet(sheet),
                "{sheet:?} was open and did not take the press"
            );
            assert!(!still_open(nav), "{sheet:?} took the press and stayed open");
            assert_eq!(ran.get(), 0, "{sheet:?} took the press and the screen ran anyway");

            // And with the sheet gone the very next press reaches the screen,
            // so closing one is a step rather than a swallowed key.
            assert_eq!(nav.press_back(), BackPress::RanTheScreensBackAction);
            assert_eq!(ran.get(), 1);
        }
    }

    /// The order `press_back`'s header claims, pinned. Open all seven at once —
    /// a state the running app cannot reach, because each sheet's scrim eats
    /// the tap that would open the next — and press Back seven times: they have
    /// to come off front-most first, which is `crate::app`'s mount order
    /// reversed, with the inline rename field last because it is not a sheet
    /// and sits under all six.
    #[test]
    fn the_overlays_close_front_most_first() {
        let nav = NavStore::new();
        let ran = screen_with_a_back_action(nav);

        let expected: Vec<Sheet> = overlays()
            .into_iter()
            .map(|(sheet, open, _)| {
                open(nav);
                sheet
            })
            .collect();

        let actual: Vec<Sheet> = expected
            .iter()
            .map(|_| match nav.press_back() {
                BackPress::ClosedASheet(sheet) => sheet,
                other => panic!("expected a sheet to close, got {other:?}"),
            })
            .collect();

        assert_eq!(actual, expected);
        assert_eq!(ran.get(), 0, "a sheet press reached the screen underneath");

        // Seven presses took seven layers off; the eighth is the screen's.
        assert_eq!(nav.press_back(), BackPress::RanTheScreensBackAction);
        assert_eq!(ran.get(), 1);
    }

    /// The registration a screen left behind when it was navigated away from
    /// must not answer for the screen that replaced it.
    ///
    /// This is the case that makes the route stamp on [`BackAction`] worth
    /// having, and it is not hypothetical: a component body runs once and there
    /// is no unmount hook clearing this, so after `song_detail` → ← → library
    /// the store still holds song detail's closure. Without the stamp, Back on
    /// the library root would run `nav.back()` — landing on the library, again
    /// — and the app would have no way out at all, which is the exact bug this
    /// card was written to remove.
    #[test]
    fn a_registration_left_behind_by_a_screen_you_have_left_does_not_answer() {
        let nav = NavStore::new();
        let ran = screen_with_a_back_action(nav);

        // What `nav.back()` from song detail does, without the screen.
        nav.route.set(Route::Library);

        assert_eq!(nav.press_back(), BackPress::NothingLeftToDoButExit);
        assert_eq!(ran.get(), 0);
    }

    /// ...and the same screen registering again is what re-arms it, so the
    /// stamp is a match rather than a one-shot fuse.
    #[test]
    fn returning_to_a_screen_re_arms_its_back_action() {
        let nav = NavStore::new();
        let ran = screen_with_a_back_action(nav);
        nav.route.set(Route::Library);
        assert_eq!(nav.press_back(), BackPress::NothingLeftToDoButExit);

        // The screen mounts again and registers again, the way every component
        // body does when the route swaps back to it.
        let ran_again = screen_with_a_back_action(nav);
        assert_eq!(nav.press_back(), BackPress::RanTheScreensBackAction);
        assert_eq!(ran_again.get(), 1);
        assert_eq!(ran.get(), 0, "the first registration answered a second time");
    }
}

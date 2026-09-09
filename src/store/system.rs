//! What the platform said, the last time anything asked it.
//!
//! Every other store in this directory holds something the *user* decided.
//! This one holds nothing the user decided at all: it is four readings taken
//! off the device — whether the system is in night mode, what colour the
//! wallpaper is, how much of the window the app is not allowed to draw in, and
//! how wide that window is — parked in signals so that a screen can read them
//! the way it reads any other reactive value.
//!
//! ## Why they are signals now, when they were plain function calls before
//!
//! Card K53, and its own note is the shortest statement of the problem:
//! *"`platform::safe_area()` is read once per screen at mount, and
//! `platform::viewport_width()` caches its Android reading in a `OnceLock` for
//! the life of the process. Both are correct today for the same reason: the
//! app is portrait-locked, so neither the insets nor the window size changes
//! underneath it. Neither is correct the moment that stops being true… Why it
//! was not fixed in K31: there is nothing to listen to."*
//!
//! There is now — `rinch::set_configuration_change_handler`, added to the
//! framework for this card — and [`SystemStore::reread`] is what it calls.
//! Card K8 arrived at the same time and added the other two readings, which
//! have exactly the same shelf life for exactly the same reason: a person
//! flips their phone to dark at sunset, or changes their wallpaper, with this
//! app in the foreground.
//!
//! ## Four readings, one store, one re-read
//!
//! They are together rather than each in the store that consumes it because
//! the *event* is one event. Android does not report "the night mode changed";
//! it reports that the configuration changed, and rinch's handler takes no
//! payload for precisely that reason (see
//! `rinch_core::events::ConfigurationChangeHandler`: *"the fields an app cares
//! about are rarely the fields the platform bothered to change"*). One event
//! that means "ask again" wants one place that asks again, and four listeners
//! that each re-read one thing would be four chances to forget one.
//!
//! ## What a re-read reaches, and the one thing it does not
//!
//! Three of the four are signals, so a change repaints whatever reads them,
//! which is the ordinary reactive path and needs nothing else said about it.
//! The fourth — the viewport width — is different in kind, because its readers
//! are not components: `song_detail`'s two page widths, `chart_surface`'s
//! column and `captured_page`'s image sizing all call
//! `platform::viewport_width()` from plain functions. So it is held here as a
//! signal *as well*, for the screens that can read one, and
//! [`platform::forget_cached_readings`] is what corrects the plain calls.
//!
//! **On the pages that were already rasterised at the old width.** K53 warns
//! that "invalidating the cache is not enough on its own", because "every
//! rasterised page column is derived from it, so pages get *cached* at a stale
//! width, not merely laid out at one". That turned out to be the one part of
//! the card's own reasoning that does not hold, and it is worth writing down
//! rather than quietly not doing: **the app has exactly one page cache and it
//! is not derived from the viewport at all.** `pdf::pages` rasterises every
//! page at a fixed `PAGE_WIDTH` of 1080 and says why in its own header
//! ("Why 1080 pixels wide and only one cache" — one render, one set of files,
//! and a card that cannot go stale differently from the viewer), and the
//! viewport width is only ever the box that fixed-size PNG is *fitted into* by
//! `chart_surface::page_box` and `song_detail::card_page_width`. So there is
//! nothing on disk to invalidate: a width change re-fits, it does not re-draw.
//!
//! What a width change genuinely cannot reach is a chart that is *already on
//! screen* at the instant it happens, because `chart_surface` and
//! `captured_page` both take their column once in the component body — for
//! reasons of their own that are about the cost of a database read and an
//! html5ever parse per redraw, not about the width — and a component body
//! runs at mount. That screen is correct again the moment it is re-entered.
//! Forcing it to remount would mean re-parsing a page that can be a megabyte
//! in order to change an `<img>` width on a device whose window cannot change
//! size while portrait-locked in the first place, and that trade is not worth
//! making until there is a device where it is.

use rinch::prelude::*;

use crate::platform::{self, SafeArea};
use crate::theme::Rgb;

/// One complete look at the platform — all four readings, taken together.
///
/// A struct rather than four arguments so that [`SystemStore::apply`] cannot
/// be called with three of them, and so that a test can hand over a whole
/// pretend device without a window to read one from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    /// `Some(true)` for night, `Some(false)` for day, `None` when the platform
    /// declined to say — see [`platform::night_mode`], where that third state
    /// is argued at length.
    pub night: Option<bool>,
    /// The wallpaper's primary colour, raw. `None` on any device with no
    /// wallpaper colours to publish, which is the common case and not a fault.
    pub wallpaper: Option<Rgb>,
    pub safe_area: SafeArea,
    pub viewport_width: f32,
}

impl Reading {
    /// The platform, right now.
    pub fn take() -> Self {
        Self {
            night: platform::night_mode(),
            wallpaper: platform::wallpaper_primary(),
            safe_area: platform::safe_area(),
            viewport_width: platform::viewport_width(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct SystemStore {
    pub night: Signal<Option<bool>>,
    pub wallpaper: Signal<Option<Rgb>>,
    pub safe_area: Signal<SafeArea>,
    pub viewport_width: Signal<f32>,
}

impl SystemStore {
    /// The store, filled from the device the app is actually running on.
    pub fn read() -> Self {
        Self::holding(Reading::take())
    }

    /// The store, filled from a reading somebody else took. The seam every
    /// test in this file goes through, and the reason none of them needs a
    /// window.
    pub fn holding(reading: Reading) -> Self {
        Self {
            night: Signal::new(reading.night),
            wallpaper: Signal::new(reading.wallpaper),
            safe_area: Signal::new(reading.safe_area),
            viewport_width: Signal::new(reading.viewport_width),
        }
    }

    /// Overwrite every signal with a fresh reading.
    ///
    /// A plain method over `Copy` handles, taking the reading as an argument
    /// rather than going and getting one, and that shape is deliberate: it is
    /// the same move `NavStore::press_back` makes, for the same reason. A
    /// function that reaches out to the platform itself can only be checked on
    /// a phone; one that is handed what the platform said can be checked at
    /// `cargo test` time on a laptop, which is card K15's rule and the only
    /// way anything in this file is pinned at all.
    pub fn apply(self, reading: Reading) {
        self.night.set(reading.night);
        self.wallpaper.set(reading.wallpaper);
        self.safe_area.set(reading.safe_area);
        self.viewport_width.set(reading.viewport_width);
    }

    /// Ask again, through `take`, and store whatever comes back.
    ///
    /// Generic over the source purely so that a test can count how many times
    /// it was asked — which is the property that matters and is not the same
    /// property as "the signals hold the right values". A handler that had
    /// quietly memoised its reading would pass every value assertion and fail
    /// this one.
    pub fn refresh(self, take: impl Fn() -> Reading) {
        self.apply(take());
    }

    /// The whole of what the app does when the platform says its configuration
    /// moved. Registered once, in `crate::app`.
    ///
    /// The cache is dropped **before** the reading is taken, not after: the
    /// `viewport_width` this is about to store comes from
    /// `platform::viewport_width()`, and asking that before forgetting would
    /// store the stale number and then clear the cache that produced it,
    /// leaving the signal a whole configuration change behind.
    pub fn reread(self) {
        platform::forget_cached_readings();
        self.refresh(Reading::take);
    }
}

impl Default for SystemStore {
    fn default() -> Self {
        Self::read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    fn day() -> Reading {
        Reading {
            night: Some(false),
            wallpaper: None,
            safe_area: SafeArea::PHONE_STANDIN,
            viewport_width: 393.0,
        }
    }

    fn night_on_a_wider_phone() -> Reading {
        Reading {
            night: Some(true),
            wallpaper: Some(Rgb::new(0x3F, 0x51, 0xB5)),
            safe_area: SafeArea {
                top: 30.0,
                right: 0.0,
                bottom: 12.0,
                left: 0.0,
            },
            viewport_width: 432.0,
        }
    }

    /// All four, in one call, because the event that causes it is one event.
    /// A version of `apply` that updated three of them would look right on the
    /// phone for as long as nobody rotated it.
    #[test]
    fn a_fresh_reading_replaces_every_one_of_the_four() {
        let system = SystemStore::holding(day());
        system.apply(night_on_a_wider_phone());

        assert_eq!(system.night.get(), Some(true));
        assert_eq!(system.wallpaper.get(), Some(Rgb::new(0x3F, 0x51, 0xB5)));
        assert_eq!(system.safe_area.get().top, 30.0);
        assert_eq!(system.viewport_width.get(), 432.0);
    }

    /// The card's own requirement: *the configuration-change handler actually
    /// re-reads rather than returning a cached value*. Asserted by counting
    /// the asks and changing the answer between them, so that a handler which
    /// held on to its first reading would fail here even though every signal
    /// it set was correct at the time it set it.
    #[test]
    fn every_refresh_asks_the_platform_again_rather_than_reusing_what_it_had() {
        let system = SystemStore::holding(day());
        let asks = Rc::new(Cell::new(0u32));

        let counted = {
            let asks = Rc::clone(&asks);
            move || {
                asks.set(asks.get() + 1);
                // A different answer every time, so "it re-read" and "it
                // happened to still be right" cannot be confused.
                if asks.get() % 2 == 0 {
                    night_on_a_wider_phone()
                } else {
                    day()
                }
            }
        };

        system.refresh(&counted);
        assert_eq!(asks.get(), 1);
        assert_eq!(system.night.get(), Some(false));

        system.refresh(&counted);
        assert_eq!(asks.get(), 2, "the second change re-read the platform");
        assert_eq!(system.night.get(), Some(true), "and the app followed it");

        system.refresh(&counted);
        assert_eq!(asks.get(), 3);
        assert_eq!(system.night.get(), Some(false), "and followed it back again");
    }

    /// A device that declines to say — an undefined `uiMode`, a live wallpaper
    /// with no colours, a JNI call that failed — is a reading like any other
    /// and must not be mistaken for "no change". Both fields go back to `None`
    /// rather than keeping the last thing that was true.
    #[test]
    fn a_platform_that_stops_answering_clears_the_readings_rather_than_keeping_them() {
        let system = SystemStore::holding(night_on_a_wider_phone());
        system.apply(Reading {
            night: None,
            wallpaper: None,
            ..night_on_a_wider_phone()
        });

        assert_eq!(system.night.get(), None);
        assert_eq!(system.wallpaper.get(), None);
    }

    /// On a laptop `Reading::take` is the desktop half of every `platform`
    /// shim, and this is what it comes back with. It is also the reading every
    /// `cargo test` in this crate that builds a store gets, so it is worth
    /// naming rather than leaving as a thing four other test modules assume.
    #[test]
    fn the_desktop_reads_as_a_phone_shaped_window_with_no_system_theme() {
        let reading = Reading::take();
        assert_eq!(reading.night, None);
        assert_eq!(reading.wallpaper, None);
        assert_eq!(reading.safe_area, SafeArea::PHONE_STANDIN);
        assert_eq!(reading.viewport_width, crate::WIDTH as f32);
    }
}

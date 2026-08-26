//! The one place in this app that opens a socket.
//!
//! Everything goes through [`rinch_http`] rather than `ureq` directly. That is
//! the plan's call and it is not cosmetic: rinch-http is one API over `ureq`
//! natively and the browser's `fetch` on wasm, so the capture engine names no
//! transport and this file is the only thing that would change if the app ever
//! grew a third target.
//!
//! [`Fetcher`] exists so the rest of the module can be tested without a
//! network. Sanitising, asset rewriting and reader extraction are the parts
//! most likely to be wrong, and none of them should need wifi to prove.
//!
//! ## What rinch-http does not give us
//!
//! Three limits are inherited rather than chosen, and each is written up in
//! `docs/CAPTURE.md`:
//!
//! * **Timeouts are fixed** at 30s connect / 60s overall. `Config` is built
//!   inside `rinch-http` and nothing is exposed to override it.
//! * **The body ceiling is 10 MiB**, from ureq's `read_to_vec`. A larger page
//!   surfaces as `HttpError::Body`, not as a truncated capture. Our own
//!   [`Limits`](super::Limits) can only be *stricter* than that, and is
//!   applied after the bytes are already in memory — there is no way to abort
//!   a download part-way through this API.
//! * **The final URL after redirects is not reported.** `Response` carries the
//!   status, the headers and the body; ureq followed the redirects and did not
//!   say where it ended up. Relative asset URLs are therefore resolved against
//!   `<base href>` if the page has one and against the *requested* URL if not,
//!   which is wrong for a page that redirects across hosts. See
//!   `super::assets::base_url`.

use super::{Failure, Limits};

/// What came back. Deliberately not `rinch_http::Response`: the rest of the
/// module should not be able to see a header it might be tempted to trust.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub status: u16,
    /// Lower-cased, parameters and all (`text/html; charset=utf-8`).
    pub content_type: String,
    pub body: Vec<u8>,
}

impl Fetched {
    pub fn is_html(&self) -> bool {
        self.content_type.is_empty()
            || self.content_type.starts_with("text/html")
            || self.content_type.starts_with("application/xhtml")
    }

    pub fn is_image(&self) -> bool {
        self.content_type.starts_with("image/")
    }
}

/// A source of bytes for a URL.
///
/// Blocking on purpose. A capture is a page fetch followed by up to a couple
/// of dozen image fetches that depend on it, which is a sequence, not a fan of
/// independent callbacks — so the engine runs start to finish on one worker
/// thread and reports progress from there. The alternative, threading
/// `rinch_http::fetch`'s main-thread callback through every step, would turn a
/// readable loop into a state machine for no gain on the two platforms this
/// app ships to.
pub trait Fetcher {
    fn get(&self, url: &str) -> Result<Fetched, Failure>;
}

/// The real one.
#[cfg(not(target_arch = "wasm32"))]
pub struct HttpFetcher {
    user_agent: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl HttpFetcher {
    pub fn new(limits: &Limits) -> Self {
        Self {
            user_agent: limits.user_agent.clone(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Fetcher for HttpFetcher {
    fn get(&self, url: &str) -> Result<Fetched, Failure> {
        // `Credentials::Omit` gets a throwaway ureq agent with an empty cookie
        // jar. That is the point: this app has no session with anybody, and a
        // capture must not carry a cookie one site set into a request to the
        // next. It costs the connection pool, which for a handful of requests
        // is nothing.
        let request = rinch_http::Request::get(url)
            .credentials(rinch_http::Credentials::Omit)
            .header("User-Agent", &self.user_agent)
            .header("Accept", "text/html,application/xhtml+xml,image/*;q=0.8,*/*;q=0.5")
            .header("Accept-Language", "en")
            // Ask for no compression. ureq transparently inflates gzip, but
            // Brotli is not in its feature set here and a br-encoded body
            // would arrive as bytes that parse as nothing at all — a silent
            // empty capture rather than an honest failure.
            .header("Accept-Encoding", "gzip, identity");

        let response = rinch_http::fetch_blocking(request).map_err(|e| match e {
            rinch_http::HttpError::InvalidRequest(m) => Failure::NotAUrl(m),
            rinch_http::HttpError::Network(m) => Failure::Unreachable(m),
            // `Body` covers two unrelated things — the 10 MiB ceiling and a
            // read that ran past the 60s global timeout — and they are
            // different sentences on E4's screen. The string is all
            // rinch-http gives us to tell them apart.
            rinch_http::HttpError::Body(m) if m.contains("timeout") => Failure::Unreachable(m),
            rinch_http::HttpError::Body(m) => Failure::TooLarge(m),
        })?;

        Ok(Fetched {
            content_type: response
                .header("content-type")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase(),
            status: response.status,
            body: response.body,
        })
    }
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use std::collections::HashMap;

    /// A canned network. Every test in this module that needs a page uses one,
    /// so the suite runs with the wifi off.
    #[derive(Default)]
    pub struct Canned {
        pages: HashMap<String, Fetched>,
    }

    impl Canned {
        pub fn html(mut self, url: &str, body: &str) -> Self {
            self.pages.insert(
                url.to_string(),
                Fetched {
                    status: 200,
                    content_type: "text/html; charset=utf-8".into(),
                    body: body.as_bytes().to_vec(),
                },
            );
            self
        }

        pub fn image(mut self, url: &str, bytes: &[u8]) -> Self {
            self.pages.insert(
                url.to_string(),
                Fetched {
                    status: 200,
                    content_type: "image/png".into(),
                    body: bytes.to_vec(),
                },
            );
            self
        }

        pub fn status(mut self, url: &str, status: u16, body: &str) -> Self {
            self.pages.insert(
                url.to_string(),
                Fetched {
                    status,
                    content_type: "text/html".into(),
                    body: body.as_bytes().to_vec(),
                },
            );
            self
        }
    }

    impl Fetcher for Canned {
        fn get(&self, url: &str) -> Result<Fetched, Failure> {
            self.pages
                .get(url)
                .cloned()
                .ok_or_else(|| Failure::Unreachable(format!("nothing canned at {url}")))
        }
    }
}

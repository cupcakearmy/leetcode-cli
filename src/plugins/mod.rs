//! Leetcode-cli plugins
//!
//! + browser cookie parsers (Chrome, Firefox)
//! + leetcode API
//!
//! ## login to `leetcode.com`
//! Leetcode-cli reads the browser cookies directly, do not need to login, please make sure you have logged in `leetcode.com` in Chrome or Firefox before using `leetcode-cli`
//!

// FIXME: Read cookies from local storage. (issue #122)
mod chrome;
mod firefox;
mod leetcode;
pub use leetcode::LeetCode;

/// Resolve cookies: config first, then Chrome, then Firefox.
pub(crate) fn cookies() -> crate::Result<chrome::Ident> {
    let ccfg = crate::config::Config::locate()?.cookies;
    if !ccfg.csrf.is_empty() && !ccfg.session.is_empty() {
        return Ok(chrome::Ident {
            csrf: ccfg.csrf,
            session: ccfg.session,
        });
    }

    let domain = ccfg.site.to_string();
    if let Ok(jar) = chrome::cookies(&domain)
        && let Ok(ident) = chrome::Ident::from_jar(&jar)
    {
        return Ok(ident);
    }
    chrome::Ident::from_jar(&firefox::cookies(&domain)?)
}

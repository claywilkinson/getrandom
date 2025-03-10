// Copyright 2019 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Interface to the random number generator of the operating system.
//!
//! # Platform sources
//!
//! | OS               | interface
//! |------------------|---------------------------------------------------------
//! | Linux, Android   | [`getrandom`][1] system call if available, otherwise [`/dev/urandom`][2] after successfully polling `/dev/random`
//! | Windows          | [`RtlGenRandom`][3]
//! | macOS            | [`getentropy()`][19] if available, otherwise [`/dev/random`][20] (identical to `/dev/urandom`)
//! | iOS              | [`SecRandomCopyBytes`][4]
//! | FreeBSD          | [`getrandom()`][21] if available, otherwise [`kern.arandom`][5]
//! | OpenBSD          | [`getentropy`][6]
//! | NetBSD           | [`/dev/urandom`][7] after successfully polling `/dev/random`
//! | Dragonfly BSD    | [`/dev/random`][8]
//! | Solaris, illumos | [`getrandom`][9] system call if available, otherwise [`/dev/random`][10]
//! | Fuchsia OS       | [`cprng_draw`][11]
//! | Redox            | [`rand:`][12]
//! | CloudABI         | [`cloudabi_sys_random_get`][13]
//! | Haiku            | `/dev/random` (identical to `/dev/urandom`)
//! | L4RE, SGX, UEFI  | [RDRAND][18]
//! | Hermit           | [RDRAND][18] as [`sys_rand`][22] is currently broken.
//! | Web browsers     | [`Crypto.getRandomValues`][14] (see [Support for WebAssembly and ams.js][14])
//! | Node.js          | [`crypto.randomBytes`][15] (see [Support for WebAssembly and ams.js][16])
//! | WASI             | [`__wasi_random_get`][17]
//!
//! Getrandom doesn't have a blanket implementation for all Unix-like operating
//! systems that reads from `/dev/urandom`. This ensures all supported operating
//! systems are using the recommended interface and respect maximum buffer
//! sizes.
//!
//! ## Support for WebAssembly and ams.js
//!
//! The three Emscripten targets `asmjs-unknown-emscripten`,
//! `wasm32-unknown-emscripten` and `wasm32-experimental-emscripten` use
//! Emscripten's emulation of `/dev/random` on web browsers and Node.js.
//!
//! The bare WASM target `wasm32-unknown-unknown` tries to call the javascript
//! methods directly, using either `stdweb` or `wasm-bindgen` depending on what
//! features are activated for this crate. Note that if both features are
//! enabled `wasm-bindgen` will be used. If neither feature is enabled,
//! `getrandom` will always fail.
//!
//! The WASI target `wasm32-wasi` uses the `__wasi_random_get` function defined
//! by the WASI standard.
//!
//!
//! ## Early boot
//!
//! It is possible that early in the boot process the OS hasn't had enough time
//! yet to collect entropy to securely seed its RNG, especially on virtual
//! machines.
//!
//! Some operating systems always block the thread until the RNG is securely
//! seeded. This can take anywhere from a few seconds to more than a minute.
//! Others make a best effort to use a seed from before the shutdown and don't
//! document much.
//!
//! A few, Linux, NetBSD and Solaris, offer a choice between blocking and
//! getting an error; in these cases we always choose to block.
//!
//! On Linux (when the `genrandom` system call is not available) and on NetBSD
//! reading from `/dev/urandom` never blocks, even when the OS hasn't collected
//! enough entropy yet. To avoid returning low-entropy bytes, we first read from
//! `/dev/random` and only switch to `/dev/urandom` once this has succeeded.
//!
//! # Error handling
//!
//! We always choose failure over returning insecure "random" bytes. In general,
//! on supported platforms, failure is highly unlikely, though not impossible.
//! If an error does occur, then it is likely that it will occur on every call to
//! `getrandom`, hence after the first successful call one can be reasonably
//! confident that no errors will occur.
//!
//! On unsupported platforms, `getrandom` always fails. See the [`Error`] type
//! for more information on what data is returned on failure.
//!
//! [1]: http://man7.org/linux/man-pages/man2/getrandom.2.html
//! [2]: http://man7.org/linux/man-pages/man4/urandom.4.html
//! [3]: https://docs.microsoft.com/en-us/windows/desktop/api/ntsecapi/nf-ntsecapi-rtlgenrandom
//! [4]: https://developer.apple.com/documentation/security/1399291-secrandomcopybytes?language=objc
//! [5]: https://www.freebsd.org/cgi/man.cgi?query=random&sektion=4
//! [6]: https://man.openbsd.org/getentropy.2
//! [7]: http://netbsd.gw.com/cgi-bin/man-cgi?random+4+NetBSD-current
//! [8]: https://leaf.dragonflybsd.org/cgi/web-man?command=random&section=4
//! [9]: https://docs.oracle.com/cd/E88353_01/html/E37841/getrandom-2.html
//! [10]: https://docs.oracle.com/cd/E86824_01/html/E54777/random-7d.html
//! [11]: https://fuchsia.googlesource.com/fuchsia/+/master/zircon/docs/syscalls/cprng_draw.md
//! [12]: https://github.com/redox-os/randd/blob/master/src/main.rs
//! [13]: https://github.com/nuxinl/cloudabi#random_get
//! [14]: https://www.w3.org/TR/WebCryptoAPI/#Crypto-method-getRandomValues
//! [15]: https://nodejs.org/api/crypto.html#crypto_crypto_randombytes_size_callback
//! [16]: #support-for-webassembly-and-amsjs
//! [17]: https://github.com/WebAssembly/WASI/blob/master/design/WASI-core.md#__wasi_random_get
//! [18]: https://software.intel.com/en-us/articles/intel-digital-random-number-generator-drng-software-implementation-guide
//! [19]: https://www.unix.com/man-page/mojave/2/getentropy/
//! [20]: https://www.unix.com/man-page/mojave/4/random/
//! [21]: https://www.freebsd.org/cgi/man.cgi?query=getrandom&manpath=FreeBSD+12.0-stable
//! [22]: https://github.com/hermitcore/libhermit-rs/blob/09c38b0371cee6f56a541400ba453e319e43db53/src/syscalls/random.rs#L21

#![doc(
    html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk.png",
    html_favicon_url = "https://www.rust-lang.org/favicon.ico",
    html_root_url = "https://rust-random.github.io/rand/"
)]
#![no_std]
#![cfg_attr(feature = "stdweb", recursion_limit = "128")]

#[macro_use]
extern crate cfg_if;

cfg_if! {
    if #[cfg(feature = "log")] {
        #[allow(unused)]
        #[macro_use]
        extern crate log;
    } else {
        #[allow(unused)]
        macro_rules! error {
            ($($x:tt)*) => {};
        }
    }
}

mod error;
pub use crate::error::Error;

#[allow(dead_code)]
mod util;
// Unlike the other Unix, Fuchsia and iOS don't use the libc to make any calls.
#[cfg(any(
    target_os = "android",
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "freebsd",
    target_os = "haiku",
    target_os = "illumos",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "solaris",
))]
#[allow(dead_code)]
mod util_libc;

// std-only trait definitions
#[cfg(feature = "std")]
mod error_impls;

// These targets read from a file as a fallback method.
#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "solaris",
    target_os = "illumos",
))]
mod use_file;

// System-specific implementations.
//
// These should all provide getrandom_inner with the same signature as getrandom.
cfg_if! {
    if #[cfg(target_os = "android")] {
        #[path = "linux_android.rs"] mod imp;
    } else if #[cfg(target_os = "cloudabi")] {
        #[path = "cloudabi.rs"] mod imp;
    } else if #[cfg(target_os = "dragonfly")] {
        #[path = "use_file.rs"] mod imp;
    } else if #[cfg(target_os = "emscripten")] {
        #[path = "use_file.rs"] mod imp;
    } else if #[cfg(target_os = "freebsd")] {
        #[path = "freebsd.rs"] mod imp;
    } else if #[cfg(target_os = "fuchsia")] {
        #[path = "fuchsia.rs"] mod imp;
    } else if #[cfg(target_os = "haiku")] {
        #[path = "use_file.rs"] mod imp;
    } else if #[cfg(target_os = "illumos")] {
        #[path = "solaris_illumos.rs"] mod imp;
    } else if #[cfg(target_os = "ios")] {
        #[path = "ios.rs"] mod imp;
    } else if #[cfg(target_os = "linux")] {
        #[path = "linux_android.rs"] mod imp;
    } else if #[cfg(target_os = "macos")] {
        #[path = "macos.rs"] mod imp;
    } else if #[cfg(target_os = "netbsd")] {
        #[path = "use_file.rs"] mod imp;
    } else if #[cfg(target_os = "openbsd")] {
        #[path = "openbsd.rs"] mod imp;
    } else if #[cfg(target_os = "redox")] {
        #[path = "use_file.rs"] mod imp;
    } else if #[cfg(target_os = "solaris")] {
        #[path = "solaris_illumos.rs"] mod imp;
    } else if #[cfg(target_os = "wasi")] {
        #[path = "wasi.rs"] mod imp;
    } else if #[cfg(windows)] {
        #[path = "windows.rs"] mod imp;
    } else if #[cfg(all(target_arch = "x86_64", any(
                  target_os = "hermit",
                  target_os = "l4re",
                  target_os = "uefi",
                  target_env = "sgx",
              )))] {
        #[path = "rdrand.rs"] mod imp;
    } else if #[cfg(target_arch = "wasm32")] {
        cfg_if! {
            if #[cfg(feature = "wasm-bindgen")] {
                #[path = "wasm32_bindgen.rs"] mod imp;
            } else if #[cfg(feature = "stdweb")] {
                #[path = "wasm32_stdweb.rs"] mod imp;
            } else {
                #[path = "dummy.rs"] mod imp;
            }
        }
    } else {
        #[path = "dummy.rs"] mod imp;
    }
}

/// Fill `dest` with random bytes from the system's preferred random number
/// source.
///
/// This function returns an error on any failure, including partial reads. We
/// make no guarantees regarding the contents of `dest` on error.
///
/// Blocking is possible, at least during early boot; see module documentation.
///
/// In general, `getrandom` will be fast enough for interactive usage, though
/// significantly slower than a user-space CSPRNG; for the latter consider
/// [`rand::thread_rng`](https://docs.rs/rand/*/rand/fn.thread_rng.html).
pub fn getrandom(dest: &mut [u8]) -> Result<(), error::Error> {
    imp::getrandom_inner(dest)
}

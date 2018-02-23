//! ## About Honggfuzz
//! 
//! Honggfuzz is a security oriented fuzzer with powerful analysis options. Supports evolutionary, feedback-driven fuzzing based on code coverage (software- and hardware-based).
//! 
//! * project homepage [honggfuzz.com](http://honggfuzz.com/)
//! * project repository [github.com/google/honggfuzz](https://github.com/google/honggfuzz)
//! * this upstream project is maintained by Google, but ...
//! * this is NOT an official Google product
//! 
//! ## Compatibility
//! 
//! * __Rust__: stable, beta, nightly
//! * __OS__: GNU/Linux, macOS, FreeBSD, Android, WSL (Windows Subsystem for Linux)
//! * __Arch__: x86_64, x86, arm64-v8a, armeabi-v7a, armeabi
//! * __Sanitizer__: none, address, thread, leak 
//! 
//! ## How to use this crate
//! 
//! Install honggfuzz commands to build with instrumentation and fuzz
//! 
//! ```sh
//! # installs hfuzz and honggfuzz subcommands in cargo
//! cargo install honggfuzz
//! ```
//! 
//! Add to your dependencies
//! 
//! ```toml
//! [dependencies]
//! honggfuzz = "0.5"
//! ```
//! 
//! Create a target to fuzz
//! 
//! ```rust
//! #[macro_use] extern crate honggfuzz;
//! 
//! fn main() {
//!     // Here you can parse `std::env::args and 
//!     // setup / initialize your project
//! 
//!     // You have full control over the loop but
//!     // you're supposed to call `fuzz` ad vitam aeternam
//!     loop {
//!         // The fuzz macro gives an arbitrary object (see `arbitrary crate`)
//!         // to a closure-like block of code.
//!         // For performance reasons, it is recommended that you use the native type
//!         // `&[u8]` when possible.
//!         // Here, this slice will contain a "random" quantity of "random" data.
//!         fuzz!(|data: &[u8]| {
//!             if data.len() != 6 {return}
//!             if data[0] != b'q' {return}
//!             if data[1] != b'w' {return}
//!             if data[2] != b'e' {return}
//!             if data[3] != b'r' {return}
//!             if data[4] != b't' {return}
//!             if data[5] != b'y' {return}
//!             panic!("BOOM")
//!         });
//!     }
//! }
//! 
//! ```
//! 
//! Fuzz for fun and profit !
//! 
//! ```sh
//! # builds with fuzzing instrumentation and then runs the "example" target
//! cargo hfuzz run example
//! ```
//! 
//! Once you got a crash, replay it easily in a debug environment
//! 
//! ```sh
//! # builds the target in debug mode and replays automatically the crash in gdb
//! cargo hfuzz run-debug example fuzzing_workspace/*.fuzz
//! ```
//! 
//! Clean
//! 
//! ```sh
//! # a wrapper on "cargo clean" which cleans the fuzzing_target directory
//! cargo hfuzz clean 
//! ```
//! 
//! ### Environment variables
//! 
//! #### `RUSTFLAGS`
//! 
//! You can use `RUSTFLAGS` to send additional arguments to `rustc`.
//! 
//! For instance, you can enable the use of LLVM's [sanitizers](https://github.com/japaric/rust-san).
//! This is a recommended option if you want to test your `unsafe` rust code but it will have an impact on performance.
//! 
//! ```sh
//! RUSTFLAGS="-Z sanitizer=address" cargo hfuzz run example
//! ```
//! 
//! #### `HFUZZ_BUILD_ARGS`
//! 
//! You can use `HFUZZ_BUILD_ARGS` to send additional arguments to `cargo build`.
//! 
//! #### `HFUZZ_RUN_ARGS`
//! 
//! You can use `HFUZZ_RUN_ARGS` to send additional arguments to `honggfuzz`.
//! See [USAGE](https://github.com/google/honggfuzz/blob/master/docs/USAGE.md) for the list of those.
//! 
//! For example:
//! 
//! ```sh
//! # 1 second of timeout
//! # use 12 fuzzing thread
//! # be verbose
//! # stop after 1000000 fuzzing iteration
//! # exit upon crash
//! HFUZZ_RUN_ARGS="-t 1 -n 12 -v -N 1000000 --exit_upon_crash" cargo hfuzz run example
//! ```
//! 
//! ## Relevant documentation about honggfuzz
//! * [USAGE](https://github.com/google/honggfuzz/blob/master/docs/USAGE.md)
//! * [FeedbackDrivenFuzzing](https://github.com/google/honggfuzz/blob/master/docs/FeedbackDrivenFuzzing.md)
//! * [PersistentFuzzing](https://github.com/google/honggfuzz/blob/master/docs/PersistentFuzzing.md)
//! 
//! ## About Rust fuzzing
//!  
//! There is other projects providing Rust fuzzing support at [github.com/rust-fuzz](https://github.com/rust-fuzz). 
//!  
//! You'll find support for [AFL](https://github.com/rust-fuzz/afl.rs) and LLVM's [LibFuzzer](https://github.com/rust-fuzz/cargo-fuzz) and there is also a [trophy case](https://github.com/rust-fuzz/trophy-case) ;-) .
//! 
//! This crate was inspired by those projects!

#[cfg(all(fuzzing, fuzzing_debug))]
extern crate memmap;

#[cfg(all(fuzzing, not(fuzzing_debug)))]
extern "C" {
    fn HF_ITER(buf_ptr: *mut *const u8, len_ptr: *mut usize );
}

/// Fuzz a closure by passing it a `&[u8]`
///
/// This slice contains a "random" quantity of "random" data.
///
/// For perstistent fuzzing to work, you have to call it ad vita aeternam in an infinite loop.
///
/// ```
/// extern crate honggfuzz;
/// use honggfuzz::fuzz;
///
/// loop {
///     fuzz(|data|{
///         if data.len() != 10 {return}
///         if data[0] != b'q' {return}
///         if data[1] != b'w' {return}
///         if data[2] != b'e' {return}
///         if data[3] != b'r' {return}
///         if data[4] != b't' {return}
///         if data[5] != b'y' {return}
///         if data[6] != b'u' {return}
///         if data[7] != b'i' {return}
///         if data[8] != b'o' {return}
///         if data[9] != b'p' {return}
///         panic!("BOOM")
///     });
/// }
/// ```
#[cfg(not(fuzzing))]
#[allow(unused_variables)]
pub fn fuzz<F>(closure: F) where F: Fn(&[u8]) {
    eprintln!("This executable hasn't been built with honggfuzz instrumentation.");
    eprintln!("Try executing \"cargo hfuzz build\" and check out \"hfuzz_target\" directory.");
    eprintln!("Or execute \"cargo hfuzz run TARGET\"");
    std::process::exit(17);
}

#[cfg(all(fuzzing, not(fuzzing_debug)))]
pub fn fuzz<F>(closure: F) where F: Fn(&[u8]) {
    let buf;
    unsafe {
        let mut buf_ptr: *const u8 = std::mem::uninitialized();
        let mut len_ptr: usize = std::mem::uninitialized();
        HF_ITER(&mut buf_ptr, &mut len_ptr);
        buf = ::std::slice::from_raw_parts(buf_ptr, len_ptr);
    }
    closure(buf);
}

#[cfg(all(fuzzing, fuzzing_debug))]
pub fn fuzz<F>(closure: F) where F: Fn(&[u8]) {
    use std::env;
    use std::fs::File;
    use memmap::MmapOptions;
    
    let filename = env::var("CARGO_HONGGFUZZ_CRASH_FILENAME").unwrap_or_else(|_|{
        eprintln!("error: Environment variable CARGO_HONGGFUZZ_CRASH_FILENAME not set. Try launching with \"cargo hfuzz run-debug TARGET CRASH_FILENAME [ ARGS ... ]\"");
        std::process::exit(1)
    });

    let file = File::open(&filename).unwrap_or_else(|_|{
        eprintln!("error: failed to open \"{}\"", &filename);
        std::process::exit(1)
    });

    let mmap = unsafe {MmapOptions::new().map(&file)}.unwrap_or_else(|_|{
        eprintln!("error: failed to mmap file \"{}\"", &filename);
        std::process::exit(1)
    });

    closure(&mmap);
}

/// Fuzz a closure-like block of code by passing it an object of arbitrary type.
///
/// You can choose the type of the argument using the syntax as in the example below.
/// Please check out the `arbitrary` crate to see which types are available.
///
/// For performance reasons, it is recommended that you use the native type `&[u8]` when possible.
///
/// For perstistent fuzzing to work, you have to call it ad vita aeternam in an infinite loop.
///
/// ```
/// #[macro_use] extern crate honggfuzz;
///
/// loop {
///     fuzz!(|data: &[u8]| {
///         if data.len() != 10 {return}
///         if data[0] != b'q' {return}
///         if data[1] != b'w' {return}
///         if data[2] != b'e' {return}
///         if data[3] != b'r' {return}
///         if data[4] != b't' {return}
///         if data[5] != b'y' {return}
///         if data[6] != b'u' {return}
///         if data[7] != b'i' {return}
///         if data[8] != b'o' {return}
///         if data[9] != b'p' {return}
///         panic!("BOOM")
///     });
/// }
/// ```
#[cfg(not(fuzzing))]
#[macro_export]
macro_rules! fuzz {
    (|$buf:ident| $body:block) => {
        honggfuzz::fuzz(|_| {});
    };
    (|$buf:ident: &[u8]| $body:block) => {
        honggfuzz::fuzz(|_| {});
    };
    (|$buf:ident: $dty: ty| $body:block) => {
        honggfuzz::fuzz(|_| {});
    };
}

#[cfg(all(fuzzing))]
#[macro_export]
macro_rules! fuzz {
    (|$buf:ident| $body:block) => {
        honggfuzz::fuzz(|$buf| $body);
    };
    (|$buf:ident: &[u8]| $body:block) => {
        honggfuzz::fuzz(|$buf| $body);
    };
    (|$buf:ident: $dty: ty| $body:block) => {
        honggfuzz::fuzz(|$buf| {
            let $buf: $dty = {
                use arbitrary::{Arbitrary, RingBuffer};
                if let Ok(d) = RingBuffer::new($buf, $buf.len()).and_then(|mut b|{
                        Arbitrary::arbitrary(&mut b).map_err(|_| "")
                    }) {
                    d
                } else {
                    return
                }
            };

            $body
        });
    };
}


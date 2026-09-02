//! Rust ignores `SIGPIPE`, so a write to a pipe whose reader has gone away
//! fails with `EPIPE` and `println!` turns that into a panic. Piping any of
//! the listing commands into `head` is enough to trigger it:
//!
//! ```text
//! $ streamdeck-ctl deck show | head -1
//! thread 'main' panicked at library/std/src/io/stdio.rs:
//! failed printing to stdout: Broken pipe (os error 32)
//! ```
//!
//! The one-shot commands should behave like `cat` or `ls` and stop quietly
//! instead. The daemons keep the Rust default: they log to the journal under
//! `Restart=on-failure`, so exiting zero would stop systemd restarting them.

/// Stop quietly when the reader of our stdout closes the pipe early.
pub fn quiet_on_broken_pipe() {
    // SAFETY: the handler only calls `signal_hook::low_level::exit`, which is
    // `_exit(2)` and async-signal-safe. Registering replaces the `SIG_IGN`
    // that Rust installs at startup, so the signal is delivered rather than
    // surfacing as an `EPIPE` write error.
    let registered = unsafe {
        signal_hook::low_level::register(signal_hook::consts::SIGPIPE, || {
            signal_hook::low_level::exit(0)
        })
    };
    // Nothing useful to do if the kernel refuses; the panic is no worse than
    // failing here would be.
    let _ = registered;
}

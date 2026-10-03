pub fn quiet_on_broken_pipe() {
    let _ = unsafe {
        signal_hook::low_level::register(signal_hook::consts::SIGPIPE, || {
            signal_hook::low_level::exit(0)
        })
    };
}

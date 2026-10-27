fn main() {
    // A closed pipe (`ccs library list | head`) ends ccs quietly, as it ends `ls`.
    // Rust ignores SIGPIPE by default, which makes `println!` panic instead.
    // SAFETY: restores the default action of one signal, before any thread starts.
    unsafe { signal(SIGPIPE, SIG_DFL) };
    std::process::exit(ccs::run(std::env::args().skip(1).collect()));
}

/// SIGPIPE is 13 on macOS and Linux, the platforms ccs supports.
const SIGPIPE: i32 = 13;
const SIG_DFL: usize = 0;

extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
}

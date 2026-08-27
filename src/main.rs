//! Viz executable entry point.

use std::ffi::OsString;
use std::io;

fn main() {
    let arguments: Vec<OsString> = std::env::args_os().collect();
    let mut output = io::stdout().lock();
    let mut error_output = io::stderr().lock();
    if let Err(error) = viz::cli::run(arguments, &mut output, &mut error_output) {
        eprintln!("viz: {error}");
        let code = if matches!(error, viz::Error::Usage(_)) {
            2
        } else {
            1
        };
        std::process::exit(code);
    }
}

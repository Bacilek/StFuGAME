//! Progress output: to the terminal and, with a timestamp, to logs/progress.log,
//! so the bot can be followed even when it runs in the background.

use std::{fs::OpenOptions, io::Write};

use chrono::Local;

const PATH: &str = "logs/progress.log";

/// Like `println!`, but also writes to the log. Never put credentials in here.
macro_rules! report {
    ($($arg:tt)*) => { $crate::report::write(&format!($($arg)*)) };
}

pub fn write(msg: &str) {
    let line = format!("{} {msg}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    println!("{line}");
    let res = std::fs::create_dir_all("logs").and_then(|()| {
        let mut f = OpenOptions::new().create(true).append(true).open(PATH)?;
        writeln!(f, "{line}")
    });
    if let Err(err) = res {
        eprintln!("Writing to {PATH} failed: {err}");
    }
}

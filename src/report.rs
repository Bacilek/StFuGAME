//! Výpis průběhu: na terminál a zároveň s časem do logs/prubeh.log,
//! aby se dalo sledovat, jak si bot vede, i když běží na pozadí.

use std::{fs::OpenOptions, io::Write};

use chrono::Local;

const PATH: &str = "logs/prubeh.log";

/// Jako `println!`, ale zapíše i do logu. Nikdy sem nedávat přihlašovací údaje.
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
        eprintln!("Zápis do {PATH} selhal: {err}");
    }
}

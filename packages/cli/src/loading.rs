use std::io::{self, Write};
use std::time::{Duration, Instant};

pub fn format_bytes(n: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = n as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

pub struct LoadingIndicator {
    label: String,
    started: Instant,
}

impl LoadingIndicator {
    pub fn start(label: &str) -> Self {
        eprint!("{label}");
        let _ = io::stderr().flush();
        Self {
            label: label.to_string(),
            started: Instant::now(),
        }
    }

    pub fn stop(self) {
        let _ = self.started;
        // Clear line roughly
        eprint!("\r{}\r", " ".repeat(self.label.len().saturating_add(8)));
        let _ = io::stderr().flush();
    }
}

pub struct ProgressReporter {
    label: String,
    last: Instant,
    bar: Option<indicatif::ProgressBar>,
}

impl ProgressReporter {
    pub fn new(label: &str) -> Self {
        let bar = if atty_stderr() {
            let pb = indicatif::ProgressBar::new_spinner();
            pb.set_style(
                indicatif::ProgressStyle::with_template("{msg} [{bar:24}] {bytes}/{total_bytes}")
                    .unwrap()
                    .progress_chars("█░"),
            );
            pb.set_message(label.to_string());
            Some(pb)
        } else {
            None
        };
        Self {
            label: label.to_string(),
            last: Instant::now(),
            bar,
        }
    }

    pub fn update(&mut self, done: u64, total: Option<u64>) {
        if let Some(pb) = &self.bar {
            if let Some(t) = total {
                pb.set_length(t);
                pb.set_position(done);
            } else {
                pb.set_position(done);
            }
            return;
        }
        if self.last.elapsed() < Duration::from_millis(80) && done > 0 {
            // throttle non-tty a bit less aggressively via milestones
        }
        self.last = Instant::now();
        if let Some(t) = total {
            if t > 0 {
                let pct = (done * 100 / t) as u32;
                if pct % 25 == 0 {
                    eprintln!("{} {}%", self.label, pct);
                }
            }
        } else if done > 0 && done % (2 * 1024 * 1024) < 64 * 1024 {
            eprintln!("{} {}", self.label, format_bytes(done));
        }
    }

    pub fn succeed(self, msg: &str) {
        if let Some(pb) = self.bar {
            pb.finish_and_clear();
        }
        println!("✅ {msg}");
    }

    pub fn fail(self, msg: &str) {
        if let Some(pb) = self.bar {
            pb.finish_and_clear();
        }
        eprintln!("❌ {msg}");
    }
}

fn atty_stderr() -> bool {
    // indicatif's IsTerminal
    use std::io::IsTerminal;
    io::stderr().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_basic() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KB");
    }
}

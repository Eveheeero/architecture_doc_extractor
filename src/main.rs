#![cfg_attr(feature = "pdf_inspector", allow(dead_code))]

#[path = "intel/intel.rs"]
mod intel;
pub(crate) mod pdf;
#[cfg(feature = "pdf_inspector")]
mod pdf_inspector;
#[cfg(test)]
mod tests;
#[allow(unused_imports)]
use simplelog::{CombinedLogger, Config, LevelFilter, SimpleLogger, WriteLogger};
#[allow(unused_imports)]
use std::fs::File;

#[cfg(not(feature = "pdf_inspector"))]
fn main() {
    setup_logger();
    intel::main();
}

#[cfg(feature = "pdf_inspector")]
fn main() {
    pdf_inspector::main();
}

fn setup_logger() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let log_level = LevelFilter::Debug;
        let log_config = Config::default();
        CombinedLogger::init(vec![
            SimpleLogger::new(log_level, log_config.clone()),
            // WriteLogger::new(log_level, log_config, File::create("last.log").unwrap()),
        ])
        .unwrap();
    });
}

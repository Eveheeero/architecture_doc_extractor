#[path = "intel/intel.rs"]
mod intel;
pub(crate) mod pdf;
#[cfg(test)]
mod tests;
use simplelog::{CombinedLogger, Config, LevelFilter, SimpleLogger, WriteLogger};
use std::fs::File;

fn main() {
    let log_level = LevelFilter::Trace;
    let log_config = Config::default();
    CombinedLogger::init(vec![
        SimpleLogger::new(log_level, log_config.clone()),
        WriteLogger::new(log_level, log_config, File::create("last.log").unwrap()),
    ])
    .unwrap();

    intel::main();
}

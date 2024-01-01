#[path = "intel/intel.rs"]
mod intel;
pub(crate) mod pdf;
#[cfg(test)]
mod tests;
use simplelog::{Config, LevelFilter, SimpleLogger};

fn main() {
    SimpleLogger::init(LevelFilter::Debug, Config::default()).unwrap();

    intel::main();
}

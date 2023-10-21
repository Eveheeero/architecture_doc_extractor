#[path = "intel/intel.rs"]
mod intel;
pub(crate) mod pdf;
#[cfg(test)]
mod tests;

fn main() {
    intel::main();
}

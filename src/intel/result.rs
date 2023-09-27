use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub(super) struct Instruction {
    pub(super) title: String,
    pub(super) summary: String,
    pub(super) instruction: (String, String),
    pub(super) description: Vec<String>,
    pub(super) operation: String,
    pub(super) flag_affected: String,
    pub(super) exceptions: HashMap<String, Vec<String>>,
    pub(super) c_and_cpp_equivalent: Vec<String>,
}

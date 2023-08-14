use crate::print_pages;

pub fn main() {
    extract_text();
}

fn extract_text() -> String {
    let doc = lopdf::Document::load_mem(include_bytes!("intel.pdf")).unwrap();
    let mut texts = Vec::new();
    for index in 129..=2266 {
        texts.append(&mut print_pages(&doc, index));
    }
    texts.join("\n")
}

fn parse_instructions() -> Vec<Instruction> {
    vec![]
}

struct Instruction {
    title: String,
    summary: String,
    instruction: (String, String),
    description: String,
    operation: String,
    flag_affected: String,
    exceptions: String,
}

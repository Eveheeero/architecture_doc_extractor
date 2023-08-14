use crate::print_pages;
use std::collections::HashMap;

pub fn main() {
    let data = extract_text();
    let _ = parse_instructions(data);
}

fn extract_text() -> Vec<String> {
    let doc = lopdf::Document::load_mem(include_bytes!("intel.pdf")).unwrap();
    let mut texts = Vec::new();
    for index in 129..=2266 {
        texts.append(&mut print_pages(&doc, index));
    }
    // std::fs::write("intel.txt", texts.join("\n")).unwrap();
    texts
}

fn parse_instructions(data: Vec<String>) -> Vec<Instruction> {
    let mut result = Vec::new();
    let mut obj = Instruction::default();
    let mut data = data.into_iter().peekable();
    let mut line;
    if let Some(line_) = data.next() {
        line = line_;
    } else {
        return result;
    }
    let mut tmp = String::new();

    loop {
        // title parsing
        obj.title = line[0..line.find('-').unwrap()].to_owned().to_lowercase();
        obj.summary = line[line.find('-').unwrap() + 1..].to_owned();
        // skip Instruction Operand Encoding, Description
        data.next();
        data.next();
        // collapse description
        loop {
            line = data.next().unwrap();
            if line == "Operation" {
                if tmp.len() > 0 {
                    obj.description.push(tmp);
                    tmp = String::new();
                }
                break;
            }
            tmp.push_str(&line);
            if (line.ends_with('.') || line.ends_with(". "))
                && data
                    .peek()
                    .unwrap()
                    .chars()
                    .next()
                    .unwrap()
                    .is_ascii_uppercase()
            {
                obj.description.push(tmp);
                tmp = String::new();
            }
        }
        // parse Operation
        loop {
            line = data.next().unwrap();
            if line == "Flags Affected" {
                if tmp.ends_with('\n') {
                    tmp.pop();
                }
                obj.operation = tmp;
                tmp = String::new();
                break;
            }
            tmp.push_str(&line);
            if line.len() != 1 {
                tmp.push('\n');
            }
        }
        // Flags Affected
        loop {
            tmp.push_str(&data.next().unwrap());
            if data.peek().unwrap().ends_with("Exceptions") {
                obj.flag_affected = tmp;
                tmp = String::new();
                break;
            }
        }
        // Exceptions
        line = data.next().unwrap();
        loop {
            let title = line;
            let mut exceptions = Vec::new();
            loop {
                line = data.next().unwrap();
                if line.ends_with("Exceptions") || line.starts_with("OpcodeInstructionOp") {
                    break;
                }
                exceptions.push(line);
            }
            obj.exceptions.insert(title, exceptions);
            if line.starts_with("OpcodeInstructionOp") {
                break;
            }
        }
        todo!();
    }

    result
}

#[derive(Debug, Default, Clone)]
struct Instruction {
    title: String,
    summary: String,
    instruction: (String, String),
    description: Vec<String>,
    operation: String,
    flag_affected: String,
    exceptions: HashMap<String, Vec<String>>,
}

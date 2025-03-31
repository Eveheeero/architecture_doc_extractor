mod result;
mod v1;
mod v2;

use crate::pdf::{self, v2::PdfString};
use result::Instruction;
use std::{collections::HashMap, sync::Once};

pub fn main() {
    let mut result = Vec::new();
    for (from, to) in [(129, 734), (742, 1476), (1481, 2196), (2198, 2266)] {
        let data = extract_text(from, to);
        result.append(&mut v2::parse_instructions(data));
    }
    let saved_instructions = save_instructions(result);
    saved_list_to_rust_enum(saved_instructions);
}

fn extract_text(from: u32, to: u32) -> Vec<Vec<PdfString>> {
    let doc = lopdf::Document::load_mem(include_bytes!("intel.pdf")).unwrap();
    use rayon::prelude::*;
    let texts: Vec<Vec<PdfString>> = (from..to)
        .into_par_iter()
        .map(|index| pdf::page_to_texts_v2(&doc, index))
        .collect();
    let file_name = format!("intel{from}_{to}.txt");
    if !std::fs::metadata(&file_name).is_ok() {
        std::fs::write(
            file_name,
            texts
                .iter()
                .map(|x| x.iter().map(PdfString::get).collect::<Vec<_>>().join("\n"))
                .collect::<Vec<_>>()
                .join("\n--------------------------------------------------\n"),
        )
        .unwrap();
    }
    texts
}

/// return is parsed instruction names
fn save_instructions(blocks: Vec<Instruction>) -> HashMap<String, Vec<String>> {
    blocks.into_iter().map(save_instruction).flatten().collect()
}

/// return is parsed instruction names
fn save_instruction(block: Instruction) -> HashMap<String, Vec<String>> {
    tracing::debug!("{} 페이지 생성중", block.title);
    let instructions = block.get_instructions_name();
    let md_contents: Vec<String> = block.into_md();
    static INIT_DIRECTORY: Once = Once::new();
    INIT_DIRECTORY.call_once(|| {
        std::fs::create_dir_all("result/intel").expect("베이스 디렉토리 생성 불가");
    });
    let mut saved_instructions = HashMap::new();
    for mut instruction in instructions.into_iter() {
        if instruction == "INT n" {
            instruction = "INT".into();
        }

        saved_instructions.insert(instruction.clone(), md_contents.clone());
        std::fs::write(
            format!("result/intel/{instruction}.md"),
            md_contents.join("\n"),
        )
        .expect(format!("{} 파일 생성 실패", instruction).as_str());
    }
    saved_instructions
}

fn saved_list_to_rust_enum(mut saved_instructions: HashMap<String, Vec<String>>) {
    let mut keys = saved_instructions.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    let mut result = Vec::new();
    result.push("enum X64 {".into());
    for instruction in keys.into_iter() {
        let docs = saved_instructions.remove(&instruction).unwrap();
        for line in docs.iter() {
            if line.is_empty() {
                result.push("    ///".into());
            } else {
                result.push(format!("    /// {}", line));
            }
        }
        // 맨 앞글자만 빼고 소문자로 바꿈
        let instruction = instruction
            .chars()
            .enumerate()
            .map(|(i, c)| if i == 0 { c } else { c.to_ascii_lowercase() })
            .collect::<String>();
        result.push(format!("    {instruction},"));
    }
    result.push("}".into());

    std::fs::write("result/intel.rs", result.join("\n")).expect("모듈 생성 실패");
}

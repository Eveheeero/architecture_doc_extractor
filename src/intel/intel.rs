mod align_with_pdf_inner_operator;
mod align_with_pdf_position;
mod category;
mod result;

use crate::pdf;
use result::Instruction;
use std::sync::Once;

pub fn main() {
    let mut result = Vec::new();
    for (from, to) in [(129, 734), (742, 1476), (1481, 2196), (2198, 2266)] {
        let data = extract_text(from, to);
        result.append(&mut align_with_pdf_position::parse_instructions(data));
    }
    blocks_into_string(result);
}

fn extract_text(from: u32, to: u32) -> Vec<Vec<String>> {
    let doc = lopdf::Document::load_mem(include_bytes!("intel.pdf")).unwrap();
    use rayon::prelude::*;
    let texts: Vec<Vec<String>> = (from..=to)
        .into_par_iter()
        .map(|index| pdf::page_to_texts_align_with_pdf_position(&doc, index))
        .collect();
    let file_name = format!("intel{from}_{to}.txt");
    if !std::fs::metadata(&file_name).is_ok() {
        std::fs::write(
            file_name,
            texts
                .iter()
                .flat_map(|x| x.to_owned())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
    }
    texts
}

fn blocks_into_string(blocks: Vec<Instruction>) {
    blocks.into_iter().for_each(block_into_string)
}

fn block_into_string(block: Instruction) {
    let instruction = block.title.to_owned();
    let description: Vec<String> = block.into_string();
    static INIT_DIRECTORY: Once = Once::new();
    INIT_DIRECTORY.call_once(|| {
        std::fs::create_dir_all("result/intel").expect("베이스 디렉토리 생성 불가");
    });
    for instruction in instruction.split('/') {
        std::fs::write(
            format!("result/intel/{instruction}.md"),
            description.join("\n"),
        )
        .expect(format!("{} 파일 생성 실패", instruction).as_str());
    }
}

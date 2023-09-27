mod category;
mod context;
mod result;

use crate::print_pages;
use category::{parse_category, Category};
use context::ParsingContent;
use result::Instruction;

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
    std::fs::write("intel.txt", texts.join("\n")).unwrap();
    texts
}

fn parse_instructions(data: Vec<String>) -> Vec<Instruction> {
    let mut iter = data.into_iter().peekable();
    let mut context = ParsingContent::default();

    loop {
        if iter.peek().is_none() {
            // 기존내용 마무리
            context.clear_stacked_status();
            break;
        }
        context.read(&mut iter);
        let category = parse_category(context.line());

        if context.last_category == Category::OpcodeDescriptionStart {
            if !context.line().ends_with("Description") {
                // OpcodeDescriptionStart가 왔으면 end가 올때까지 무시
                context.stack();
                continue;
            } else {
                // OpcodeDescription 파싱
                // TODO stacked_content로 OpcodeDescription 테이블 가져옴
                context.stacked_content.clear();
                context.last_category = Category::OpcodeDescription;
                continue;
            }
        }
        if context.last_category == Category::IntrinsicEquivalentStart {
            if context.line() == " Compiler Intrinsic Equivalent" {
                context.last_category = Category::IntrinsicEquivalent;
            }
            continue;
        }

        match category {
            Category::Summary => {
                context.clear_stacked_status();
                // 이전 데이터 등록 (last_category가 NeedIgnore일경우 처음이니까 무시)
                if context.last_category != Category::NeedIgnore {
                    dbg!(&context.instruction);
                    context.result.push(context.instruction.clone());
                }
                context.instruction = Instruction::default();
                // 인스트럭션과 메인 설명 삽입
                let mut line = context.line().splitn(2, '-');
                let title = line.next().unwrap().trim().to_owned();
                let summary = line.next().unwrap().trim().to_owned();
                context.instruction.title = title;
                context.instruction.summary = summary;
            }
            Category::OpcodeDescription => {
                context.clear_stacked_status();
                context.last_category = category;
                context.set_title(context.line().to_owned());
            }
            Category::OpcodeDescriptionStart => {
                context.clear_stacked_status();
                context.last_category = category;
                context.stack();
                context.set_title(context.stacked_content.join(""));
                context.stacked_content.clear();
            }
            Category::InstructionOperandEncoding => {
                context.clear_stacked_status();
                context.last_category = category;
            }
            Category::Description => {
                context.clear_stacked_status();
                context.last_category = category;
            }
            Category::Operation => {
                context.clear_stacked_status();
                context.last_category = category;
            }
            Category::FlagsAffected => {
                context.clear_stacked_status();
                context.last_category = category;
            }
            Category::Exceptions => {
                context.clear_stacked_status();
                context.set_title(context.line().to_owned());
                context.last_category = category;
            }
            Category::None => context.stack(),
            Category::NeedIgnore => {}
            Category::IntrinsicEquivalent => {
                context.clear_stacked_status();
                context.last_category = category;
            }
            Category::IntrinsicEquivalentStart => {
                context.clear_stacked_status();
                context.last_category = category;
            }
        };
    }

    context.result
}

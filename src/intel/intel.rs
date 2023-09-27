mod category;
mod context;
mod result;

use crate::print_pages;
use category::{parse_category, Category};
use context::ParsingContext;
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
    let mut context = ParsingContext::default();

    loop {
        if iter.peek().is_none() {
            // 기존내용 마무리
            clear_stacked_status(&mut context);
            break;
        }
        context.read(&mut iter);
        let category = parse_category(context.line());

        if context.last_category == Category::OpcodeDescriptionStart {
            if !context.line().ends_with("Description") {
                // OpcodeDescriptionStart가 왔으면 end가 올때까지 무시
                context.stack(None);
                continue;
            } else {
                // OpcodeDescription 파싱
                // TODO stacked_content로 OpcodeDescription 테이블 가져옴
                context.clear_stacked_data();
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
                clear_stacked_status(&mut context);
                // 이전 데이터 등록 (last_category가 NeedIgnore일경우 처음이니까 무시)
                if context.last_category != Category::NeedIgnore {
                    dbg!(&context.instruction);
                    context.next_instruction()
                }
                // 인스트럭션과 메인 설명 삽입
                let mut line = context.line().splitn(2, '-');
                let title = line.next().unwrap().trim().to_owned();
                let summary = line.next().unwrap().trim().to_owned();
                context.instruction.title = title;
                context.instruction.summary = summary;
            }
            Category::OpcodeDescription => {
                clear_stacked_status(&mut context);
                context.last_category = category;
                context.stack(None);
            }
            Category::OpcodeDescriptionStart => {
                clear_stacked_status(&mut context);
                context.last_category = category;
                context.stack(None);
                let stacked = context.clear_stacked_data().join("");
                context.stack(Some(stacked));
            }
            Category::InstructionOperandEncoding => {
                clear_stacked_status(&mut context);
                context.last_category = category;
            }
            Category::Description => {
                clear_stacked_status(&mut context);
                context.last_category = category;
            }
            Category::Operation => {
                clear_stacked_status(&mut context);
                context.last_category = category;
            }
            Category::FlagsAffected => {
                clear_stacked_status(&mut context);
                context.last_category = category;
            }
            Category::Exceptions => {
                clear_stacked_status(&mut context);
                context.stack(None);
                context.last_category = category;
            }
            Category::None => context.stack(None),
            Category::NeedIgnore => {}
            Category::IntrinsicEquivalent => {
                clear_stacked_status(&mut context);
                context.last_category = category;
            }
            Category::IntrinsicEquivalentStart => {
                clear_stacked_status(&mut context);
                context.last_category = category;
            }
        };
    }

    context.done()
}

/// 이전까지 파싱했던 내용을 저장한다.
fn clear_stacked_status(context: &mut ParsingContext) {
    match context.last_category {
        Category::OpcodeDescription => {
            let content = context.clear_stacked_data().join("");
            // instruction에 저장
            // TODO
        }
        Category::OpcodeDescriptionStart => {
            // DescriptionStart가 왔으면 end가 올때까지 무시
            // instruction.instruction...
        }
        Category::InstructionOperandEncoding => {
            // 파싱 계획 없음
            context.clear_stacked_data();
        }
        Category::Description => {
            context.instruction.description = context.clear_stacked_data();
        }
        Category::Operation => {
            context.instruction.operation = context.clear_stacked_data().join("");
        }
        Category::FlagsAffected => {
            context.instruction.flag_affected = context.clear_stacked_data().join("");
        }
        Category::Exceptions => {
            // stacked_content를 다른것으로 파싱
            let mut stacked_content = context.clear_stacked_data();
            context
                .instruction
                .exceptions
                .insert(stacked_content.pop().unwrap(), stacked_content);
        }
        Category::IntrinsicEquivalent => {
            context.instruction.c_and_cpp_equivalent = context.clear_stacked_data();
        }

        Category::NeedIgnore => {}
        _ => unreachable!(),
    }
}

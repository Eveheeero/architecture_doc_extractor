mod category;
mod context;
mod result;

use crate::print_pages;
use category::{parse_category, Category};
use context::ParsingContext;
use result::Instruction;
use std::sync::Once;

pub fn main() {
    let mut result = Vec::new();
    for (from, to) in [(129, 734)] {
        let data = extract_text(from, to);
        result.append(&mut parse_instructions(data));
    }
    blocks_into_string(result);
}

fn extract_text(from: u32, to: u32) -> Vec<Vec<String>> {
    let doc = lopdf::Document::load_mem(include_bytes!("intel.pdf")).unwrap();
    let mut texts = Vec::new();
    for index in from..=to {
        texts.push(print_pages(&doc, index));
    }
    if !std::fs::metadata("intel.txt").is_ok() {
        std::fs::write(
            "intel.txt",
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

fn parse_instructions(data: Vec<Vec<String>>) -> Vec<Instruction> {
    let mut context = ParsingContext::default();

    for page in data.into_iter() {
        let mut iter = page.into_iter().peekable();
        let mut page_first = true;
        context.last_category = context.last_plain_category;
        // 페이지당 푸터나 헤더때문에 3줄, 4줄씩 지워야함
        loop {
            let line = iter.next().unwrap();
            if line.starts_with("Vol. ") || line.ends_with("Vol. 2A") {
                break;
            }
        }

        loop {
            if iter.peek().is_none() {
                // 기존내용 마무리
                clear_stacked_status(&mut context);
                break;
            }
            // 라인 읽기
            context.read(&mut iter);
            // 읽은 라인이 어떤내용인지 파싱
            let mut category = parse_category(context.line());

            if category == Category::Summary && !page_first {
                category = Category::None;
            }
            if context.last_category == Category::OpcodeDescriptionStart {
                if !context.line().ends_with("Description") {
                    // OpcodeDescriptionStart가 왔으면 end가 올때까지 무시
                    context.stack(None);
                    continue;
                } else {
                    // OpcodeDescription 파싱
                    // TODO stacked_content로 OpcodeDescription 테이블 가져옴
                    context.clear_stacked_data();
                    context.set_last_category(Category::OpcodeDescription);
                    continue;
                }
            }
            if context.last_category == Category::IntrinsicEquivalentStart {
                if context.line().contains(" Compiler Intrinsic Equivalent") {
                    context.set_last_category(Category::IntrinsicEquivalent);
                }
                continue;
            }

            page_first = false;
            parse_about_category(&mut context, category);
        }
    }

    clear_stacked_status(&mut context);
    context.done()
}

/// 이전까지 파싱했던 내용을 저장한다.
fn clear_stacked_status(context: &mut ParsingContext) {
    match context.last_category {
        Category::Summary => {
            // 인스트럭션과 메인 설명 삽입
            let stacked = context.clear_stacked_data().join("");
            let mut line = stacked.splitn(2, '-');
            let title = line.next().unwrap().trim().to_owned();
            let summary = line.next().unwrap().trim().to_owned();
            context.instruction.title = title;
            context.instruction.summary = summary;
        }
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
            let mut stacked = context.clear_stacked_data();
            context.instruction.description.append(&mut stacked);
        }
        Category::Operation => {
            let stacked = context.clear_stacked_data().join("");
            context.instruction.operation.push_str(&stacked);
        }
        Category::FlagsAffected => {
            let stacked = context.clear_stacked_data().join("");
            context.instruction.flag_affected.push_str(&stacked);
        }
        Category::Exceptions => {
            // stacked_content를 다른것으로 파싱
            let mut stacked_content = context.clear_stacked_data();
            if stacked_content.len() >= 2 {
                let title = stacked_content.remove(0);
                context
                    .instruction
                    .exceptions
                    .insert(title, stacked_content);
            }
        }
        Category::IntrinsicEquivalent => {
            let mut stacked = context.clear_stacked_data();
            context
                .instruction
                .c_and_cpp_equivalent
                .append(&mut stacked);
        }

        Category::NeedIgnore => {}
        _ => unreachable!(),
    }
}

/// 카테고리에 대해 파싱을 진행한다.
fn parse_about_category(context: &mut ParsingContext, category: Category) {
    match category {
        Category::Summary => {
            clear_stacked_status(context);
            // 이전 데이터 등록 (last_category가 NeedIgnore일경우 처음이니까 무시)
            if context.last_category != Category::NeedIgnore {
                dbg!(&context.instruction);
                context.next_instruction()
            }
            context.set_last_category(category);
            context.stack(None);
        }
        Category::OpcodeDescription => {
            clear_stacked_status(context);
            context.set_last_category(category);
            context.stack(None);
        }
        Category::OpcodeDescriptionStart => {
            clear_stacked_status(context);
            context.set_last_category(category);
            context.stack(None);
            let stacked = context.clear_stacked_data().join("");
            context.stack(Some(stacked));
        }
        Category::InstructionOperandEncoding => {
            clear_stacked_status(context);
            context.set_last_category(category);
        }
        Category::Description => {
            clear_stacked_status(context);
            context.set_last_category(category);
        }
        Category::Operation => {
            clear_stacked_status(context);
            context.set_last_category(category);
        }
        Category::FlagsAffected => {
            clear_stacked_status(context);
            context.set_last_category(category);
        }
        Category::Exceptions => {
            clear_stacked_status(context);
            context.stack(None);
            context.set_last_category(category);
        }
        Category::None => context.stack(None),
        Category::NeedIgnore => {}
        Category::IntrinsicEquivalent => {
            clear_stacked_status(context);
            context.set_last_category(category);
        }
        Category::IntrinsicEquivalentStart => {
            clear_stacked_status(context);
            context.set_last_category(category);
        }
    }
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
    std::fs::write(
        format!("result/intel/{instruction}.md"),
        description.join("\n"),
    )
    .expect(format!("{} 파일 생성 실패", instruction).as_str());
}

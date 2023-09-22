use regex::Regex;

use crate::print_pages;
use std::{collections::HashMap, sync::OnceLock};

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
    let mut iter = data.into_iter().peekable();
    let mut line: String;
    let mut last_category: Category = Category::NeedIgnore;
    let mut stacked_content = Vec::new();
    let mut instruction = Instruction::default();

    loop {
        let close = || match last_category {
            Category::OpcodeDescription => todo!(),
            Category::OpcodeDescriptionStart => {
                // DescriptionStart가 왔으면 end가 올때까지 무시
            }
            Category::InstructionOperandEncoding => todo!(),
            Category::Description => todo!(),
            Category::Operation => todo!(),
            Category::FlagsAffected => todo!(),
            Category::Exceptions => todo!(),

            Category::NeedIgnore => {}
            _ => unreachable!(),
        };

        if iter.peek().is_none() {
            // 기존내용 마무리
            close();
            break;
        }
        line = iter.next().unwrap();
        let category = parse_category(&line);

        if last_category == Category::OpcodeDescriptionStart
            && category != Category::OpcodeDescriptionEnd
        {
            // OpcodeDescriptionStart가 왔으면 end가 올때까지 무시
            stacked_content.push(line);
            continue;
        }

        match category {
            Category::Summary => {
                close();
                // 이전 데이터 등록 (last_category가 NeedIgnore일경우 처음이니까 무시)
                if last_category != Category::NeedIgnore {
                    result.push(instruction.clone());
                }
                instruction = Instruction::default();
                // 인스트럭션과 메인 설명 삽입
                let mut line = line.splitn(2, '-');
                instruction.title = line.next().unwrap().trim().to_string();
                instruction.summary = line.next().unwrap().trim().to_string();
            }
            Category::OpcodeDescription => todo!(),
            Category::OpcodeDescriptionStart => {
                close();
                last_category = category;
            }
            Category::OpcodeDescriptionEnd => {
                if last_category == Category::OpcodeDescriptionStart {
                    // OpcodeDescription 파싱
                    // TODO stacked_content로 OpcodeDescription 테이블 가져옴
                    stacked_content.clear();
                    last_category = Category::OpcodeDescription;
                } else {
                    // 이전 카테고리 상태가 start가 아니었으면 잘못 탐지된것임
                    stacked_content.push(line);
                }
            }
            Category::InstructionOperandEncoding => todo!(),
            Category::Description => todo!(),
            Category::Operation => todo!(),
            Category::FlagsAffected => todo!(),
            Category::Exceptions => todo!(),
            Category::None => stacked_content.push(line),
            Category::NeedIgnore => {}
        };
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    /// 줄거리가 포함된 라인, -을 기준으로 왼쪽이 인스트럭션, 오른쪽이 설명
    Summary,
    /// 옵코드 별 설명, Opcode로 시작하고 Description에서 끝난다. 중간 내용은 계속 바뀌며, 줄바꿈이 올 수 있다.
    /// TODO 수정 필요
    OpcodeDescription,
    OpcodeDescriptionStart,
    OpcodeDescriptionEnd,
    /// 인스트럭션 인코딩 방법, 다음 라인에 내용이 들어오는데 표시할 필요는 없는 듯 하다
    InstructionOperandEncoding,
    /// 옵코드 설명, Description이 온 이후, 다음 Category가 올때까지가 설명임, 이전라인의 끝을 rtrim했을때 .으로 끝나고, 다음라인의 시작이 대문자이면 라인바꿈
    Description,
    /// 옵코드 가상코드, 해당 라인 이후부터 시작
    Operation,
    /// 해당 라인 이후부터 다음 Category가 올때까지 설명이 이어진다.
    FlagsAffected,
    /// Exceptions. 종류가 무엇인지는 봐야한다. 다음 라인부터 설명이 적혀있다. Same exceptions as .. mode. 가 올수도 있다.
    /// \# 가 온 이후에 대문자로 요약이 온 후, 설명이 온다. \#가 없을 수 있다. 없는경우 이전 \# 사용
    Exceptions,
    /// belong to before category
    None,
    /// 필요 없는 내용 (페이지 끝 주석이거나, 파싱할 필요 없는 내용)
    NeedIgnore,
}

fn parse_category(data: impl AsRef<str>) -> Category {
    static SUMMARY: OnceLock<Regex> = OnceLock::new();
    static OPCODE_DESCRIPTION_START: OnceLock<Regex> = OnceLock::new();
    static OPCODE_DESCRIPTION_END: OnceLock<Regex> = OnceLock::new();
    static INSTRUCTION_OPERAND_ENCODING: OnceLock<Regex> = OnceLock::new();
    static DESCRIPTION: OnceLock<Regex> = OnceLock::new();
    static OPERATION: OnceLock<Regex> = OnceLock::new();
    static FLAGAFFECTED: OnceLock<Regex> = OnceLock::new();
    static EXCEPTIONS: OnceLock<Regex> = OnceLock::new();
    static IGNORE: OnceLock<Regex> = OnceLock::new();
    let summary = SUMMARY.get_or_init(|| Regex::new("^[^a-z ]*-.+$").unwrap());
    // Opcode로 시작해서 여러 줄 거쳐서 Description으로 끝나는 경우
    let opcode_description_start =
        OPCODE_DESCRIPTION_START.get_or_init(|| Regex::new("^Opcode").unwrap());
    let opcode_description_end =
        OPCODE_DESCRIPTION_END.get_or_init(|| Regex::new("Description$").unwrap());
    let instruction_operand_encoding = INSTRUCTION_OPERAND_ENCODING
        .get_or_init(|| Regex::new("^Op/En.*Operand 1Operand 2Operand 3Operand 4$").unwrap());
    let description = DESCRIPTION.get_or_init(|| Regex::new("^Description$").unwrap());
    let operation = OPERATION.get_or_init(|| Regex::new("^Operation$").unwrap());
    let flag_effected = FLAGAFFECTED.get_or_init(|| Regex::new("^Flags Affected$").unwrap());
    let exceptions = EXCEPTIONS.get_or_init(|| Regex::new("^.* Mode Exceptions$").unwrap());
    let ignore = IGNORE.get_or_init(|| Regex::new("(^Instruction Operand Encoding$)").unwrap());

    let data = data.as_ref();
    match () {
        () if summary.is_match(data) => Category::Summary,
        () if opcode_description_start.is_match(data) && opcode_description_end.is_match(data) => {
            Category::OpcodeDescription
        }
        () if opcode_description_start.is_match(data) => Category::OpcodeDescriptionStart,
        () if opcode_description_end.is_match(data) => Category::OpcodeDescriptionEnd,
        () if instruction_operand_encoding.is_match(data) => Category::InstructionOperandEncoding,
        () if description.is_match(data) => Category::Description,
        () if operation.is_match(data) => Category::Operation,
        () if flag_effected.is_match(data) => Category::FlagsAffected,
        () if exceptions.is_match(data) => Category::Exceptions,
        () if ignore.is_match(data) => Category::NeedIgnore,
        _ => Category::None,
    }
}

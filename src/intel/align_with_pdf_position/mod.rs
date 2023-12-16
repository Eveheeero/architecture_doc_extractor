mod parse_section;

use super::result::Instruction;

#[derive(PartialEq, Eq)]
enum Section {
    InstructionsStart,
    Instruction,
    TitleStart,
    Title,
    Description,
    Operation,
    FlagsAffected,
    Exceptions(String),
    None,
}

impl Section {
    fn is_exceiption(&self) -> bool {
        match self {
            Section::Exceptions(_) => true,
            _ => false,
        }
    }
    fn get_exception_kind(&self) -> &str {
        match self {
            Section::Exceptions(kind) => kind,
            _ => unreachable!(),
        }
    }
}

#[allow(dead_code)]
pub(super) fn parse_instructions(data: Vec<Vec<String>>) -> Vec<Instruction> {
    let mut result = Vec::new();
    let mut now = Instruction::default();
    let mut now_section = Section::None;
    for page in data.into_iter() {
        let (page, operation, summary) = get_operation_summary(&page);
        // 기존 인스트럭션이랑 다르면 푸시
        if operation != now.title {
            if !now.title.is_empty() {
                result.push(now);
            }
            now = Instruction::default();
            now.title = operation;
            now.summary = summary.trim().to_owned();
        }

        for line in page.into_iter() {
            /* 현재 섹션에 따라 몇몇 특수처리 */
            if now_section == Section::InstructionsStart {
                /* Opcode/Instruction/OP가 오는건 37...가 왔을떄 끝 */
                if !line.contains("\x01") {
                    continue;
                } else {
                    now_section = Section::Instruction;
                }
            }

            /* 현재 라인이 어떤 섹션에 속하는지 파싱 */
            let parsed_section = parse_section::parse_now_section(&now, line);

            /* 파싱된 섹션 결과물에 따라 연산 */
            match parsed_section {
                Section::InstructionsStart => now_section = parsed_section,
                Section::None if now_section == Section::Instruction => {}
                Section::TitleStart => now_section = parsed_section,
                Section::Description => {
                    now_section = parsed_section;
                }
                Section::None if now_section == Section::Description => {
                    now.description.push(line.into());
                }
                Section::Operation => {
                    now_section = parsed_section;
                }
                Section::None if now_section == Section::Operation => {
                    now.operation += &line;
                }
                Section::FlagsAffected => {
                    now_section = parsed_section;
                }
                Section::None if now_section == Section::FlagsAffected => {
                    now.flag_affected += &line;
                }
                Section::Exceptions(ref kind) => {
                    now_section = Section::Exceptions(kind.into());
                    now.exceptions.insert(kind.into(), Vec::new());
                }
                Section::None if now_section.is_exceiption() => {
                    now.exceptions
                        .get_mut(now_section.get_exception_kind())
                        .unwrap()
                        .push(line.into());
                }
                _ => {}
            }
        }
    }
    result
}

fn get_operation_summary(page: &[String]) -> (&[String], String, String) {
    let mut lasts = Vec::new();
    let mut temp = String::new();
    let mut to = page.len();
    for (i, line) in page.iter().rev().enumerate() {
        if line.contains('-') {
            lasts.push(line.clone() + &temp);
            temp.clear();
            if lasts.len() == 2 {
                to = page.len() - i - 1;
                break;
            }
        } else {
            temp = line.clone() + &temp;
        }
    }

    let target = lasts.iter().filter(|x| !x.contains('.')).next().unwrap();
    let (operation, summary) = target.split_once("-").unwrap();
    (&page[1..to], operation.to_owned(), summary.to_owned())
}

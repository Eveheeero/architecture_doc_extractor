use super::result::Instruction;

#[derive(PartialEq, Eq)]
enum Section {
    InstructionsStart,
    Instruction,
    None,
}

#[allow(dead_code)]
pub(super) fn parse_instructions(data: Vec<Vec<String>>) -> Vec<Instruction> {
    let mut result = Vec::new();
    let mut now = Instruction::default();
    let mut now_section = Section::None;
    let mut stacked: Vec<String> = Vec::new();
    for page in data.into_iter() {
        let (operation, summary) = page.last().unwrap().split_once('-').unwrap();
        let page = &page[1..page.len() - 1];
        // 기존 인스트럭션이랑 다르면 푸시
        if operation != now.title {
            if !now.title.is_empty() {
                result.push(now);
            }
            now = Instruction::default();
            now.title = operation.to_owned();
            now.summary = summary.trim().to_owned();
        }

        for line in page.into_iter() {
            // 인스트럭션 타이틀이면 스킵
            if line.starts_with(&format!("{title}-", title = now.title)) {
                continue;
            }
            if now_section == Section::InstructionsStart {
                if !line.contains("\x01") {
                    continue;
                } else {
                    now_section = Section::Instruction;
                }
            }

            let parsed_section = parse_now_section(line);

            match parsed_section {
                Section::InstructionsStart => now_section = parsed_section,
                Section::Instruction => unreachable!(),
                Section::None if now_section == Section::Instruction => stacked.push(line.into()),
                _ => {}
            }
        }
    }
    result
}

fn parse_now_section(line: impl AsRef<str>) -> Section {
    let line: &str = line.as_ref();
    match () {
        () if line.starts_with("Opcode\x01Instruction\x01Op") => Section::InstructionsStart,
        _ => Section::None,
    }
}

mod parse_section;

use super::result::Instruction;
use regex::{Regex, RegexSet};
#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

#[derive(PartialEq, Eq)]
enum Section {
    InstructionsStart,
    Instruction,
    Description,
    Operation,
    FlagsAffected,
    Exceptions(String),
    FootNote(u8),
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
    for (index, page) in data.into_iter().enumerate() {
        debug!("{}번째 페이지 파싱중", index);

        let (mut page, operation, summary) = get_operation_summary(&page);
        // 기존 인스트럭션이랑 다르면 푸시
        if operation != now.title {
            if !now.title.is_empty() {
                result.push(now);
            }
            now = Instruction::default();
            now.title = operation;
            now.summary = summary.trim().to_owned();

            /* 페이지가 변경됐을때 Opcode가 나오기 전까지는 타이틀이라 무시 */
            page = skip_until_title_end(page);
            customize_title_summary(&mut now);
        }

        for line in page.into_iter() {
            trace!("라인 내용 : {}", line);

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
                Section::FootNote(num) => {
                    // 각주로 인식은 하지만 따로 저장하지 않음.
                    now_section = Section::FootNote(num);
                }
                _ => {}
            }
        }
    }
    result
}

/// 전체 페이지에서, 인스트럭션과 인스트럭션의 설명을 분리한 후, footer 및 header를 제거한 후 반환
fn get_operation_summary(page: &[String]) -> (&[String], String, String) {
    /*
    Vol. 2A3-55 같은 형식의 문자열 및
    AESDEC128KL-.... 같은 형색의 문자열을 인식해야함 (여러 줄로 나뉘어 있을 수 있음) (끝에 . 없음)
     */

    //  Vol. 2A3-55 혹은 3-48Vol. 2A같은 형식의 문자열을 인식
    let regex1 = Regex::new(r"^(Vol\. \d[A-Z]\d-\d+|\d-\d+Vol\. \d[A-Z])$").unwrap();
    // AESDEC128KL-.... 같은 형색의 문자열을 인식
    let regex2 = RegexSet::new([
        "^[A-Z]([A-Z0-9()/]|cc|, ?|x\\d{1,2})+-",
        // INT n/INTO/INT3/INT1-
        "^INT n/INTO/INT3/INT1-",
        // ADDX -
        "^ADOX -",
        // FCOMI/FCOMIP/ FUCOMI/FUCOMIP-
        "^FCOMI/FCOMIP/ FUCOMI/FUCOMIP-",
        // PREFETCHh-
        "^PREFETCHh-",
        // RORX -
        "^RORX -",
        // VF[,N]M(ADD|SUB)[132,213,231](P|S)H- (PH와 SH 따로) (ADD와 SUB 따로)
        "^VF\\[,N]M(ADD|SUB)\\[132,213,231](P|S)H-",
        // INT n/INTO/INT3/IN
        "^INT n/INTO/INT3/IN$",
        // KSHIFTLW/KSHIFTLB/KSHIFTLQ/KSHIF
        "^KSHIFTLW/KSHIFTLB/KSHIFTLQ/KSHIF$",
        // PUNPCKLBW/PUNPCKLWD/PUNPCKLD
        "^PUNPCKLBW/PUNPCKLWD/PUNPCKLD$",
        // VEXTRACTI128/VEXTRACTI32x4/VEXTRACTI64x2/VEXTRACTI
        "^VEXTRACTI128/VEXTRACTI32x4/VEXTRACTI64x2/VEXTRACTI$",
        // VFMADDSUB132PD/VFMADDSUB213PD/VFMADDSUB231P
        "^VFMADDSUB132PD/VFMADDSUB213PD/VFMADDSUB231P$",
        // VFMADDSUB132PH/VFMADDSUB213PH/
        "^VFMADDSUB132PH/VFMADDSUB213PH/$",
        // VFMSUBADD132PD/VFMSUBADD213PD/VFMSUBADD
        "^VFMSUBADD132PD/VFMSUBADD213PD/VFMSUBADD$",
        // VFMSUBADD132PH/VFMSUBADD213P
        "^VFMSUBADD132PH/VFMSUBADD213P$",
        // VFMSUBADD132PS/VFMSUBADD213PS/VFMSUBADD
        "^VFMSUBADD132PS/VFMSUBADD213PS/VFMSUBADD$",
        // VFNMADD132PD/VFNMADD213PD/
        "^VFNMADD132PD/VFNMADD213PD/$",
        // VFNMADD132SD/VFNMADD213SD/
        "^VFNMADD132SD/VFNMADD213SD/$",
    ])
    .unwrap();
    // 제외정규식
    let regex2_filter = RegexSet::new([
        // T1 (INT n/INTO/INT3/INT1)
        "^T1-",
        // TLD (KSHIFTLW/KSHIFTLB/KSHIFTLQ/KSHIFTLD)
        "^TLD-",
        // Q/PUNPCKLQDQ (PUNPCKLBW/PUNPCKLWD/PUNPCKLDQ/PUNPCKLQDQ)
        "^Q/PUNPCKLQDQ-",
        // VFMADDSUB231PH (VFMADDSUB132PH/VFMADDSUB213PH/VFMADDSUB231PH)
        "^VFMADDSUB231PH-",
        // H/VFMSUBADD231PH (VFMSUBADD132PH/VFMSUBADD213PH/VFMSUBADD231PH)
        "^H/VFMSUBADD231PH-",
        // VFNMADD231PD (VFNMADD132PD/VFNMADD213PD/VFNMADD231PD)
        "^VFNMADD231PD-",
        // VFNMADD231SD (VFNMADD132SD/VFNMADD213SD/VFNMADD231SD)
        "^VFNMADD231SD-",
    ])
    .unwrap();

    let mut matched1 = false;
    let mut matched2 = false;
    let mut title_and_summary = String::new();
    let mut temp = String::new();
    let mut to = page.len();
    for (i, line) in page.iter().rev().enumerate() {
        if matched1 && matched2 {
            to = page.len() - i - 2;
            break;
        }

        if regex1.is_match(&line) {
            temp.clear();
            matched1 = true;
        } else if regex2.is_match(&line) && !regex2_filter.is_match(&line) {
            title_and_summary = line.clone() + &temp;
            temp.clear();
            matched2 = true;
        } else {
            trace!("Footer 파싱 중 올바르지 않은 라인 발견 : {}", line);
            temp = line.clone() + &temp;
        }
    }

    trace!("Footer내부 인스트럭션 및 설명 : {}", title_and_summary);
    let (operation, summary) = title_and_summary
        .split_once("-")
        .expect("예외처리할 패턴이 발생했습니다.");
    (&page[1..to], operation.to_owned(), summary.to_owned())
}

fn skip_until_title_end(page: &[String]) -> &[String] {
    let mut result = page;
    while !result
        .get(0)
        .expect("이번 footer 혹은 지난 footer 파싱이 잘못되었습니다.")
        .starts_with("Opcode")
    {
        result = &result[1..];
    }
    result
}

/// 특수한 인스트럭션에 대한 설명을 수정한다.
fn customize_title_summary(instruction: &mut Instruction) {
    match instruction.title.as_str() {
        "F2XM1" => instruction.summary = "Compute (2^x)-1".to_owned(),
        _ => {}
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct MdTable {
    pub(crate) headers: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
}

impl MdTable {
    pub(crate) fn to_md_lines(&self) -> Vec<String> {
        if self.headers.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::new();
        let header_line = format!("| {} |", self.headers.join(" | "));
        result.push(header_line);
        let sep_line = format!(
            "| {} |",
            self.headers
                .iter()
                .map(|_| "---".to_owned())
                .collect::<Vec<_>>()
                .join(" | ")
        );
        result.push(sep_line);
        for row in &self.rows {
            let mut cells: Vec<String> = row.clone();
            cells.resize(self.headers.len(), String::new());
            result.push(format!("| {} |", cells.join(" | ")));
        }
        result
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ArmEncoding {
    /// 인코딩 이름 (e.g., "ADD_32_addsub_imm")
    pub(crate) name: String,
    /// 인코딩 라벨 (e.g., "32-bit")
    pub(crate) label: String,
    /// 어셈블리 템플릿 (e.g., "ADD <Wd|WSP>, <Wn|WSP>, #<imm>{, <shift>}")
    pub(crate) asm_template: String,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct BitField {
    /// 최상위 비트 위치
    pub(crate) hibit: u8,
    /// 필드 폭
    pub(crate) width: u8,
    /// 필드 이름
    pub(crate) name: String,
    /// 상수 값 (있으면)
    pub(crate) constants: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ArmInstruction {
    /// XML id (e.g., "ADD_addsub_imm") — 유일한 식별자
    pub(crate) id: String,
    /// 표시 제목 (e.g., "ADD (immediate)")
    pub(crate) heading: String,
    /// 니모닉 (e.g., "ADD")
    pub(crate) mnemonic: String,
    /// 인스트럭션 클래스 (e.g., "general", "advsimd", "sve")
    pub(crate) instr_class: String,
    /// 간단한 요약
    pub(crate) brief: String,
    /// 상세 설명
    pub(crate) description: Vec<String>,
    /// 인코딩 변형
    pub(crate) encodings: Vec<ArmEncoding>,
    /// 인코딩 다이어그램 비트필드
    pub(crate) bitfields: Vec<BitField>,
    /// 오퍼랜드 설명 (심볼, 설명)
    pub(crate) operand_explanations: Vec<(String, String)>,
    /// 디코드 의사코드
    pub(crate) decode_pseudocode: String,
    /// 실행 의사코드
    pub(crate) operation: String,
    /// 에일리어스 참조
    pub(crate) aliases: Vec<String>,
    /// 운영 노트
    pub(crate) operational_notes: Vec<String>,
}

impl ArmInstruction {
    /// 파일명용 슬러그 생성: heading + id 기반 (유일성 보장)
    /// e.g., "ADD (immediate)" + id "ADD_addsub_imm" → "ADD_immediate__ADD_addsub_imm"
    /// heading만으로는 충돌 가능하므로 항상 id를 접미사로 붙임
    pub(crate) fn filename_slug(&self) -> String {
        let base = self
            .heading
            .replace('(', "")
            .replace(')', "")
            .replace(',', "")
            .replace('/', "_")
            .replace('.', "_")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("_");
        if base.is_empty() {
            return self.id.clone();
        }
        if self.id.is_empty() {
            return base;
        }
        // heading slug + XML id로 유일성 보장
        format!("{base}__{}", self.id)
    }

    /// 인스트럭션 이름 반환 (니모닉)
    pub(crate) fn get_instruction_name(&self) -> String {
        self.mnemonic.clone()
    }

    /// Markdown으로 변환
    pub(crate) fn into_md(self) -> Vec<String> {
        let mut result = Vec::new();

        // 제목
        result.push(format!("# {}", self.heading));

        // 요약
        result.push("".to_owned());
        result.push(self.brief.clone());

        // 인코딩 변형 목록
        if !self.encodings.is_empty() {
            result.push("".to_owned());
            for enc in &self.encodings {
                if !enc.label.is_empty() {
                    result.push(format!("- `{}` — {}", enc.asm_template, enc.label));
                } else {
                    result.push(format!("- `{}`", enc.asm_template));
                }
            }
        }

        // 인코딩 다이어그램
        if !self.bitfields.is_empty() {
            result.push("".to_owned());
            result.push("## Encoding".to_owned());
            result.push("".to_owned());
            result.append(&mut self.bitfields_to_md());
        }

        // 상세 설명
        if !self.description.is_empty() {
            result.push("".to_owned());
            result.push("## Description".to_owned());
            result.push("".to_owned());
            for line in &self.description {
                result.push(line.clone());
            }
        }

        // 오퍼랜드 설명
        if !self.operand_explanations.is_empty() {
            result.push("".to_owned());
            result.push("## Operands".to_owned());
            result.push("".to_owned());
            for (sym, desc) in &self.operand_explanations {
                result.push(format!("- `{sym}` — {desc}"));
            }
        }

        // 에일리어스
        if !self.aliases.is_empty() {
            result.push("".to_owned());
            result.push("## Aliases".to_owned());
            result.push("".to_owned());
            for alias in &self.aliases {
                result.push(format!("- {alias}"));
            }
        }

        // 디코드 의사코드
        if !self.decode_pseudocode.is_empty() {
            result.push("".to_owned());
            result.push("## Decode".to_owned());
            result.push("".to_owned());
            result.push("```".to_owned());
            for line in self.decode_pseudocode.lines() {
                result.push(line.to_owned());
            }
            result.push("```".to_owned());
        }

        // 실행 의사코드
        if !self.operation.is_empty() {
            result.push("".to_owned());
            result.push("## Operation".to_owned());
            result.push("".to_owned());
            result.push("```".to_owned());
            for line in self.operation.lines() {
                result.push(line.to_owned());
            }
            result.push("```".to_owned());
        }

        // 운영 노트
        if !self.operational_notes.is_empty() {
            result.push("".to_owned());
            result.push("## Operational Notes".to_owned());
            result.push("".to_owned());
            for note in &self.operational_notes {
                result.push(format!("- {note}"));
            }
        }

        result.push("".to_owned());
        result
    }

    fn bitfields_to_md(&self) -> Vec<String> {
        if self.bitfields.is_empty() {
            return Vec::new();
        }

        // 헤더: 비트 위치
        let mut headers = Vec::new();
        let mut values = Vec::new();
        for bf in &self.bitfields {
            let label = if bf.width == 1 {
                format!("{}", bf.hibit)
            } else {
                let start_bit = bf
                    .hibit
                    .checked_sub(bf.width.saturating_sub(1))
                    .unwrap_or(0);
                format!("{}:{}", bf.hibit, start_bit)
            };
            headers.push(label);

            if !bf.name.is_empty() && bf.constants.is_empty() {
                values.push(bf.name.clone());
            } else if !bf.constants.is_empty() {
                values.push(bf.constants.join(""));
            } else {
                values.push("".to_owned());
            }
        }

        let table = MdTable {
            headers,
            rows: vec![values],
        };
        table.to_md_lines()
    }
}

use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub(super) struct Instruction {
    /// 메인 인스트럭션 (AAA)
    pub(super) title: String,
    /// 인스트럭션 요약
    pub(super) summary: String,
    /// 상세 인스트럭션 별 설명
    pub(super) instruction: (String, String),
    /// 상세설명
    pub(super) description: Vec<String>,
    /// c 가상코드
    pub(super) operation: String,
    /// 영향 받는 플래그
    pub(super) flag_affected: String,
    /// 오류
    pub(super) exceptions: HashMap<String, Vec<String>>,
    /// c/c++ 대체함수
    pub(super) c_and_cpp_equivalent: Vec<String>,
}

impl Instruction {
    /// Parse instruction name
    pub(super) fn get_instructions_name(&self) -> Vec<String> {
        [self.title.clone()].into()
    }
    /// Instruction to result string
    pub(super) fn into_md(self) -> Vec<String> {
        let mut result = Vec::new();

        // 제목
        result.push(format!("# {title}", title = self.title));

        // 요약
        result.push("".to_owned());
        result.push(format!("{summary}", summary = self.summary));

        // 인스트럭션 별 설명

        // 문서 하이퍼링크

        // 설명
        let mut description = self.get_description();
        if !description.is_empty() {
            result.push("".to_owned());
            result.append(&mut description);
        }
        drop(description);

        // 호환성

        // 옵코드

        // Flags affected
        if !self.flag_affected.is_empty() {
            result.push("".to_owned());
            result.push("## Flags affected".to_owned());
            result.push(format!(
                "- {flag_affected}",
                flag_affected = self.flag_affected
            ));
        }

        // 예외
        // C/C++ 코드
        // 명령

        result.push("".to_owned());
        result
    }

    fn get_description(&self) -> Vec<String> {
        todo!()
    }
}

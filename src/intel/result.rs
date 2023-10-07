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
    pub(super) fn into_string(self) -> Vec<String> {
        todo!()
    }
}

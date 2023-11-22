#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Category {
    /// 줄거리가 포함된 라인, -을 기준으로 왼쪽이 인스트럭션, 오른쪽이 설명
    Summary,
    /// 옵코드 별 설명, Opcode로 시작하고 Description에서 끝난다. 중간 내용은 계속 바뀌며, 줄바꿈이 올 수 있다.
    /// TODO 수정 필요
    OpcodeDescription,
    OpcodeDescriptionStart,
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
    /// c/c++코드와의 동일성
    IntrinsicEquivalent,
    IntrinsicEquivalentStart,
}

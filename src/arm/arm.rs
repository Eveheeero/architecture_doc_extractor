pub(crate) mod result;
mod v1;

use result::ArmInstruction;
use std::collections::HashMap;
use std::io::Read;
use std::sync::Once;

pub fn main() {
    let xml_files = extract_xml_files();
    let instructions = v1::parse_all_instructions(xml_files);
    let saved = save_instructions(instructions);
    saved_list_to_rust_enum(saved);
}

/// tar.gz에서 XML 파일들 추출
fn extract_xml_files() -> HashMap<String, Vec<u8>> {
    let tar_gz_data = include_bytes!("arm_xml.tar.gz");
    let gz_decoder = flate2::read::GzDecoder::new(&tar_gz_data[..]);
    let mut archive = tar::Archive::new(gz_decoder);

    let mut xml_files = HashMap::new();
    for entry in archive.entries().expect("tar 아카이브 읽기 실패") {
        let mut entry = entry.expect("tar 엔트리 읽기 실패");
        let path = entry
            .path()
            .expect("경로 읽기 실패")
            .to_string_lossy()
            .to_string();

        if path.ends_with(".xml") {
            let mut data = Vec::new();
            entry.read_to_end(&mut data).expect("XML 파일 읽기 실패");
            // 파일명만 추출
            let filename = path.rsplit('/').next().unwrap_or(&path).to_owned();
            if xml_files.contains_key(&filename) {
                panic!("중복 XML 파일명: {filename} (경로: {path})");
            }
            xml_files.insert(filename, data);
        }
    }

    tracing::debug!("Extracted {} XML files from tar.gz", xml_files.len());
    xml_files
}

/// 인스트럭션을 MD 파일로 저장
fn save_instructions(instructions: Vec<ArmInstruction>) -> Vec<(String, Vec<String>)> {
    let mut saved: Vec<(String, Vec<String>)> = Vec::new();
    static INIT_DIRECTORY: Once = Once::new();
    INIT_DIRECTORY.call_once(|| {
        std::fs::create_dir_all("result/arm").expect("ARM 결과 디렉토리 생성 불가");
    });

    for instr in instructions {
        let slug = instr.filename_slug();
        let mnemonic = instr.get_instruction_name();
        tracing::debug!("{} 페이지 생성중", instr.heading);

        let md_contents: Vec<String> = instr.into_md();

        let filepath = format!("result/arm/{slug}.md");
        std::fs::write(&filepath, md_contents.join("\n"))
            .unwrap_or_else(|e| tracing::warn!("{filepath} 생성 실패: {e}"));

        saved.push((mnemonic, md_contents));
    }

    saved
}

/// Rust enum 생성: 니모닉별로 첫 번째 variant의 docs 사용
fn saved_list_to_rust_enum(saved: Vec<(String, Vec<String>)>) {
    // 니모닉별 첫 번째 variant의 docs만 사용
    let mut mnemonic_docs: HashMap<String, Vec<String>> = HashMap::new();
    for (mnemonic, docs) in &saved {
        mnemonic_docs
            .entry(mnemonic.clone())
            .or_insert_with(|| docs.clone());
    }

    let mut keys: Vec<String> = mnemonic_docs.keys().cloned().collect();
    keys.sort();

    let mut result = Vec::new();
    result.push("enum Aarch64 {".into());

    let mut seen_variants: HashMap<String, usize> = HashMap::new();
    for mnemonic in keys {
        let docs = mnemonic_docs.remove(&mnemonic).unwrap();
        for line in docs.iter() {
            if line.is_empty() {
                result.push("    ///".into());
            } else {
                result.push(format!("    /// {}", line));
            }
        }
        let variant = sanitize_rust_ident(&mnemonic);
        let count = seen_variants.entry(variant.clone()).or_insert(0);
        let final_variant = if *count == 0 {
            variant.clone()
        } else {
            format!("{variant}_{count}")
        };
        *seen_variants.get_mut(&variant).unwrap() += 1;
        result.push(format!("    {final_variant},"));
    }
    result.push("}".into());

    std::fs::write("result/arm.rs", result.join("\n")).expect("ARM 모듈 생성 실패");
}

/// 니모닉을 유효한 Rust enum variant 이름으로 변환
fn sanitize_rust_ident(mnemonic: &str) -> String {
    let mut result = String::new();
    for (i, c) in mnemonic.chars().enumerate() {
        if c.is_ascii_alphanumeric() || c == '_' {
            if i == 0 {
                result.push(c.to_ascii_uppercase());
            } else {
                result.push(c.to_ascii_lowercase());
            }
        } else if c == '.' {
            // B.cond → B_cond
            result.push('_');
        }
        // 기타 특수문자 무시
    }
    if result.is_empty() {
        return "Unknown".to_owned();
    }
    // 숫자로 시작하면 접두사 추가
    if result.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        result = format!("X{result}");
    }
    result
}

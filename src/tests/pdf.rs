#[test]
fn extract_page() {
    crate::setup_logger();
    let doc = lopdf::Document::load("src/intel/intel.pdf").unwrap();
    let contents1 = crate::pdf::get_page_contents(&doc, 129);
    let contents2 = crate::pdf::get_page_contents2(&doc, 129);

    for (contents1, contents2) in std::iter::zip(&contents1.operations, contents2.operations) {
        assert_eq!(contents1.operator, contents2.operator);
    }
}

#[test]
fn print_page_contents() {
    crate::setup_logger();
    let doc = lopdf::Document::load("src/intel/intel.pdf").unwrap();
    let contents = crate::pdf::get_page_contents(&doc, 129);
    for operation in contents.operations {
        if !matches!(
            operation.operator.as_str(),
            "Tj" | "TJ" | "TD" | "Tm" | "Tlm"
        ) {
            continue;
        }
        println!("{} {:?}", operation.operator, operation.operands);
    }
}

#[test]
fn extract_page_texts() {
    crate::setup_logger();
    let doc = lopdf::Document::load("src/intel/intel.pdf").unwrap();
    let texts = crate::pdf::page_to_texts(&doc, 129);
    for text in texts {
        println!("{}", text);
    }
}

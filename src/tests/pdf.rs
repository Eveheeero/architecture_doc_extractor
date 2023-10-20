#[test]
fn extract_page() {
    let doc = lopdf::Document::load("src/intel/intel.pdf").unwrap();
    let contents1 = crate::pdf::get_page_contents(&doc, 129);
    let contents2 = crate::pdf::get_page_contents2(&doc, 129);

    for (contents1, contents2) in std::iter::zip(&contents1.operations, contents2.operations) {
        assert_eq!(contents1.operator, contents2.operator);
    }
}

#[test]
fn print_page_contents() {
    let doc = lopdf::Document::load("src/intel/intel.pdf").unwrap();
    let contents = crate::pdf::get_page_contents(&doc, 129);
    for operation in contents.operations {
        println!("{} {:?}", operation.operator, operation.operands);
    }
}

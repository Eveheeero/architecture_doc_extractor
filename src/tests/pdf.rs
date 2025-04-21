fn get_pdf() -> &'static lopdf::Document {
    static ONCE: std::sync::OnceLock<lopdf::Document> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| lopdf::Document::load("src/intel/intel.pdf").unwrap())
}

#[test]
fn extract_page() {
    crate::setup_logger();
    let doc = get_pdf();
    let contents1 = crate::pdf::get_page_contents(&doc, 129);
    let contents2 = crate::pdf::get_page_contents2(&doc, 129);

    for (contents1, contents2) in std::iter::zip(&contents1.operations, contents2.operations) {
        assert_eq!(contents1.operator, contents2.operator);
    }
}

#[test]
fn print_page_contents() {
    crate::setup_logger();
    let doc = get_pdf();
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
fn extract_page_texts_v1() {
    crate::setup_logger();
    let doc = get_pdf();
    let texts = crate::pdf::page_to_texts_v1(&doc, 129);
    for text in texts {
        println!("{}", text);
    }
}

#[test]
fn char_width() {
    crate::setup_logger();
    let doc = get_pdf();
    let page = 1;
    assert_eq!(crate::pdf::get_char_width(&doc, page, "TT4", b' '), 0.247);
    assert_eq!(crate::pdf::get_char_width(&doc, page, "TT4", b'!'), 0.194);

    let fonts = crate::pdf::get_pdf_fonts(doc, page);
    let tt4 = fonts.get("TT4");
    assert!(tt4.is_some());
    let tt4 = tt4.unwrap();
    assert_eq!(tt4.get_char_width(b' '), 0.247);
    assert_eq!(tt4.get_char_width(b'!'), 0.194);
}

#[test]
fn extract_page_texts_v2() {
    crate::setup_logger();
    let doc = get_pdf();
    let page = 129;
    let mut boxes = crate::pdf::page_to_boxes_v2(&doc, page);
    boxes.prepare_cells();
    let chars = crate::pdf::v2::operator_to_chars(
        crate::pdf::get_pdf_fonts(doc, page),
        crate::pdf::get_page_contents(doc, page).operations,
    );
    let mut texts = crate::pdf::v2::detect_strings(chars);
    crate::pdf::v2::sort_strings(&mut texts);
    for text in texts {
        println!("{}", text.get());
    }
    assert!(true);
}

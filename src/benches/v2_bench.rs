#![feature(test)]
extern crate test;

use architecture_doc_extractor::pdf::v2::operator_to_boxes;
use lopdf::{content::Operation, Object};
use test::Bencher;

fn box1(vec: &mut Vec<Operation>) {
    vec.push(Operation::new(
        "re",
        vec![
            Object::Integer(100),
            Object::Integer(600),
            Object::Integer(1800),
            Object::Integer(1200),
        ],
    ));
    vec.push(Operation::new("f", vec![]));
}

#[bench]
fn bench_operator_to_boxes(b: &mut Bencher) {
    let mut ops = Vec::new();
    box1(&mut ops);
    b.iter(|| {
        operator_to_boxes(ops.clone());
    });
}

#[bench]
fn bench_pdf_boxes_prepare_cells(b: &mut Bencher) {
    let mut ops = Vec::new();
    box1(&mut ops);
    let boxes = operator_to_boxes(ops.clone());
    b.iter(|| {
        let mut boxes = boxes.clone();
        boxes.prepare_cells();
    });
}

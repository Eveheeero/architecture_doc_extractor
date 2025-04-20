#![feature(test)]
extern crate test;

use test::Bencher;

#[bench]
fn bench(b: &mut Bencher) {}

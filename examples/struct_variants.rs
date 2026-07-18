//! Struct-style and multi-field tuple enum variants (issue #4), fully
//! derived — contrast with `examples/struct_enum.rs`, which deliberately
//! hand-writes its `Encodeable` impl to show what the derive used to be
//! unable to generate.

use minnow::Encodeable;

#[derive(Debug, Encodeable)]
pub enum Shape {
    Point,
    Circle {
        #[encode(float(min = 0.0, max = 1_000.0, precision = 1))]
        radius: f64,
    },
    Rectangle(
        #[encode(float(min = 0.0, max = 1_000.0, precision = 1))] f64,
        #[encode(float(min = 0.0, max = 1_000.0, precision = 1))] f64,
    ),
}

fn main() {
    println!("size report:\n{}", Shape::size_report());

    for input in [
        Shape::Point,
        Shape::Circle { radius: 12.5 },
        Shape::Rectangle(3.0, 4.0),
    ] {
        println!("input: {input:?}");

        let compressed = input.encode_bytes();
        println!("bytes: {}", compressed.len());

        let output = Shape::decode_bytes(&compressed).expect("round-trip should succeed");
        println!("output: {output:?}");
    }
}

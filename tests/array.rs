use minnow::Encodeable;

#[derive(Debug, Encodeable, PartialEq)]
pub struct MyStruct {
    #[encode(float(min = 0.0, max = 5_000.0, precision = 0))]
    three_vec: [f64; 3],
}

#[test]
fn round_trip() {
    let input = MyStruct {
        three_vec: [1.0, 2.0, 3.0],
    };

    let compressed = input.encode_bytes();

    let output = MyStruct::decode_bytes(&compressed);

    assert_eq!(input, output);
}

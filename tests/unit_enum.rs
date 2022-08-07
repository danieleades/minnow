use minnow::Encodeable;

#[derive(Debug, Encodeable, PartialEq, Eq)]
pub enum MyEnum {
    A,
    B,
    C,
}

#[test]
fn round_trip() {
    let input = MyEnum::B;

    let compressed = input.encode_bytes();

    let output = MyEnum::decode_bytes(&compressed);

    assert_eq!(input, output);
}

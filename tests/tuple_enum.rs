use minnow::Encodeable;

#[derive(Debug, Encodeable, PartialEq)]
pub enum MyEnum {
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
    A(f64),
    #[encode(float(min = 0.0, max = 5_000.0, precision = 0))]
    B(f64),
}

#[derive(Debug, Encodeable, PartialEq)]
pub enum MyNestedEnum {
    A(MyEnum),
    B(MyEnum),
}

#[test]
fn round_trip() {
    let input = MyNestedEnum::B(MyEnum::A(5.0));

    let compressed = input.encode_bytes();

    let output = MyNestedEnum::decode_bytes(&compressed);

    assert_eq!(input, output);
}

use minnow::Encodeable;

#[derive(Debug, Encodeable, PartialEq)]
pub struct MyStruct(
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))] f64,
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))] f64,
);

#[derive(Debug, Encodeable, PartialEq)]
pub struct MyNestedStruct(MyStruct);

#[test]
fn round_trip() {
    let input = MyNestedStruct(MyStruct(5.0, 10.0));

    let compressed = input.encode_bytes();

    let output = MyNestedStruct::decode_bytes(&compressed);

    assert_eq!(input, output);
}

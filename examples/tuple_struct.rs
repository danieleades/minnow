use guppy::Encodeable;

#[derive(Debug, Encodeable)]
pub struct MyStruct(
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))] f64,
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))] f64,
);

#[derive(Debug, Encodeable)]
pub struct MyNestedStruct(MyStruct);

fn main() {
    let input = MyNestedStruct(MyStruct(5.0, 10.0));

    println!("input: {:?}", input);

    let compressed = input.encode_bytes().unwrap();
    println!("bytes: {}", compressed.len());

    let output = MyNestedStruct::decode_bytes(&compressed).unwrap();
    println!("output: {:?}", output);
}

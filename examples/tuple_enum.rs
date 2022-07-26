use guppy::Encodeable;

#[derive(Debug, Encodeable)]
pub enum MyEnum {
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
    A(f64),
    #[encode(float(min = 0.0, max = 5_000.0, precision = 0))]
    B(f64),
}

#[derive(Debug, Encodeable)]
pub enum MyNestedEnum {
    A(MyEnum),
    B(MyEnum),
}

fn main() {
    let input = MyNestedEnum::B(MyEnum::A(5.0));

    println!("input: {:?}", input);

    let compressed = input.encode_bytes().unwrap();
    println!("bytes: {}", compressed.len());

    let output = MyNestedEnum::decode_bytes(&compressed).unwrap();
    println!("output: {:?}", output);
}

use minnow::Encodeable;

#[derive(Debug, Encodeable)]
pub enum MyEnum {
    A,
    B,
    C,
}

fn main() {
    let input = MyEnum::B;

    println!("input: {:?}", input);

    let compressed = input.encode_bytes().unwrap();
    println!("bytes: {}", compressed.len());

    let output = MyEnum::decode_bytes(&compressed).unwrap();
    println!("output: {:?}", output);
}

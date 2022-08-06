# Minnow

Minnow is a library for serialising objects into extremely compact binary representations using [arithmetic coding](https://en.wikipedia.org/wiki/Arithmetic_coding).

Minnow is a derive macro and convenience layer over the [Arithmetic-Coding](https://github.com/danieleades/arithmetic-coding) library.

```rust
use minnow::Encodeable;

#[derive(Debug, Encodeable, PartialEq)]
pub struct NavigationReport {
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
    pub x: f64,
    #[encode(float(min = -10_000.0, max = 10_000.0, precision = 1))]
    pub y: f64,
    #[encode(float(min = -5_000.0, max = 0.0, precision = 0))]
    pub z: f64,
    pub vehicle_class: Option<VehicleClass>,
    pub battery_ok: Option<bool>,
}

#[derive(Debug, Encodeable, PartialEq)]
pub enum VehicleClass {
    Auv,
    Usv,
    Ship,
}

let input = NavigationReport {
    x: 450.0,
    y: 550.0,
    z: -100.0,
    vehicle_class: Some(VehicleClass::Auv),
    battery_ok: Some(true),
};

let compressed = input.encode_bytes().unwrap();
let output = NavigationReport::decode_bytes(&compressed).unwrap();

assert_eq!(input, output);
```

Minnow was originally conceived as a library for creating compact messages for underwater acoustic communications. It is heavily inspired by [Dynamic Compact Control Language (DCCL)](https://libdccl.org/3.0/)

## Licensing

This project is publicly available under the GNU General Public License v3.0. It may optionally be distibruted under the permissive MIT license by commercial arrangement.

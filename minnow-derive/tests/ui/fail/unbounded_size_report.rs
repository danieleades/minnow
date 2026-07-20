// `#[encode(unbounded)]` opts a schema out of the `Bounded` impl, so asking
// it for a size report (or a length-validated decode) is a compile error —
// the budget guarantee cannot silently disappear at runtime.
use minnow::Bounded;
use minnow_derive::Encodeable;

#[derive(Encodeable)]
#[encode(unbounded)]
pub struct Telemetry {
    pub healthy: bool,
}

fn main() {
    let _ = Telemetry::size_report();
}

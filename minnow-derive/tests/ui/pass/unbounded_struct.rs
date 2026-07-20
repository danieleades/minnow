// `#[encode(unbounded)]`: a schema containing an `Encodeable`-only
// (unbounded) field type compiles once the missing budget is acknowledged —
// it gets the codec impl but no `Bounded` impl.
use minnow::{DecodeVisitor, EncodeVisitor};
use minnow_derive::Encodeable;

pub struct Varint(pub u64);

impl minnow::Encodeable for Varint {
    type Config = ();

    fn encode_with_config<W>(
        &self,
        _visitor: &mut EncodeVisitor<W>,
        _config: (),
    ) -> Result<(), minnow::EncodeError>
    where
        W: bitstream_io::BitWrite,
    {
        Ok(())
    }

    fn decode_with_config<R>(
        _visitor: &mut DecodeVisitor<R>,
        _config: (),
    ) -> Result<Self, minnow::DecodeError>
    where
        R: bitstream_io::BitRead,
    {
        Ok(Self(0))
    }
}

#[derive(Encodeable)]
#[encode(unbounded)]
pub struct Telemetry {
    pub healthy: bool,
    pub uptime: Varint,
}

fn main() {}

// A field whose type implements only `Encodeable` (no `Bounded`) has no
// weight, so the derived `Bounded` impl cannot exist. Without an explicit
// `#[encode(unbounded)]` acknowledging the missing budget, the derive must
// fail, with the error pointing at the offending field type.
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
pub struct Telemetry {
    pub uptime: Varint,
}

fn main() {}

//! Round-trip tests for `String` (issue #3), both hand-configured
//! (`StringModel`) and via the `#[encode(string(...))]` derive sugar.

use minnow::{DecodeError, Encodeable, EncodeableCustom, StringModel};

#[derive(Debug, Encodeable, PartialEq, Clone)]
pub struct Message {
    #[encode(string(max_length = 32))]
    pub text: String,
}

#[test]
fn round_trips_empty_and_max_and_multibyte() {
    let config = StringModel::new(16).unwrap();

    // Empty.
    let bytes = String::new().encode_bytes_with_config(config);
    assert_eq!(
        String::decode_bytes_with_config(&bytes, config).unwrap(),
        ""
    );

    // Exactly max_length bytes (ASCII, so 16 bytes == 16 chars).
    let max = "0123456789abcdef".to_string();
    assert_eq!(max.len(), 16);
    let bytes = max.clone().encode_bytes_with_config(config);
    assert_eq!(
        String::decode_bytes_with_config(&bytes, config).unwrap(),
        max
    );

    // Multibyte UTF-8 (emoji, accented characters), well under the byte
    // budget.
    for s in ["héllo", "日本語", "🦀🎉", "café"] {
        let bytes = s.to_string().encode_bytes_with_config(config);
        assert_eq!(String::decode_bytes_with_config(&bytes, config).unwrap(), s);
    }
}

#[test]
fn derive_string_sugar_round_trips() {
    for text in ["", "hello, world!", "日本語のテスト"] {
        let value = Message {
            text: text.to_string(),
        };
        let bytes = value.encode_bytes();
        let decoded = Message::decode_bytes(&bytes).unwrap();
        assert_eq!(decoded, value);
    }
}

#[test]
fn encoding_beyond_max_length_is_rejected_not_panicked() {
    let value = Message {
        // Each 'x' is a 3-byte character (well beyond the 32-byte budget in
        // total), exercising the byte-length (not char-count) bound.
        text: "€".repeat(20),
    };
    // `encode_bytes` panics on I/O failure (infallible for `Vec<u8>` in
    // general), but exceeding `max_length` is a configuration error the
    // fallible `encode_with_config` path must surface cleanly rather than
    // panic.
    let mut writer = bitstream_io::BitWriter::endian(Vec::new(), bitstream_io::BigEndian);
    let mut encoder = minnow::EncodeVisitor::new(minnow::PRECISION, &mut writer);
    let config = minnow_derive_config();
    let err = value
        .text
        .encode_with_config(&mut encoder, config)
        .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

fn minnow_derive_config() -> StringModel {
    StringModel::new(32).unwrap()
}

#[test]
fn corrupt_bytes_never_panic_and_invalid_utf8_is_reported() {
    // Feed a hand-built, well-formed-length-but-corrupt byte sequence through
    // `String`'s decode path and confirm it either decodes or reports
    // `InvalidUtf8`/other `DecodeError` — never panics.
    let config = StringModel::new(8).unwrap();
    for len in 0..8 {
        for fill in [0x00_u8, 0xff_u8] {
            let raw_bytes: Vec<u8> = vec![fill; len];
            let encoded = raw_bytes.encode_bytes_with_config(minnow::SeqModel {
                max_len: 8,
                elem: minnow::IntModel::<u8>::default(),
            });
            let result = String::decode_bytes_with_config(&encoded, config);
            // Either it's a valid (if degenerate) UTF-8 string, or it's
            // reported as invalid UTF-8 -- either way, no panic, and any
            // error is a `DecodeError`.
            if let Err(err) = result {
                assert!(matches!(
                    err,
                    DecodeError::InvalidUtf8(_) | DecodeError::Length { .. }
                ));
            }
        }
    }
}

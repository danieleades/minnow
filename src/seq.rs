//! A length-prefixed model for bounded sequences (`Vec<T>`, issue #5), and the
//! byte-sequence machinery [`crate::StringModel`] builds on.
//!
//! # Wire format
//!
//! A sequence of at most `max_len` elements is encoded as:
//!
//! 1. a **uniform** length symbol over `0..=max_len` (`max_len + 1` equally
//!    likely values), costing exactly `log₂(max_len + 1)` bits;
//! 2. that many elements, each encoded with the shared `elem` config.
//!
//! # The math
//!
//! Per the cardinality semiring (see [`crate::Weight`]), a bounded sequence is
//! a sum-of-products over its possible lengths:
//!
//! ```text
//! W(Vec<T>) = Σ_{k=0}^{L} W(T)^k        (saturating)
//! ```
//!
//! and the bit-accounting methods follow directly from the uniform length
//! prefix:
//!
//! ```text
//! worst_case_bits = log₂(L + 1) + L · worst_case_bits(T)   (the full vector)
//! best_case_bits  = log₂(L + 1)                            (the empty vector)
//! ```
//!
//! # Deliberate design decision: uniform, not cardinality-weighted, length
//!
//! A *cardinality*-weighted length prefix (interval width `W(T)^k` for length
//! `k`, à la [`crate::WeightedModel`]) would make every value of `Vec<T>` cost
//! exactly `log₂ W(Vec<T>)` bits — matching the minimax-optimal scheme used
//! for enums (see the crate documentation). But it would also make the
//! **empty** vector cost almost as much as the **longest** one, since nearly
//! all of `W(Vec<T>)`'s cardinality lives in the `k = L` term. That defeats
//! the purpose of a variable-length field: short values should be cheap. The
//! uniform length prefix trades a worst-case redundancy of
//! `log₂(L + 1) − log₂(A / (A − 1))` bits (where `A = W(T)`) for exactly that
//! property — a short vector costs `log₂(L + 1) + k · log₂ A`, not
//! `log₂ W(Vec<T>)` regardless of `k`. A cardinality-weighted opt-in can be
//! added later; it is not implemented here.
//!
//! Because the length prefix is uniform, `Vec<T>`'s `best_case_bits` and
//! `worst_case_bits` genuinely differ (unlike every uniformly-weighted
//! fixed-size type), so the pre-decode length window (see
//! [`crate::DecodeError::Length`]) is wide rather than pinned to an exact
//! value — it still never rejects a genuinely valid encoding.

use std::{convert::Infallible, ops::Range};

use arithmetic_coding::one_shot;
use bitstream_io::{BitRead, BitWrite};

use crate::{
    Bounded, DecodeError, DecodeVisitor, EncodeError, EncodeVisitor, Encodeable, SizeReport, Weight,
};

/// A uniform one-shot model over `0..=max_len` — the length prefix shared by
/// [`SeqModel`]/[`crate::StringModel`].
///
/// This is a runtime-sized sibling of [`crate::OneShot`] (which needs its
/// symbol count as a `const` generic); `max_len` is only known at runtime
/// here, since it comes from a field's config rather than the type itself.
#[derive(Debug, Clone, Copy)]
struct LengthModel {
    max_len: u32,
}

impl one_shot::Model for LengthModel {
    type B = u128;
    type Symbol = u32;
    type ValueError = Infallible;

    fn probability(&self, symbol: &Self::Symbol) -> Result<Range<Self::B>, Self::ValueError> {
        let value = u128::from(*symbol);
        #[allow(clippy::range_plus_one)]
        Ok(value..value + 1)
    }

    fn max_denominator(&self) -> Self::B {
        u128::from(self.max_len) + 1
    }

    fn symbol(&self, value: Self::B) -> Self::Symbol {
        // `value < max_denominator() = max_len + 1 <= u32::MAX + 1`, but the
        // arithmetic decoder only ever produces `value` within that bound, so
        // this always fits `u32`.
        u32::try_from(value).unwrap_or(u32::MAX)
    }
}

/// The configuration for a bounded sequence: a maximum length and the
/// per-element configuration.
///
/// See the module documentation for the wire format and the weight/bit-cost
/// formulas.
#[derive(Debug, Clone, Copy)]
pub struct SeqModel<C> {
    /// The maximum number of elements the sequence may contain.
    pub max_len: u32,
    /// The configuration shared by every element.
    pub elem: C,
}

impl<T, C> Bounded for Vec<T>
where
    T: Bounded<Config = C>,
    C: Clone,
{
    fn weight(config: &Self::Config) -> Weight {
        let elem_weight = T::weight(&config.elem);

        // Closed form when every element weighs exactly one (e.g. a `Vec` of
        // a unit type): every term of the sum is `1`, so the total is simply
        // `max_len + 1`. Without this, a huge `max_len` with `elem_weight ==
        // 1` would never trip the saturation short-circuit below (the sum
        // grows by exactly 1 each iteration, so it would take `u128::MAX`
        // iterations to saturate) and the loop would never terminate in
        // practice.
        if elem_weight == Weight::ONE {
            return Weight::new(u128::from(config.max_len) + 1);
        }

        // General case: Σ_{k=0}^{max_len} elem_weight^k, saturating. This
        // loop always terminates quickly regardless of `max_len`: once
        // `elem_weight > 1`, the running total saturates (or the running
        // power underflows to zero, for `elem_weight == 0`) within at most a
        // few hundred iterations.
        let mut total = Weight::ZERO;
        let mut power = Weight::ONE;
        for _ in 0..=config.max_len {
            total = total + power;
            if total.is_saturated() || power == Weight::ZERO {
                return total;
            }
            power = power * elem_weight;
        }
        total
    }

    fn worst_case_bits(config: &Self::Config) -> f64 {
        let length_bits = length_bits(config.max_len);
        f64::from(config.max_len).mul_add(T::worst_case_bits(&config.elem), length_bits)
    }

    fn best_case_bits(config: &Self::Config) -> f64 {
        // The cheapest value is the empty vector: just the length prefix.
        length_bits(config.max_len)
    }

    fn report(config: &Self::Config) -> SizeReport {
        // A compact tree — a `length` leaf plus one element *template* —
        // rather than one node per slot: `max_len` may be in the billions,
        // and this report is built on every `decode_bytes` call. The node's
        // bit total is computed arithmetically instead of summed from
        // children, so it stays exact.
        let length_leaf = SizeReport::leaf(length_bits(config.max_len)).with_name("length");
        let element =
            T::report(&config.elem).with_name(format!("element (worst case × {})", config.max_len));
        SizeReport {
            name: None,
            bits: Self::worst_case_bits(config),
            children: vec![length_leaf, element],
        }
    }
}

// The codec needs only `T: Encodeable`: the length prefix is *uniform* over
// `0..=max_len` (see the module docs), so unlike `Option`'s weighted
// discriminant it never consults the element's cardinality. A bounded-length
// `Vec` of an unbounded element type therefore encodes fine — it just has no
// size budget.
impl<T, C> Encodeable for Vec<T>
where
    T: Encodeable<Config = C>,
    C: Clone,
{
    type Config = SeqModel<C>;

    fn encode_with_config<W>(
        &self,
        visitor: &mut EncodeVisitor<W>,
        config: Self::Config,
    ) -> Result<(), EncodeError>
    where
        W: BitWrite,
    {
        let len = self.len();
        if len > config.max_len as usize {
            // No clamping mode for sequences: truncation would silently
            // drop elements, which is not a nearest-value projection.
            return Err(EncodeError::TooLong {
                len,
                max_len: config.max_len,
            });
        }
        // `len <= config.max_len`, a `u32`, so this cast cannot truncate.
        #[allow(clippy::cast_possible_truncation)]
        let len_symbol = len as u32;

        visitor.encode_one(
            LengthModel {
                max_len: config.max_len,
            },
            &len_symbol,
        )?;
        for item in self {
            item.encode_with_config(visitor, config.elem.clone())?;
        }
        Ok(())
    }

    fn decode_with_config<R>(
        visitor: &mut DecodeVisitor<R>,
        config: Self::Config,
    ) -> Result<Self, DecodeError>
    where
        R: BitRead,
        Self: Sized,
    {
        let len = visitor.decode_one(LengthModel {
            max_len: config.max_len,
        })?;
        // Defensive: the length model's own denominator (`max_len + 1`) means
        // the arithmetic decoder can never actually produce a value outside
        // `0..=max_len`, but a corrupt stream must never be trusted to
        // preserve that invariant, and `Vec::with_capacity` below must never
        // be handed an unvalidated, attacker-controlled length.
        if len > config.max_len {
            return Err(DecodeError::InvalidSymbol {
                symbol: u128::from(len),
            });
        }

        let mut result = Vec::with_capacity(len as usize);
        for _ in 0..len {
            result.push(T::decode_with_config(visitor, config.elem.clone())?);
        }
        Ok(result)
    }
}

/// `log₂(max_len + 1)`, computed without risking `u32` overflow (`max_len`
/// may be `u32::MAX`).
fn length_bits(max_len: u32) -> f64 {
    (f64::from(max_len) + 1.0).log2()
}

#[cfg(test)]
mod tests {
    use arithmetic_coding::one_shot::Model;

    use super::{LengthModel, SeqModel};
    use crate::{Bounded, Encodeable, Weight};

    #[test]
    fn length_model_covers_full_range() {
        let model = LengthModel { max_len: 5 };
        assert_eq!(model.max_denominator(), 6);
        for symbol in 0..=5_u32 {
            let range = model.probability(&symbol).unwrap();
            assert_eq!(model.symbol(range.start), symbol);
        }
    }

    #[test]
    fn weight_matches_geometric_sum() {
        // elem weight 2 (bool), max_len 3: 1 + 2 + 4 + 8 = 15.
        let config = SeqModel {
            max_len: 3,
            elem: (),
        };
        assert_eq!(<Vec<bool> as Bounded>::weight(&config), Weight::new(15));
    }

    #[test]
    fn weight_with_unit_element_is_length_plus_one() {
        // elem weight 1 (unit type): every length costs the same, so the sum
        // is exactly max_len + 1. Also exercises that a huge max_len doesn't
        // hang: this would need u32::MAX iterations without the closed form.
        let config = SeqModel {
            max_len: u32::MAX,
            elem: (),
        };
        assert_eq!(
            <Vec<()> as Bounded>::weight(&config),
            Weight::new(u128::from(u32::MAX) + 1)
        );
    }

    #[test]
    fn best_case_matches_empty_vec() {
        let config = SeqModel {
            max_len: 10,
            elem: (),
        };
        let empty: Vec<bool> = Vec::new();
        let bytes = empty.encode_bytes_with_config(config).unwrap();

        // The empty vector is the cheapest value: just the length prefix,
        // `log2(11)` bits.
        let best_bits = <Vec<bool> as Bounded>::best_case_bits(&config);
        assert!((best_bits - 11_f64.log2()).abs() < 1e-9);
        assert!(bytes.len() <= <Vec<bool> as Bounded>::report(&config).total_bytes());
    }

    #[test]
    fn round_trips() {
        let config = SeqModel {
            max_len: 5,
            elem: (),
        };
        for len in 0..=5 {
            let value: Vec<bool> = vec![true; len];
            let bytes = value.encode_bytes_with_config(config).unwrap();
            let decoded = <Vec<bool>>::decode_bytes_with_config(&bytes, config).unwrap();
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn encoding_over_max_len_is_rejected() {
        let config = SeqModel {
            max_len: 2,
            elem: (),
        };
        let value: Vec<bool> = vec![true; 3];
        let mut writer = bitstream_io::BitWriter::endian(Vec::new(), bitstream_io::BigEndian);
        let mut encoder = crate::EncodeVisitor::new(crate::PRECISION, &mut writer);
        let err = value.encode_with_config(&mut encoder, config).unwrap_err();
        assert!(matches!(err, crate::EncodeError::TooLong { .. }));
    }

    /// The report (and the decode length check that used to build it) must
    /// stay `O(1)` in `max_len`: a huge but valid bound must not materialise
    /// one node per element slot.
    #[test]
    fn report_is_compact_for_huge_bounds() {
        let config = SeqModel {
            max_len: u32::MAX,
            elem: (),
        };
        let report = <Vec<bool>>::report(&config);
        // One `length` leaf plus one element template, regardless of the bound.
        assert_eq!(report.children.len(), 2);

        // The pre-decode length check runs without touching the report tree;
        // empty input is simply outside the schema's length window.
        let err = <Vec<bool>>::decode_bytes_with_config(&[], config).unwrap_err();
        assert!(matches!(err, crate::DecodeError::Length { .. }));
    }
}

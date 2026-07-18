//! Worst-case size reporting for encodeable schemas.
//!
//! A [`SizeReport`] is a tree mirroring the structure of an encodeable type:
//! one node per field/variant, each carrying the worst-case number of bits that
//! part contributes. It is the image of the cardinality semiring (see
//! [`crate::Weight`]) under the homomorphism `w ↦ log₂ w`, which turns the
//! product rule for structs into a **sum of per-field bit counts** and the sum
//! rule for enums into a **max over variants**.
//!
//! # Why sums in log-space
//!
//! A weight *product* overflows `u128` for large messages, after which
//! [`Weight`](crate::Weight) only reports a lower bound. Accumulating the
//! report as an `f64` sum of `log₂` terms instead stays accurate to a fraction
//! of a bit regardless of message size, so [`SizeReport`] is the authoritative
//! worst-case figure.
//!
//! # The termination constant
//!
//! Arithmetic-coding flush emits a bounded number of trailing bits, independent
//! of the coder precision. Minnow models this as [`TERMINATION_BITS`] and folds
//! it into [`SizeReport::total_bytes`] so the byte figure is a true upper bound
//! on the encoded length.

use std::fmt;

/// The maximum number of bits the arithmetic coder appends when terminating a
/// message (the "flush" overhead).
///
/// Terminating an `arithmetic-coding` 0.4 stream writes at most two bits after
/// the final symbol, regardless of the coder precision or the models used. This
/// constant is added to the fractional payload size in
/// [`SizeReport::total_bytes`] so the reported byte count is a genuine upper
/// bound on the encoded length.
pub const TERMINATION_BITS: f64 = 2.0;

/// Round a bit count up to whole bytes: `ceil(bits / 8)`.
pub(crate) fn bytes_for(bits: f64) -> usize {
    // `bits` is non-negative and far below `usize::MAX * 8` for any real schema,
    // so this cast neither loses sign nor truncates meaningfully.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let bytes = (bits / 8.0).ceil() as usize;
    bytes
}

/// A tree describing the worst-case encoded size of a schema.
///
/// Each node names a field or variant (`None` for anonymous/root nodes), the
/// worst-case `bits` it contributes (already aggregated over its subtree), and
/// its `children`. See the module documentation for how the aggregation
/// works and why it is accumulated in `f64` log-space.
///
/// Use [`total_bits`](SizeReport::total_bits) /
/// [`total_bytes`](SizeReport::total_bytes) for the headline figures, or the
/// [`Display`](fmt::Display) impl for an indented per-field breakdown.
#[derive(Debug, Clone, PartialEq)]
pub struct SizeReport {
    /// The field or variant name, or `None` for an anonymous / root node.
    pub name: Option<String>,
    /// The worst-case number of bits contributed by this node's whole subtree.
    pub bits: f64,
    /// The per-field / per-variant breakdown.
    pub children: Vec<SizeReport>,
}

impl SizeReport {
    /// A leaf carrying a fixed worst-case bit count and no children.
    #[must_use]
    pub fn leaf(bits: f64) -> Self {
        Self {
            name: None,
            bits,
            children: Vec::new(),
        }
    }

    /// Attach (or replace) this node's name, e.g. when embedding a field's
    /// report under its field name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// A product node (struct / tuple / array): its worst-case size is the
    /// **sum** of its children's, matching the weight product rule under
    /// `log₂`.
    #[must_use]
    pub fn product(children: Vec<SizeReport>) -> Self {
        let bits = children.iter().map(SizeReport::total_bits).sum();
        Self {
            name: None,
            bits,
            children,
        }
    }

    /// A sum node (enum): its worst-case size is the **max** over its variant
    /// children, since exactly one variant is ever encoded.
    #[must_use]
    pub fn sum(children: Vec<SizeReport>) -> Self {
        let bits = children
            .iter()
            .map(SizeReport::total_bits)
            .fold(0.0_f64, f64::max);
        Self {
            name: None,
            bits,
            children,
        }
    }

    /// A single enum variant node: the discriminant cost plus the payload's
    /// worst-case size, with a `discriminant` leaf and (for non-unit variants)
    /// the payload breakdown as children.
    #[must_use]
    pub fn enum_variant(
        name: impl Into<String>,
        discriminant_bits: f64,
        payload: SizeReport,
    ) -> Self {
        let total = discriminant_bits + payload.total_bits();
        let mut children = vec![SizeReport::leaf(discriminant_bits).with_name("discriminant")];
        if payload.bits != 0.0 || !payload.children.is_empty() {
            children.push(payload.with_name("payload"));
        }
        Self {
            name: Some(name.into()),
            bits: total,
            children,
        }
    }

    /// The worst-case number of bits for this node's subtree.
    #[must_use]
    pub fn total_bits(&self) -> f64 {
        self.bits
    }

    /// The worst-case encoded size in bytes, including coder termination.
    ///
    /// Computed as `ceil((total_bits + TERMINATION_BITS) / 8)` using
    /// [`TERMINATION_BITS`]. This is a genuine upper bound on the length of any
    /// value of the schema.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        crate::report::bytes_for(self.total_bits() + TERMINATION_BITS)
    }

    fn fmt_indented(&self, f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
        for _ in 0..depth {
            write!(f, "  ")?;
        }
        let name = self.name.as_deref().unwrap_or("total");
        if depth == 0 {
            writeln!(
                f,
                "{name}: {:.2} bits ({} bytes)",
                self.total_bits(),
                self.total_bytes()
            )?;
        } else {
            writeln!(f, "{name}: {:.2} bits", self.bits)?;
        }
        for child in &self.children {
            child.fmt_indented(f, depth + 1)?;
        }
        Ok(())
    }
}

impl fmt::Display for SizeReport {
    /// Render an indented per-field breakdown, bits to two decimal places. The
    /// root line also shows the total byte count.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_indented(f, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::{SizeReport, TERMINATION_BITS};

    #[test]
    #[allow(clippy::float_cmp)]
    fn product_sums_children() {
        let report = SizeReport::product(vec![
            SizeReport::leaf(3.0).with_name("a"),
            SizeReport::leaf(5.0).with_name("b"),
        ]);
        assert_eq!(report.total_bits(), 8.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn sum_takes_max() {
        let report = SizeReport::sum(vec![SizeReport::leaf(3.0), SizeReport::leaf(5.0)]);
        assert_eq!(report.total_bits(), 5.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn bytes_round_up_with_termination() {
        // 6 payload bits + 2 termination = 8 bits = 1 byte.
        assert_eq!(SizeReport::leaf(6.0).total_bytes(), 1);
        // 7 payload bits + 2 termination = 9 bits = 2 bytes.
        assert_eq!(SizeReport::leaf(7.0).total_bytes(), 2);
        assert_eq!(TERMINATION_BITS, 2.0);
    }
}

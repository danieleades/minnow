//! Cardinality weights — the semiring that underpins Minnow's size model.
//!
//! Every encodeable type `T` (under a given configuration) has a **weight**
//! `W(T)`: the number of distinct values it can encode. Algebraic data types
//! map onto a semiring homomorphism into the natural numbers:
//!
//! | Type | Weight |
//! |------|--------|
//! | leaf model (`bool`, quantised `f64`, …) | the model's denominator `N` |
//! | product (struct, tuple, `[T; N]`) | `∏ W(fieldᵢ)` |
//! | sum (enum, [`Option<T>`]) | `Σ W(variantᵥ)` |
//!
//! # Semiring laws
//!
//! [`Weight`] forms a commutative semiring `(ℕ, +, ×, 0, 1)`:
//!
//! * `+` is associative and commutative, with identity [`Weight::ZERO`];
//! * `×` is associative and commutative, with identity [`Weight::ONE`];
//! * `×` distributes over `+`;
//! * [`Weight::ZERO`] annihilates under `×` (`0 × w = 0`).
//!
//! The size report ([`crate::SizeReport`]) is the image of this semiring under
//! the homomorphism `w ↦ log₂ w`, which turns products into sums of bit counts.
//!
//! # Why this matters (minimax optimality)
//!
//! Encoding an enum discriminant with interval widths proportional to each
//! variant's payload weight makes *every* value of a type cost exactly
//! `log₂ W(T)` bits (plus at most two bits of coder-termination overhead). That
//! is the information-theoretic minimum worst-case length for a code over
//! `W(T)` values: no code can beat `log₂ W(T)` in the worst case, and uniform
//! leaves plus weighted discriminants achieve it with equality.
//!
//! # Saturation
//!
//! A product of weights overflows `u128` for messages larger than ~16 bytes, so
//! the arithmetic here is **saturating** rather than wrapping. Saturation is
//! *sticky*: once a computation reaches [`Weight::SATURATED`] every subsequent
//! `+`/`×` stays saturated, and [`Weight::is_saturated`] lets callers detect
//! it. A saturated weight is a *lower bound* on the true cardinality, so
//! callers that need an exact figure (the size report) should accumulate in
//! `f64` log₂-space instead of reading [`Weight::log2`] off a
//! possibly-saturated product.

use std::ops::{Add, Mul};

/// The number of distinct values an encodeable type can represent.
///
/// A newtype over `u128` with saturating semiring arithmetic. See the
/// module documentation for the semiring laws and the meaning of
/// saturation.
///
/// The sentinel value [`u128::MAX`] denotes a *saturated* weight — a
/// cardinality at least that large, whose exact value was lost to overflow.
/// Construct weights with [`Weight::new`], combine them with `+`/`*` (or the
/// explicit `saturating_*` methods), and read the bit cost with
/// [`Weight::log2`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Weight(u128);

impl Weight {
    /// The multiplicative identity: a type with exactly one encodable value
    /// (for example a unit struct or unit enum variant), which costs zero bits.
    pub const ONE: Self = Self(1);
    /// A saturated weight — a cardinality at least [`u128::MAX`], whose exact
    /// value overflowed. Saturation is sticky (see the module documentation).
    pub const SATURATED: Self = Self(u128::MAX);
    /// The additive identity: a type with no encodable values.
    pub const ZERO: Self = Self(0);

    /// Construct a weight from an exact cardinality.
    #[must_use]
    pub const fn new(cardinality: u128) -> Self {
        Self(cardinality)
    }

    /// The underlying cardinality.
    ///
    /// Returns [`u128::MAX`] if the weight is
    /// [saturated](Weight::is_saturated), in which case the true
    /// cardinality is only known to be at least this large.
    #[must_use]
    pub const fn get(self) -> u128 {
        self.0
    }

    /// Whether this weight has saturated (overflowed `u128`).
    ///
    /// A saturated weight is a lower bound on the true cardinality.
    #[must_use]
    pub const fn is_saturated(self) -> bool {
        self.0 == u128::MAX
    }

    /// Saturating addition (the sum rule for enums / [`Option`]).
    ///
    /// Saturation is sticky: if either operand is saturated, or the exact sum
    /// would overflow, the result is [`Weight::SATURATED`].
    #[must_use]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        if self.is_saturated() || rhs.is_saturated() {
            return Self::SATURATED;
        }
        Self(self.0.saturating_add(rhs.0))
    }

    /// Saturating multiplication (the product rule for structs / arrays).
    ///
    /// [`Weight::ZERO`] annihilates *exactly*, even against a saturated
    /// operand: a product containing a zero-cardinality component has no
    /// values at all, however large the other factors. Otherwise saturation
    /// is sticky: if either operand is saturated, or the exact product would
    /// overflow, the result is [`Weight::SATURATED`].
    #[must_use]
    pub const fn saturating_mul(self, rhs: Self) -> Self {
        if self.0 == 0 || rhs.0 == 0 {
            return Self::ZERO;
        }
        if self.is_saturated() || rhs.is_saturated() {
            return Self::SATURATED;
        }
        Self(self.0.saturating_mul(rhs.0))
    }

    /// Saturating exponentiation (the weight of `[T; exp]` is
    /// `W(T).pow(exp)`).
    ///
    /// Saturation is sticky, exactly as for [`Weight::saturating_mul`]
    /// (`SATURATED.pow(0)` is still `ONE`, matching `x⁰ = 1`).
    #[must_use]
    pub const fn pow(self, exp: u32) -> Self {
        Self(self.0.saturating_pow(exp))
    }

    /// The number of bits needed to distinguish this many values: `log₂(w)`.
    ///
    /// This is the fundamental quantity reported by [`crate::SizeReport`]. For
    /// a [saturated](Weight::is_saturated) weight it is a lower bound;
    /// prefer accumulating `f64` bit counts directly when saturation is
    /// possible.
    #[must_use]
    pub fn log2(self) -> f64 {
        // A `u128 -> f64` cast loses precision beyond 53 bits, but a size report
        // is accurate to a fraction of a bit regardless, which is far finer than
        // any use of the figure requires.
        #[allow(clippy::cast_precision_loss)]
        let value = self.0 as f64;
        value.log2()
    }
}

impl Add for Weight {
    type Output = Self;

    /// Saturating addition; see [`Weight::saturating_add`].
    fn add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }
}

impl Mul for Weight {
    type Output = Self;

    /// Saturating multiplication; see [`Weight::saturating_mul`].
    fn mul(self, rhs: Self) -> Self {
        self.saturating_mul(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::Weight;

    #[test]
    fn identities() {
        let w = Weight::new(7);
        assert_eq!(w + Weight::ZERO, w);
        assert_eq!(w * Weight::ONE, w);
        assert_eq!(w * Weight::ZERO, Weight::ZERO);
    }

    #[test]
    fn distributive() {
        let a = Weight::new(3);
        let b = Weight::new(5);
        let c = Weight::new(7);
        assert_eq!(a * (b + c), a * b + a * c);
    }

    #[test]
    fn saturation_is_sticky() {
        let big = Weight::new(u128::MAX / 2 + 1);
        let sum = big + big;
        assert!(sum.is_saturated());
        // Once saturated, it stays saturated.
        assert!((sum + Weight::ONE).is_saturated());
        assert!((sum * Weight::new(2)).is_saturated());
        assert_eq!(sum, Weight::SATURATED);
    }

    #[test]
    fn zero_annihilates_even_when_saturated() {
        // `0 × w = 0` holds exactly for every operand, including a saturated
        // one — a product containing an empty component has no values.
        assert_eq!(Weight::SATURATED * Weight::ZERO, Weight::ZERO);
        assert_eq!(Weight::ZERO * Weight::SATURATED, Weight::ZERO);
        assert_eq!(Weight::ZERO.pow(3), Weight::ZERO);
    }

    #[test]
    fn pow_saturates() {
        assert_eq!(Weight::new(2).pow(10), Weight::new(1024));
        assert_eq!(Weight::new(3).pow(0), Weight::ONE);
        assert_eq!(Weight::SATURATED.pow(0), Weight::ONE);
        assert!(Weight::SATURATED.pow(1).is_saturated());
        assert!(Weight::new(1 << 40).pow(10).is_saturated());
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn log2_matches() {
        assert_eq!(Weight::new(4).log2(), 2.0);
        assert_eq!(Weight::new(1).log2(), 0.0);
    }
}

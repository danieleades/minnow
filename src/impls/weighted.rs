//! A one-shot model over a weighted discriminant — the engine behind
//! automatic enum weighting.

use std::{convert::Infallible, ops::Range};

use arithmetic_coding::one_shot;

use crate::MAX_DENOMINATOR;

/// A one-shot model over `k` symbols whose interval widths are proportional to
/// caller-supplied weights `w₁ … w_k`.
///
/// This generalises [`OneShot`](crate::OneShot) (which is the uniform special
/// case, all widths equal). Giving symbol `v` an interval of width `wᵥ` out of
/// a total `S = Σ wᵥ` makes it cost `log₂(S / wᵥ)` bits. When the weights are
/// the payload cardinalities of an enum's variants, the discriminant cost of a
/// variant plus its payload cost sums to `log₂ S` for **every** value — the
/// minimax-optimal uniform code over the whole type (see [`crate::Weight`]).
///
/// # Rescaling
///
/// The arithmetic coder requires the total denominator `S` to stay within
/// [`MAX_DENOMINATOR`](crate::MAX_DENOMINATOR). If the supplied weights sum to
/// more than that, they are rescaled proportionally to a total of exactly
/// `MAX_DENOMINATOR` using integer largest-remainder rounding, with every width
/// kept `≥ 1` so no symbol becomes unencodable.
///
/// Rescaling perturbs the induced distribution slightly; the extra redundancy
/// is `Σᵥ (wᵥ/S) · log₂((wᵥ/S) / (w̃ᵥ/S̃))` bits, where `w̃` are the rescaled
/// weights. At `S̃ = MAX_DENOMINATOR ≈ 2⁶²` this is negligible (far below one
/// bit) for any realistic schema.
#[derive(Debug, Clone)]
pub struct WeightedModel {
    /// Cumulative sums: `cumulative[0] = 0`, `cumulative[i+1] = cumulative[i] +
    /// wᵢ`. Length is `k + 1`.
    cumulative: Vec<u128>,
    /// The total denominator `S = Σ wᵢ` (after any rescaling), fixed at
    /// construction.
    total: u128,
}

impl WeightedModel {
    /// Build a weighted model from interval widths.
    ///
    /// Each weight is treated as at least `1` (a zero-width interval would make
    /// its symbol unencodable). If the weights sum to more than
    /// [`MAX_DENOMINATOR`](crate::MAX_DENOMINATOR) they are rescaled to fit;
    /// see the [type documentation](WeightedModel#rescaling).
    ///
    /// # Panics
    ///
    /// Panics if `weights` is empty; a model must describe at least one symbol.
    #[must_use]
    pub fn new(weights: impl IntoIterator<Item = u128>) -> Self {
        let weights: Vec<u128> = weights.into_iter().map(|w| w.max(1)).collect();
        assert!(
            !weights.is_empty(),
            "a WeightedModel needs at least one symbol"
        );

        let sum = weights.iter().copied().fold(0_u128, u128::saturating_add);

        let widths = if sum > MAX_DENOMINATOR {
            rescale(&weights, MAX_DENOMINATOR)
        } else {
            weights
        };

        let mut cumulative = Vec::with_capacity(widths.len() + 1);
        let mut acc = 0_u128;
        cumulative.push(0);
        for w in widths {
            acc += w;
            cumulative.push(acc);
        }

        Self {
            cumulative,
            total: acc,
        }
    }

    /// The number of symbols the model describes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cumulative.len() - 1
    }

    /// Whether the model describes no symbols. Always `false` for models built
    /// by [`WeightedModel::new`], which rejects empty input.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The total denominator `S` (after any rescaling).
    #[must_use]
    pub fn total(&self) -> u128 {
        self.total
    }

    /// The (possibly rescaled) interval width of symbol `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index >= self.len()`.
    #[must_use]
    pub fn width(&self, index: usize) -> u128 {
        self.cumulative[index + 1] - self.cumulative[index]
    }

    /// The worst-case bits to encode symbol `index`: `log₂(S / wᵢ)`.
    #[must_use]
    pub fn discriminant_bits(&self, index: usize) -> f64 {
        // `u128 -> f64` loses precision beyond 53 bits, but the ratio is a size
        // report accurate to a fraction of a bit, which is more than enough.
        #[allow(clippy::cast_precision_loss)]
        let total = self.total() as f64;
        #[allow(clippy::cast_precision_loss)]
        let width = self.width(index) as f64;
        (total / width).log2()
    }
}

/// Rescale `weights` proportionally so they sum to exactly `target`, using
/// integer largest-remainder rounding with every result `≥ 1`.
///
/// Assumes the weights currently sum to more than `target`, and that
/// `target > weights.len()` (guaranteed here: `target = MAX_DENOMINATOR ≈ 2⁶²`
/// dwarfs any realistic symbol count).
fn rescale(weights: &[u128], target: u128) -> Vec<u128> {
    let k = weights.len();
    // Reserve one unit for every symbol so none becomes unencodable, then
    // distribute the remaining budget proportionally.
    let budget = target - k as u128;

    // `f64` ratios are accurate enough: the resulting redundancy is documented
    // as negligible on `WeightedModel`.
    #[allow(clippy::cast_precision_loss)]
    let sum: f64 = weights.iter().map(|&w| w as f64).sum();

    let mut floors = Vec::with_capacity(k);
    let mut remainders = Vec::with_capacity(k);
    let mut used = 0_u128;
    for &w in weights {
        #[allow(clippy::cast_precision_loss)]
        let share = (w as f64 / sum) * budget as f64;
        let floor = share.floor();
        // `share <= budget` and non-negative, so this cast is in range.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let floor_u = floor as u128;
        floors.push(floor_u);
        remainders.push(share - floor);
        used += floor_u;
    }

    // `f64` rounding error could in principle push the sum of floors past the
    // integer budget; the arithmetic must not depend on that never happening.
    // Trim any excess from the largest width (which dwarfs the at-most-`k`-unit
    // excess), keeping every width `>= 0` here (`>= 1` after the reserved unit
    // is added back below).
    let excess = used.saturating_sub(budget);
    if excess > 0 {
        let largest = (0..k)
            .max_by_key(|&i| floors[i])
            .expect("weights are non-empty");
        let trim = excess.min(floors[largest]);
        floors[largest] -= trim;
        used -= trim;
    }

    // Distribute the rounding leftover (strictly fewer than `k` units) to the
    // symbols with the largest fractional remainders.
    let mut leftover = budget.saturating_sub(used);
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_by(|&a, &b| {
        remainders[b]
            .partial_cmp(&remainders[a])
            .expect("remainders are finite")
    });
    for &i in &order {
        if leftover == 0 {
            break;
        }
        floors[i] += 1;
        leftover -= 1;
    }

    // Add back the reserved unit so every width is `≥ 1`; the total is now
    // exactly `k + budget = target`.
    for f in &mut floors {
        *f += 1;
    }
    floors
}

impl one_shot::Model for WeightedModel {
    type B = u128;
    type Symbol = u32;
    type ValueError = Infallible;

    fn probability(&self, symbol: &Self::Symbol) -> Result<Range<Self::B>, Self::ValueError> {
        let index = *symbol as usize;
        debug_assert!(
            index < self.len(),
            "symbol {symbol} is out of range for a WeightedModel over {} symbols",
            self.len()
        );
        Ok(self.cumulative[index]..self.cumulative[index + 1])
    }

    fn max_denominator(&self) -> Self::B {
        self.total()
    }

    fn symbol(&self, value: Self::B) -> Self::Symbol {
        // `cumulative` is sorted and starts at 0, so `partition_point` finds the
        // interval `[cumulative[i], cumulative[i+1])` containing `value`.
        let index = self.cumulative.partition_point(|&c| c <= value) - 1;
        // `index < len <= u32::MAX` for any real schema.
        #[allow(clippy::cast_possible_truncation)]
        let symbol = index as u32;
        symbol
    }
}

#[cfg(test)]
mod tests {
    use arithmetic_coding::one_shot::Model;

    use super::WeightedModel;
    use crate::MAX_DENOMINATOR;

    #[test]
    fn probability_symbol_round_trip() {
        let model = WeightedModel::new([1, 3]);
        assert_eq!(model.total(), 4);
        assert_eq!(model.probability(&0).unwrap(), 0..1);
        assert_eq!(model.probability(&1).unwrap(), 1..4);
        // Every value in each interval maps back to that symbol.
        assert_eq!(model.symbol(0), 0);
        for v in 1..4 {
            assert_eq!(model.symbol(v), 1);
        }
    }

    #[test]
    fn binary_search_edges() {
        let model = WeightedModel::new([2, 2, 2]);
        assert_eq!(model.total(), 6);
        assert_eq!(model.symbol(0), 0);
        assert_eq!(model.symbol(1), 0);
        assert_eq!(model.symbol(2), 1);
        assert_eq!(model.symbol(3), 1);
        assert_eq!(model.symbol(4), 2);
        assert_eq!(model.symbol(5), 2);
    }

    #[test]
    fn zero_weights_become_one() {
        let model = WeightedModel::new([0, 0]);
        assert_eq!(model.total(), 2);
        assert_eq!(model.width(0), 1);
        assert_eq!(model.width(1), 1);
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn rescales_to_fit() {
        // Weights 1 : 2 : 3 scaled to sum <= MAX_DENOMINATOR.
        let unit = MAX_DENOMINATOR;
        let model = WeightedModel::new([unit, 2 * unit, 3 * unit]);
        let total = model.total();
        assert!(total <= MAX_DENOMINATOR, "total {total} exceeds the bound");
        // Every width is at least 1.
        for i in 0..model.len() {
            assert!(model.width(i) >= 1);
        }
        // Proportionality is preserved to a good approximation (1 : 2 : 3).
        let w0 = model.width(0) as f64;
        let w1 = model.width(1) as f64;
        let w2 = model.width(2) as f64;
        assert!((w1 / w0 - 2.0).abs() < 1e-6, "w1/w0 = {}", w1 / w0);
        assert!((w2 / w0 - 3.0).abs() < 1e-6, "w2/w0 = {}", w2 / w0);
    }

    #[test]
    fn max_denominator_matches_total() {
        let model = WeightedModel::new([5, 7, 11]);
        assert_eq!(Model::max_denominator(&model), 23);
    }
}

# Minnow

Minnow is a library for serialising objects into extremely compact binary representations using [arithmetic coding](https://en.wikipedia.org/wiki/Arithmetic_coding).

Minnow is a derive macro and convenience layer over the [Arithmetic-Coding](https://github.com/danieleades/arithmetic-coding) library.

Minnow was originally conceived as a library for creating compact messages for underwater acoustic communications. It is heavily inspired by [Dynamic Compact Control Language (DCCL)](https://libdccl.org/3.0/), and — as shown below — reliably beats DCCL's own worst-case size for the schemas DCCL was designed around, because Minnow spends *fractional* bits per field instead of rounding every field up to a whole number of bits.

## Licensing

This project is publicly available under the GNU General Public License v3.0. It may optionally be distributed under the permissive MIT license by commercial arrangement.

## Quick start

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

Minnow builds on **stable** Rust — no nightly toolchain or unstable features required. Decoding operates on untrusted input, so `decode_bytes` returns a `Result<Self, minnow::DecodeError>` rather than panicking: a truncated, corrupt, or malformed byte string is always reported as an error, never undefined behaviour or a crash.

`NavigationReport` above encodes to **7 bytes** — see [How Minnow compresses](#how-minnow-compresses) for exactly why.

## How Minnow compresses

Minnow's compression rests on one idea: every encodeable type `T` has a
**weight** `W(T)` — the number of distinct values it can encode — and the
number of bits needed to encode *any* value of `T` is `log₂ W(T)`, no more.
Getting every schema down to that bound, exactly, for every value, is what
the rest of this section explains.

### Weights are a semiring

| Type | Weight |
|---|---|
| leaf model (`bool`, quantised `f64`, bounded integer, …) | the model's denominator `N` |
| product (struct, tuple, `[T; N]`) | `∏ W(fieldᵢ)` |
| sum (enum, `Option<T>`) | `Σ W(variantᵥ)` |
| bounded sequence (`Vec<T>`, `String`, max length `L`) | `Σ_{k=0}^{L} W(T)^k` |

Structs multiply their fields' weights together (a product type — more
fields means more distinguishable combinations). Enums add their variants'
weights (a sum type — a value is *one* variant *or* another). This is a
commutative semiring homomorphism into the natural numbers, and the size
report (`size_report()`) is its image under `w ↦ log₂ w`, which turns
products into sums of bit counts and turns the enum sum rule into a
per-variant cost — see [`Weight`](https://docs.rs/minnow/latest/minnow/struct.Weight.html)
and [`SizeReport`](https://docs.rs/minnow/latest/minnow/struct.SizeReport.html).

### Weighted discriminants: the minimax-optimal encoding

A naive enum encoding assigns every variant the same discriminant cost
(`log₂(variant count)` bits) regardless of how much each variant's payload
actually varies. Minnow instead encodes the discriminant with a **weighted**
model: the arithmetic coder's interval is split into `k` sub-intervals whose
*widths* are proportional to each variant's payload weight, rather than all
being equal. Concretely, for a total `S = Σᵥ W(v)`, variant `v` gets an
interval of width `W(v)`, so:

```text
cost(discriminant of v) = log2(S / W(v))
cost(payload of v)      = log2(W(v))
total                   = log2(S)          — the same, for every v
```

That total is identical for every variant, and it equals `log₂ W(T)` for the
whole sum type. This is not a heuristic: `log₂ W(T)` is the
information-theoretic minimum worst-case length for a code over `W(T)`
distinguishable values (no code can beat it, by a basic counting argument),
and uniform leaves plus weighted discriminants achieve that bound with
equality for *every* value — not just on average. Encoding terminates with
at most `TERMINATION_BITS = 2.0` bits of flush overhead beyond that, from
finalising the arithmetic coder's internal state, so the true worst case is
`log₂ W(T) + 2` bits. This all falls out automatically from the derive macro:
no annotation is needed to get weighted discriminants on an enum or
`Option<T>`.

**Worked example** — `Option<VehicleClass>`, where `VehicleClass` has three
unit variants (`W(VehicleClass) = 3`):

* **Naive, uniform discriminant** (a 50/50 `None`-vs-`Some` bit, then the
  payload if present): `1 + log₂(3) ≈ 2.58` bits worst case.
* **Weighted discriminant** (what the derive actually emits: `{None: 1,
  Some: 3}`, total `S = 4`): every value — `None`, `Some(Auv)`,
  `Some(Usv)`, `Some(Ship)` — costs exactly `log₂(4) = 2.0` bits.

See `examples/weighted_enum.rs` for this worked example as runnable code
(it asserts the 2.0-bit figure), and `tests/size_law.rs` for the same
assertion as a regression test.

Applied to the quick-start example: `NavigationReport`'s two position floats
(200,001 distinguishable values each, `min = -10,000.0, max = 10,000.0,
precision = 1`) cost `log₂(200,001) ≈ 17.61` bits each; its depth float
(5,001 values, `min = -5,000.0, max = 0.0, precision = 0`) costs `log₂(5,001)
≈ 12.29` bits; the weighted `Option<VehicleClass>` costs exactly `2.00` bits;
the weighted `Option<bool>` costs exactly `log₂(3) ≈ 1.58` bits. Summed:
**51.09 bits**, which — with the 2-bit termination overhead folded in and
rounded up to a whole byte — is **7 bytes**. DCCL3's default codec, which
whole-bit-packs each field (`ceil(log₂ N)` bits, discarding the fractional
remainder), costs `18 + 18 + 13 + 2 + 2 = 53` bits for the identical schema.
Minnow's fractional accounting recovers essentially all of that 1.91-bit
slack. `examples/dccl_comparison.rs` extends this to a repeated,
bounded `Vec<NavigationReport>` and measures actual encoded bytes against
DCCL's formula across several lengths.

### Manual weights: optimising for a known prior

`#[encode(weight = N)]` on an enum variant overrides its automatic
(payload-cardinality) discriminant weight. This subsumes the automatic case
rather than replacing it: with weights set proportional to *prior
probability × payload weight*, the induced code minimises **expected**
length (the classical Shannon-optimal code for a known distribution); with
the default weights (payload weight only), it minimises **worst-case**
length. One mechanism, two provable optimality modes, chosen by what you put
in the attribute.

For example, biasing a two-variant enum so one variant is a thousand times
more likely than the other:

```rust
use minnow::{Bounded, Encodeable};

#[derive(Debug, Encodeable)]
enum Reading {
    #[encode(weight = 1000)]
    Common,
    Rare,
}

// Cardinality is unchanged (still 2 distinct values) — only the *coding*
// cost shifts.
assert_eq!(<Reading as Bounded>::weight(&()).get(), 2);

// `Common` costs log2(1001/1000) ≈ 0.0014 bits; `Rare` costs log2(1001) ≈
// 9.97 bits. An array of all-`Common` values now encodes far smaller than
// an array of all-`Rare` values, even though both are the "same type".
```

See `tests/manual_weight.rs` for this example exercised end-to-end
(including the round trip and the size comparison on repeated values).

### Sequences: a deliberate exception

For `Vec<T>` and `String`, Minnow does **not** use a cardinality-weighted
length the way it does for enum discriminants. A weighted length would make
every value of `Vec<T>` cost exactly `log₂ W(Vec<T>)` bits — matching the
enum scheme's minimax optimality — but nearly all of that cardinality lives
in the longest possible sequence (`k = L`), so the **empty** sequence would
cost almost as much as the **longest** one. That defeats the entire purpose
of a variable-length field: short values should be cheap.

Instead, Minnow prefixes a sequence with a **uniform** length symbol over
`0..=L` (`log₂(L + 1)` bits), then encodes that many elements at their own
per-element cost:

```text
worst_case_bits(Vec<T>) = log2(L + 1) + L * worst_case_bits(T)
best_case_bits(Vec<T>)  = log2(L + 1)                            (the empty sequence)
```

The trade-off, quantified precisely: relative to the minimax-optimal
`log₂ W(Vec<T>)` bound, the uniform length prefix costs `log₂(L + 1) −
log₂(A / (A − 1))` extra worst-case bits (where `A = W(T)`) — for
`A = 200,001` (one of `NavigationReport`'s float fields) and `L = 10` that's
about `3.459 − 0.000007 ≈ 3.459` bits of redundancy. In exchange, a
short sequence of length `k` costs `log₂(L + 1) + k · log₂ A` — not
`log₂ W(Vec<T>)` regardless of `k` — which is the property that actually
matters for variable-length data. A cardinality-weighted length model is a
possible opt-in follow-up; it is not implemented today.

`String` is modelled as exactly this sequence scheme over raw UTF-8 bytes
(`SeqModel<IntModel<u8>>` under the hood, `max_length` in **bytes**, each
byte uniform over all 256 values); an alphabet-restricted model (e.g.
printable ASCII, à la DCCL) would shrink the per-byte cost below 8 bits but
is not implemented here.

## Size reports

`Bounded::size_report()` returns a [`SizeReport`] tree describing the
worst-case encoded size of a schema, broken down per field/variant — this
answers "how big can this message get?" without having to encode a worst-case
value by hand. Running `examples/navigation_report.rs`:

```text
size report:
total: 51.09 bits (7 bytes)
  x: 17.61 bits
  y: 17.61 bits
  z: 12.29 bits
  vehicle_class: 2.00 bits
  battery_ok: 1.58 bits
```

`total_bytes()` (`7` above) is a genuine upper bound on `encode_bytes().len()`
for any value of the type: it is `ceil((total_bits + TERMINATION_BITS) / 8)`,
where `TERMINATION_BITS = 2.0` accounts for the arithmetic coder's flush
overhead. Under Minnow's default (automatic) weighting every value of a fixed
schema encodes to the *same* length (up to that termination rounding), so the
size report is not just an upper bound — it is, in practice, the size.

## Bounded and unbounded models

The trait surface splits Minnow's two promises apart, in the spirit of
`Iterator`/`ExactSizeIterator`:

* **`Encodeable`** is the codec: a `Config` plus encode/decode. It says
  nothing about size — open-ended varints, unbounded sequences, and adaptive
  models are all expressible at this tier.
* **`Bounded: Encodeable`** is the budget guarantee: `weight()`,
  `worst_case_bits()`/`best_case_bits()`, and `size_report()`. Everything
  DCCL-like — capacity planning, the pre-decode length window — lives here.

Boundedness propagates through the type system. `#[derive(Encodeable)]`
emits both impls, and the `Bounded` impl requires every field type to be
`Bounded` — so a schema that contains even one unbounded field simply *does
not implement* `Bounded`: calling `size_report()` on it is a **compile
error**, not a runtime `None`, and the budget guarantee cannot silently
disappear. A schema with an unbounded field must acknowledge the missing
budget explicitly with a container-level opt-out:

```rust,ignore
#[derive(Encodeable)]
#[encode(unbounded)]        // no `Bounded` impl is generated
pub struct Telemetry {
    pub healthy: bool,
    pub uptime_seconds: Varint,   // some type implementing only `Encodeable`
}
```

The from-bytes decode entry points follow the same split: `decode_bytes` /
`decode_bytes_with_config` validate the input length against the schema's
provable window first, so they require `Bounded`; an unbounded schema (which
has no window) decodes via the explicitly-named
`decode_bytes_unvalidated` / `decode_bytes_unvalidated_with_config`, whose
integrity caveats are documented on the trait. An `#[encode(unbounded)]`
*enum* must also give every variant an explicit `#[encode(weight = N)]`,
since automatic discriminant weighting uses payload cardinalities — exactly
what an unbounded payload doesn't have.

No unbounded leaf models ship with the crate yet (bounded budgets are
Minnow's point); the tier exists so they *can* — see
`tests/unbounded.rs` for a hand-written open-ended varint exercising it
end-to-end.

## Supported types and attribute forms

A field (struct field, or enum-variant field/payload) carries at most one
`#[encode(...)]` attribute naming its runtime configuration. No attribute
means `Config::default()`.

| Rust type | `#[encode(...)]` form | Weight | Notes |
|---|---|---|---|
| `bool` | *(none)* | `2` | uniform coin flip |
| `f64` | `float(min = a, max = b, precision = p)` | `round((b − a) · 10^p) + 1` | lossy quantisation to `p` decimal digits (`p` may be negative); `Default` spans ±1,000,000 at precision 0 |
| `u8`, `u16`, `u32`, `i8`, `i16`, `i32` | `int(min = a, max = b)` | `b − a + 1` | `Default` spans the type's full native range |
| `u64`, `i64` | `int(min = a, max = b)` | `b − a + 1` | no `Default`: the full native range (`2^64`) exceeds the coder's precision bound |
| `String` | `string(max_length = n)` | bounded byte-sequence weight | UTF-8 bytes, uniform over 256 per byte, `n` in bytes |
| `Vec<T>` | `seq(max_len = n)` or `seq(max_len = n, elem = <expr>)` | `Σ_{k=0}^{n} W(T)^k` | uniform length prefix (see "sequences" above); `elem` may be omitted when `T::Config: Default` |
| `Option<T>` | *(automatic)* | `1 + W(T)` | weighted discriminant — no annotation needed |
| enum (unit / tuple / struct variants) | per-field/payload attributes, plus optional `weight = N` per variant | `Σ W(variant)` | weighted discriminant by default; `weight` overrides it (see "manual weights" above) |
| struct / tuple struct | per-field attributes | `∏ W(field)` | product rule |
| `[T; N]` | the element's own attribute, applied once | `W(T)^N` | fixed-size array |
| `()` / unit struct / unit variant | *(none)* | `1` | zero bits on the wire |
| any type implementing `Encodeable` | `config = <expr>` | user-defined | the escape hatch every other form is sugar for — an arbitrary Rust expression evaluated as the runtime config |

## Encode-domain semantics: errors, not silent coercion

A value outside its model's configured range **fails to encode** with a typed
`EncodeError::OutOfRange` rather than being silently coerced. The distinction
Minnow draws is between two kinds of loss:

- **In-range quantisation** (450.03 → 450.0 at `precision = 1`) always
  happens and is not an error: its loss is bounded by half a quantisation
  step, which the schema explicitly declares.
- **Out-of-range coercion** (12 000 clamped to a declared maximum of 10 000)
  has *unbounded* loss and delivers plausible-but-wrong data to the receiver,
  so it is an error unless a field explicitly opts in.

Fields backed by naturally saturating sources (sensor channels) can opt into
clamping — `FloatModel::new(…)?.clamping()`, `IntModel::new(…)?.clamping()`,
or the `clamping` flag in the derive attributes:

```rust,ignore
#[encode(float(min = -10_000.0, max = 10_000.0, precision = 1, clamping))]
pub x: f64,
```

NaN and infinite floats are an `EncodeError::NonFinite` in every mode (no
nearest representable value exists), and a `Vec`/`String` longer than its
`max_len` is an `EncodeError::TooLong` (truncation would drop elements, which
is not a nearest-value projection — there is no clamping mode for sequences).

## Integrity

Minnow's arithmetic-coded output is **not self-validating**. An
arithmetic-coded stream is not self-delimiting the way, say, a
length-prefixed or tag-delimited format is: almost every byte string of a
plausible length decodes to *some* value of the schema, valid or not.

Two mitigations are built in, but neither is a full checksum:

* **Length window.** Before decoding, `decode_bytes` validates the input's
  length against the schema's `[best_case_bits, total_bytes]` window (see
  [`DecodeError::Length`]). Under uniform (automatic) weighting that window
  is at most one byte wide, so this reliably catches whole-byte truncation
  or a message from the wrong schema — but it cannot detect corruption that
  preserves the byte count. The window comes from the schema's `Bounded`
  impl, so only bounded schemas have it; `decode_bytes_unvalidated` (the
  entry point for unbounded schemas) skips it entirely.
* **Symbol validation.** A decoded discriminant or length outside its
  model's valid range is reported as [`DecodeError::InvalidSymbol`] rather
  than causing undefined behaviour or an out-of-bounds access — but a
  corrupt stream that happens to decode to *in-range* symbols throughout
  will decode to `Ok` with a silently wrong value.

Applications that need genuine end-to-end integrity — detecting bit flips,
partial corruption, or tampering — should wrap Minnow's output in their own
outer framing (e.g. a CRC or cryptographic checksum over `encode_bytes()`'s
output), the same way DCCL relies on its own transport framing rather than
building integrity checks into the codec itself.

[`SizeReport`]: https://docs.rs/minnow/latest/minnow/struct.SizeReport.html
[`DecodeError::Length`]: https://docs.rs/minnow/latest/minnow/enum.DecodeError.html#variant.Length
[`DecodeError::InvalidSymbol`]: https://docs.rs/minnow/latest/minnow/enum.DecodeError.html#variant.InvalidSymbol

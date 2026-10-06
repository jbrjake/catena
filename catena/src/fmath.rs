//! The only caller of transcendental functions in `catena` (plan §11).
//!
//! `std`'s `f64::sin`, `exp`, `atan2` and their kin resolve to the platform's libm, which differs
//! in the last ulp between glibc, macOS and MSVC, and one ulp decides a `round()` often enough to
//! break a golden committed on another OS. Every transcendental call in the crate goes through
//! this module, which wraps the pure-Rust `libm` crate (the same bits everywhere). `cargo xtask
//! lint` denies the `std` methods everywhere else in `catena/src`. `sqrt` is correctly rounded by
//! IEEE 754 and needs no wrapper.

/// Sine of `x` radians, with the same bits on every platform.
pub(crate) fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Cosine of `x` radians, with the same bits on every platform.
pub(crate) fn cos(x: f64) -> f64 {
    libm::cos(x)
}

#[cfg(test)]
mod tests {
    // `allow`, not `expect`: clippy 1.99's `float_cmp` no longer fires on these asserts and
    // earlier releases do, so an expectation is unfulfilled on one side or the other.
    #![allow(
        clippy::float_cmp,
        reason = "the claim is bit-exact values at the points where they are exactly known"
    )]

    use super::{cos, sin};

    #[test]
    fn exact_values_hold_bit_for_bit() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert_eq!(
            sin(-0.0).to_bits(),
            (-0.0f64).to_bits(),
            "sign of zero survives"
        );
        assert!(sin(f64::NAN).is_nan());
        assert!(cos(f64::INFINITY).is_nan());
    }

    #[test]
    fn values_match_the_textbook_within_an_ulp() {
        let half_pi = std::f64::consts::FRAC_PI_2;
        assert!((sin(half_pi) - 1.0).abs() <= f64::EPSILON);
        assert!(cos(half_pi).abs() <= f64::EPSILON);
        assert!((sin(std::f64::consts::FRAC_PI_6) - 0.5).abs() <= f64::EPSILON);
    }
}

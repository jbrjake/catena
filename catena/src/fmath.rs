//! The only caller of transcendental functions in `catena` (plan §11).
//!
//! `std`'s `f64::sin`, `exp`, `atan2` and their kin resolve to the platform's libm, which differs
//! in the last ulp between glibc, macOS and MSVC, and one ulp decides a `round()` often enough to
//! break a golden committed on another OS. Every transcendental call in the crate goes through
//! this module, which wraps the pure-Rust `libm` crate (the same bits everywhere). `cargo xtask
//! lint` denies the `std` methods everywhere else in `catena/src`. `sqrt` is correctly rounded by
//! IEEE 754 and needs no wrapper.

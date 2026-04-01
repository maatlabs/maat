//! Embedded standard library sources for the Maat programming language.
//!
//! Each constant holds the raw Maat source text for its standard library module.
//! The compiler embeds these strings at build time and injects them into the
//! module graph when a `use std::X` import is encountered, without touching
//! the file system at runtime.
#![forbid(unsafe_code)]

/// Source for `std::math`: comparison and numeric utility functions (`cmp::min`, `cmp::max`, `cmp::clamp`).
pub const STD_MATH: &str = include_str!("../std/math.maat");

/// Source for `std::string`: string manipulation functions.
pub const STD_STRING: &str = include_str!("../std/string.maat");

/// Source for `std::vec`: `Vector<T>` utility functions.
pub const STD_VEC: &str = include_str!("../std/vec.maat");

/// Source for `std::set`: `Set<T>` utility functions.
pub const STD_SET: &str = include_str!("../std/set.maat");

/// Source for `std::map`: `Map<K, V>` utility functions.
pub const STD_MAP: &str = include_str!("../std/map.maat");

/// Reference documentation for `cmp`: polymorphic comparison and numeric conversion builtins.
///
/// These are implemented natively in the compiler; this file is not injected as a parsed module.
pub const STD_CMP: &str = include_str!("../std/cmp.maat");

/// Reference documentation for numeric error types (`ParseIntError`).
///
/// These types are implemented natively in the compiler; this file is not injected as a parsed module.
pub const STD_NUM: &str = include_str!("../std/num.maat");

/// Reference documentation for `Option<T>`.
///
/// `Option<T>` is a compiler-builtin type; this file is not injected as a parsed module.
pub const STD_OPTION: &str = include_str!("../std/option.maat");

/// Reference documentation for `Result<T, E>`.
///
/// `Result<T, E>` is a compiler-builtin type; this file is not injected as a parsed module.
pub const STD_RESULT: &str = include_str!("../std/result.maat");

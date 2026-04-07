# maat_stdlib

Standard library sources for the Maat programming language.

## Role

`maat_stdlib` ships `.maat` source files that are embedded in the `maat` binary at compile time and injected into every program's module graph before user code is resolved. It provides the foundational modules available under the `std::` namespace, giving Maat programs access to common data structures and algorithms without external dependencies.

## Modules

| Module        | Contents                                     |
| ------------- | -------------------------------------------- |
| `std::math`   | Integer power, absolute value, min/max       |
| `std::vec`    | Vector construction and manipulation helpers |
| `std::map`    | Ordered map utilities                        |
| `std::set`    | Ordered set utilities                        |
| `std::string` | String formatting and conversion helpers     |
| `std::option` | `Option`-style pattern utilities             |
| `std::result` | `Result`-style pattern utilities             |
| `std::cmp`    | Comparison utilities                         |
| `std::num`    | Numeric trait helpers                        |

## Usage

Standard library modules are available automatically; no explicit import is needed for items re-exported from the prelude. For non-prelude items:

```rust
use std::math;

fn main() {
    let x = math::pow(2, 10); // 1024
    println!("{x}");
}
```

## API Docs

[docs.rs/maat_stdlib](https://docs.rs/maat_stdlib/latest/maat_stdlib/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.

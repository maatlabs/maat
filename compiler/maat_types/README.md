# maat_types

`Hindley-Milner` type inference engine for the Maat programming language.

## Role

`maat_types` implements `Algorithm W` with explicit annotations, numeric promotion rules, and compile-time overflow checking. It sits between macro expansion and code generation in the pipeline, validating user-defined types (structs, enums, traits, `impl` blocks) and producing a typed `TypeEnv` used downstream. Type errors are accumulated for multi-error reporting; no type variable escapes the checker as an unresolved inference variable.

## Usage

```rust
use maat_types::TypeChecker;
use maat_ast::Program;

let mut checker = TypeChecker::new();
checker.check_program(&program);

if checker.errors().is_empty() {
    // program is well-typed; proceed to code generation
} else {
    for err in checker.errors() {
        eprintln!("{err}");
    }
}
```

## API Docs

[docs.rs/maat_types](https://docs.rs/maat_types/latest/maat_types/)

## Repository

[github.com/maatlabs/maat](https://github.com/maatlabs/maat). See the [project README](https://github.com/maatlabs/maat/blob/main/README.md) for an overview of the full compiler pipeline.

//! Multi-module type checking, compilation, and linking pipeline.
//!
//! Orchestrates per-module type checking in topological order (each module
//! gets its own [`TypeEnv`] with imported bindings), then compiles all
//! modules using a single shared [`Compiler`]. The shared compiler ensures
//! that globals, constants, and type registry entries occupy a unified
//! index space, making linking implicit in the compilation step.

use std::collections::HashMap;

use maat_ast::{Program, Stmt, TypeExpr};
use maat_bytecode::Bytecode;
use maat_codegen::Compiler;
use maat_errors::ModuleErrorKind;
use maat_runtime::{TypeDef, VariantInfo};
use maat_span::Span;
use maat_types::{
    EnumDef, ImplDef, StructDef, TraitDef, Type, TypeChecker, TypeEnv, TypeScheme, VariantKind,
};

use crate::{ModuleExports, ModuleGraph, ModuleId, ModuleResult};

/// Type-checks, compiles, and links all modules in the given graph.
///
/// The pipeline operates in two phases:
///
/// 1. **Type checking** — Each module is type-checked independently with
///    its own [`TypeEnv`], after injecting public exports from dependencies.
///    This enforces module-level visibility while allowing cross-module
///    type resolution.
///
/// 2. **Compilation** — All modules are compiled in topological order
///    (leaves first, root last) using a single shared [`Compiler`]. This
///    ensures that:
///    - `define_symbol` reuses existing global indices for imported names
///      rather than allocating duplicates
///    - Constants and type definitions share a single pool
///    - The resulting instruction stream naturally executes dependency
///      initialization code before the root module
///
/// The output is a single [`Bytecode`] ready for VM execution or
/// serialization to `.mtc`.
///
/// # Errors
///
/// Returns a [`ModuleError`](maat_errors::ModuleError) if type checking or
/// compilation fails for any module.
pub fn check_and_compile(graph: &mut ModuleGraph) -> ModuleResult<Bytecode> {
    let mut exports: HashMap<ModuleId, ModuleExports> = HashMap::new();
    let topo_order = graph.topo_order().to_vec();

    // Phase 1: Type-check each module independently and extract exports.
    // Resolved imports are cached to avoid redundant work in phase 2.
    let mut cached_imports: HashMap<ModuleId, Vec<ResolvedImport>> = HashMap::new();

    for &module_id in &topo_order {
        let node = graph.node(module_id);
        let file_path = node.path.clone();

        let imports = resolve_imports(&node.program, &exports, graph)?;

        let program = &mut graph.node_mut(module_id).program;
        let mut checker = TypeChecker::new();
        for import in &imports {
            inject_import(checker.env_mut(), import);
        }
        checker.check_program_mut(program);

        let type_errors = checker.errors();
        if !type_errors.is_empty() {
            let messages = type_errors.iter().map(|e| e.kind.to_string()).collect();
            return Err(ModuleErrorKind::TypeErrors {
                file: file_path.clone(),
                messages,
            }
            .at(Span::ZERO, file_path));
        }

        let module_exports = extract_exports(program, checker.env());
        exports.insert(module_id, module_exports);

        let fold_errors = maat_ast::fold::fold_constants(program);
        if !fold_errors.is_empty() {
            let messages = fold_errors.iter().map(|e| e.kind.to_string()).collect();
            return Err(ModuleErrorKind::TypeErrors {
                file: file_path.clone(),
                messages,
            }
            .at(Span::ZERO, file_path));
        }

        cached_imports.insert(module_id, imports);
    }

    // Phase 2: Compile all modules with a shared compiler. Because the
    // compiler's symbol table and constant pool persist across modules,
    // `define_symbol` reuses existing global indices for imported symbols,
    // i.e., no post-hoc linking or index remapping is required.
    //
    // After each non-root module is compiled, newly-defined globals that
    // are not part of the module's public exports are hidden from the
    // symbol table, preventing private symbols from leaking into
    // subsequent modules.
    let mut compiler = Compiler::new();

    for &module_id in &topo_order {
        let file_path = graph.node(module_id).path.clone();

        if let Some(imports) = cached_imports.get(&module_id) {
            for import in imports {
                inject_import_into_compiler(&mut compiler, import);
            }
        }

        let before = compiler.symbols_table_mut().global_symbol_names();

        let program = &graph.node(module_id).program;
        compiler.compile_program(program).map_err(|e| {
            ModuleErrorKind::CompileErrors {
                file: file_path.clone(),
                messages: vec![e.to_string()],
            }
            .at(Span::ZERO, file_path.clone())
        })?;

        // Mask all newly-defined globals after compiling a non-root
        // module. Masking hides symbols from resolution without removing
        // their storage indices, so that `inject_import_into_compiler`
        // in subsequent iterations can unmask and reuse the same global
        // slot via `define_symbol`. This prevents both private and public
        // symbols from leaking into modules that have not explicitly
        // imported them.
        if module_id != ModuleId::ROOT {
            let after = compiler.symbols_table_mut().global_symbol_names();
            for name in after {
                if !before.contains(&name) {
                    compiler.symbols_table_mut().mask_symbol(&name);
                }
            }
        }
    }

    let root_path = graph.root().path.clone();
    compiler.bytecode().map_err(|e| {
        ModuleErrorKind::CompileErrors {
            file: root_path.clone(),
            messages: vec![e.to_string()],
        }
        .at(Span::ZERO, root_path)
    })
}

/// Returns the per-module public exports extracted during type checking.
///
/// This is a convenience for tests and tooling that need to inspect
/// which items are publicly visible from each module without running
/// the full compilation phase.
pub fn check_exports(graph: &mut ModuleGraph) -> ModuleResult<HashMap<ModuleId, ModuleExports>> {
    let mut exports: HashMap<ModuleId, ModuleExports> = HashMap::new();
    let topo_order = graph.topo_order().to_vec();

    for &module_id in &topo_order {
        let node = graph.node(module_id);
        let file_path = node.path.clone();

        let imports = resolve_imports(&node.program, &exports, graph)?;

        let program = &mut graph.node_mut(module_id).program;
        let mut checker = TypeChecker::new();
        for import in &imports {
            inject_import(checker.env_mut(), import);
        }
        checker.check_program_mut(program);

        let type_errors = checker.errors();
        if !type_errors.is_empty() {
            let messages = type_errors.iter().map(|e| e.kind.to_string()).collect();
            return Err(ModuleErrorKind::TypeErrors {
                file: file_path.clone(),
                messages,
            }
            .at(Span::ZERO, file_path));
        }

        let module_exports = extract_exports(program, checker.env());
        exports.insert(module_id, module_exports);
    }

    Ok(exports)
}

/// A resolved import ready to be injected into a module's type environment.
#[derive(Debug)]
struct ResolvedImport {
    /// The local name under which this item is available.
    local_name: String,
    /// The exported item.
    kind: ImportKind,
}

/// The kind of item being imported.
#[derive(Debug)]
enum ImportKind {
    Binding(TypeScheme),
    Struct(StructDef),
    Enum(EnumDef),
    Trait(TraitDef),
    Impl(ImplDef),
}

/// Resolves all `use` statements in a module's program against the available exports.
fn resolve_imports(
    program: &Program,
    exports: &HashMap<ModuleId, ModuleExports>,
    graph: &ModuleGraph,
) -> ModuleResult<Vec<ResolvedImport>> {
    let mut result = Vec::new();

    for stmt in &program.statements {
        let Stmt::Use(use_stmt) = stmt else {
            continue;
        };

        // Determine the module path and items to import.
        //
        // For group imports (`use foo::{bar, baz};`), the full path
        // identifies the module and `items` lists the imported names.
        //
        // For non-group imports (`use foo::bar;` or `use std::math::abs;`),
        // everything except the last segment identifies the module, and
        // the last segment is the imported item.
        let (module_path, items_to_import) = if let Some(items) = &use_stmt.items {
            (use_stmt.path.as_slice(), items.clone())
        } else if use_stmt.path.len() >= 2 {
            let split = use_stmt.path.len() - 1;
            (&use_stmt.path[..split], vec![use_stmt.path[split].clone()])
        } else {
            // `use foo;` (bare module import) is intentionally a no-op.
            // Maat requires explicit item imports (`use foo::bar;` or
            // `use foo::{bar, baz};`) for ZK auditability. The bare
            // form is silently skipped; any attempt to use unimported
            // items will fail with an undefined variable error.
            continue;
        };

        let target_id = find_module_by_path(graph, module_path);

        let Some(target_id) = target_id else {
            // Module not in the graph; use of its items will fail
            // with an undefined variable error during compilation.
            continue;
        };
        let Some(target_exports) = exports.get(&target_id) else {
            continue;
        };

        for item_name in &items_to_import {
            find_exports(target_exports, item_name, &mut result);
        }
    }

    Ok(result)
}

/// Finds a module in the graph by matching a use-path against qualified paths.
///
/// For a single-segment path like `["math"]`, matches modules whose
/// qualified path ends with `"math"`. For multi-segment paths like
/// `["std", "math"]`, requires an exact match against the full
/// qualified path.
fn find_module_by_path(graph: &ModuleGraph, module_path: &[String]) -> Option<ModuleId> {
    graph
        .nodes()
        .find(|n| {
            if module_path.len() == 1 {
                n.qualified_path
                    .last()
                    .is_some_and(|last| last == &module_path[0])
            } else {
                n.qualified_path == module_path
            }
        })
        .map(|n| n.id)
}

/// Finds all exports matching `name` and appends them to `result`.
///
/// When a struct or enum is found, any associated `impl` blocks from the
/// same module are also included so that method resolution works across
/// module boundaries.
fn find_exports(exports: &ModuleExports, name: &str, result: &mut Vec<ResolvedImport>) {
    if let Some((_, scheme)) = exports.bindings.iter().find(|(n, _)| n == name) {
        result.push(ResolvedImport {
            local_name: name.to_string(),
            kind: ImportKind::Binding(scheme.clone()),
        });
        return;
    }

    // For struct/enum imports, also pull in associated impl blocks.
    let mut is_type_import = false;

    if let Some(def) = exports.structs.iter().find(|d| d.name == name) {
        result.push(ResolvedImport {
            local_name: name.to_string(),
            kind: ImportKind::Struct(def.clone()),
        });
        is_type_import = true;
    }
    if let Some(def) = exports.enums.iter().find(|d| d.name == name) {
        result.push(ResolvedImport {
            local_name: name.to_string(),
            kind: ImportKind::Enum(def.clone()),
        });
        is_type_import = true;
    }

    if is_type_import {
        for imp in &exports.impls {
            let matches = match &imp.self_type {
                Type::Struct(n, _) | Type::Enum(n, _) => n == name,
                _ => false,
            };
            if matches {
                result.push(ResolvedImport {
                    local_name: String::new(),
                    kind: ImportKind::Impl(imp.clone()),
                });
            }
        }
        return;
    }

    if let Some(def) = exports.traits.iter().find(|d| d.name == name) {
        result.push(ResolvedImport {
            local_name: name.to_string(),
            kind: ImportKind::Trait(def.clone()),
        });
    }
}

/// Injects a resolved import into the compiler's symbol table and type registry.
///
/// For bindings, `define_symbol` reuses the existing global index if the symbol
/// was already defined by a prior module's compilation (this is the mechanism
/// by which the shared compiler avoids duplicate global slots for cross-module
/// references).
fn inject_import_into_compiler(compiler: &mut Compiler, import: &ResolvedImport) {
    match &import.kind {
        ImportKind::Binding(_) => {
            let _ = compiler
                .symbols_table_mut()
                .define_symbol(&import.local_name, false);
        }
        ImportKind::Struct(def) => {
            compiler.type_registry_mut().push(TypeDef::Struct {
                name: def.name.clone(),
                field_names: def.fields.iter().map(|(n, _)| n.clone()).collect(),
            });
        }
        ImportKind::Enum(def) => {
            compiler.type_registry_mut().push(TypeDef::Enum {
                name: def.name.clone(),
                variants: def
                    .variants
                    .iter()
                    .map(|v| VariantInfo {
                        name: v.name.clone(),
                        field_count: match &v.kind {
                            VariantKind::Unit => 0,
                            VariantKind::Tuple(fields) => fields.len() as u8,
                            VariantKind::Struct(fields) => fields.len() as u8,
                        },
                    })
                    .collect(),
            });
        }
        ImportKind::Trait(_) => {
            // Traits have no runtime representation; they only affect
            // type checking which is handled by inject_import.
        }
        ImportKind::Impl(def) => {
            // Register each method as a global symbol so that method
            // calls compile correctly.
            let type_name = match &def.self_type {
                Type::Struct(n, _) | Type::Enum(n, _) => n.clone(),
                _ => return,
            };
            for (method_name, _) in &def.methods {
                let qualified = format!("{type_name}::{method_name}");
                let _ = compiler
                    .symbols_table_mut()
                    .define_symbol(&qualified, false);
            }
        }
    }
}

/// Injects a resolved import into a module's type environment.
fn inject_import(env: &mut TypeEnv, import: &ResolvedImport) {
    match &import.kind {
        ImportKind::Binding(scheme) => {
            env.define_scheme(&import.local_name, scheme.clone());
        }
        ImportKind::Struct(def) => {
            env.register_struct(def.clone());
        }
        ImportKind::Enum(def) => {
            env.register_enum(def.clone());
        }
        ImportKind::Trait(def) => {
            env.register_trait(def.clone());
        }
        ImportKind::Impl(def) => {
            env.register_impl(def.clone());
        }
    }
}

/// Extracts public exports from a type-checked module.
///
/// Scans the AST for `pub` items and collects their type information
/// from the type environment.
fn extract_exports(program: &Program, env: &TypeEnv) -> ModuleExports {
    let mut exports = ModuleExports::default();

    for stmt in &program.statements {
        match stmt {
            Stmt::FuncDef(func) if func.is_public => {
                if let Some(scheme) = env.lookup_scheme(&func.name) {
                    exports.bindings.push((func.name.clone(), scheme.clone()));
                }
            }
            Stmt::StructDecl(decl) if decl.is_public => {
                if let Some(def) = env.lookup_struct(&decl.name) {
                    exports.structs.push(def.clone());
                }
            }
            Stmt::EnumDecl(decl) if decl.is_public => {
                if let Some(def) = env.lookup_enum(&decl.name) {
                    exports.enums.push(def.clone());
                }
            }
            Stmt::TraitDecl(decl) if decl.is_public => {
                if let Some(def) = env.lookup_trait(&decl.name) {
                    exports.traits.push(def.clone());
                }
            }
            Stmt::ImplBlock(impl_block) => {
                let type_name = match &impl_block.self_type {
                    TypeExpr::Named(n) => &n.name,
                    TypeExpr::Generic(name, _, _) => name,
                    _ => continue,
                };

                let pub_methods = impl_block
                    .methods
                    .iter()
                    .filter(|m| m.is_public)
                    .map(|m| m.name.as_str())
                    .collect::<Vec<&str>>();

                if pub_methods.is_empty() {
                    continue;
                }

                // Find the matching ImplDef and export only public methods.
                for imp in env.all_impls() {
                    let matches = match &imp.self_type {
                        Type::Struct(n, _) | Type::Enum(n, _) => n == type_name,
                        _ => false,
                    };
                    if matches {
                        let filtered = ImplDef {
                            self_type: imp.self_type.clone(),
                            trait_name: imp.trait_name.clone(),
                            methods: imp
                                .methods
                                .iter()
                                .filter(|(name, _)| pub_methods.contains(&name.as_str()))
                                .cloned()
                                .collect(),
                        };
                        if !filtered.methods.is_empty() {
                            exports.impls.push(filtered);
                        }
                    }
                }
            }
            Stmt::Use(use_stmt) if use_stmt.is_public => {
                // Re-exports: `pub use foo::bar;` — forward the binding.
                let item_name = use_stmt
                    .items
                    .as_ref()
                    .and_then(|items| items.first())
                    .or_else(|| use_stmt.path.last());
                if let Some(name) = item_name {
                    if let Some(scheme) = env.lookup_scheme(name) {
                        exports.bindings.push((name.clone(), scheme.clone()));
                    }
                    if let Some(def) = env.lookup_struct(name) {
                        exports.structs.push(def.clone());
                    }
                    if let Some(def) = env.lookup_enum(name) {
                        exports.enums.push(def.clone());
                    }
                    if let Some(def) = env.lookup_trait(name) {
                        exports.traits.push(def.clone());
                    }
                }
            }
            _ => {}
        }
    }

    exports
}

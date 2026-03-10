//! Multi-module type checking and compilation pipeline.
//!
//! Orchestrates per-module type checking and compilation in topological
//! order, threading public exports from dependency modules into each
//! downstream module's type environment.

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

/// The result of type-checking and compiling all modules in a module graph.
#[derive(Debug)]
pub struct CompiledModules {
    /// Per-module compiled bytecodes, indexed by `ModuleId`.
    pub bytecodes: HashMap<ModuleId, Bytecode>,
    /// Per-module public exports, indexed by `ModuleId`.
    pub exports: HashMap<ModuleId, ModuleExports>,
}

/// Type-checks and compiles all modules in the given graph in topological order.
///
/// For each module (leaves first, root last):
/// 1. Resolves `use` statements by importing public exports from dependency modules
/// 2. Type-checks the module, enforcing visibility
/// 3. Extracts public exports for downstream consumers
/// 4. Compiles the module to bytecode
///
/// # Errors
///
/// Returns a [`ModuleError`](maat_errors::ModuleError) if type checking or
/// compilation fails for any module.
pub fn check_and_compile(graph: &mut ModuleGraph) -> ModuleResult<CompiledModules> {
    let mut exports: HashMap<ModuleId, ModuleExports> = HashMap::new();
    let mut bytecodes: HashMap<ModuleId, Bytecode> = HashMap::new();

    let topo_order = graph.topo_order().to_vec();

    for &module_id in &topo_order {
        let node = graph.node(module_id);
        let qualified_path = node.qualified_path.clone();
        let file_path = node.path.clone();

        let imports = resolve_imports(&node.program, &qualified_path, &exports, graph)?;

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

        let mut compiler = Compiler::new();
        for import in &imports {
            inject_import_into_compiler(&mut compiler, import);
        }
        compiler.compile_program(program).map_err(|e| {
            ModuleErrorKind::CompileErrors {
                file: file_path.clone(),
                messages: vec![e.to_string()],
            }
            .at(Span::ZERO, file_path.clone())
        })?;

        let bytecode = compiler.bytecode().map_err(|e| {
            ModuleErrorKind::CompileErrors {
                file: file_path.clone(),
                messages: vec![e.to_string()],
            }
            .at(Span::ZERO, file_path)
        })?;

        bytecodes.insert(module_id, bytecode);
    }

    Ok(CompiledModules { bytecodes, exports })
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
    _qualified_path: &[String],
    exports: &HashMap<ModuleId, ModuleExports>,
    graph: &ModuleGraph,
) -> ModuleResult<Vec<ResolvedImport>> {
    let mut result = Vec::new();

    for stmt in &program.statements {
        let Stmt::Use(use_stmt) = stmt else {
            continue;
        };

        let target_module_name = &use_stmt.path[0];
        let target_id = graph
            .nodes()
            .find(|n| {
                n.qualified_path
                    .last()
                    .is_some_and(|last| last == target_module_name)
            })
            .map(|n| n.id);

        let Some(target_id) = target_id else {
            // Module not found in the graph; this will be caught as a
            // type error downstream. Skip for now.
            continue;
        };
        let Some(target_exports) = exports.get(&target_id) else {
            continue;
        };

        let items_to_import = if let Some(items) = &use_stmt.items {
            // `use foo::{bar, baz};`: import specific items.
            items.clone()
        } else if use_stmt.path.len() >= 2 {
            // `use foo::bar;`: import `bar` from module `foo`.
            vec![use_stmt.path.last().unwrap().clone()]
        } else {
            // `use foo;` without qualified path resolution is not yet
            // supported because qualified
            // module-path resolution is not yet implemented; reject
            // this form with a clear error.
            continue;
        };

        for item_name in &items_to_import {
            find_exports(target_exports, item_name, &mut result);
        }
    }

    Ok(result)
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
/// Defines imported bindings as global symbols so that cross-module references
/// compile without "undefined variable" errors. The actual values will be
/// linked in at the linking phase.
fn inject_import_into_compiler(compiler: &mut Compiler, import: &ResolvedImport) {
    match &import.kind {
        ImportKind::Binding(_) => {
            let _ = compiler
                .symbols_table_mut()
                .define_symbol(&import.local_name);
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
                let _ = compiler.symbols_table_mut().define_symbol(&qualified);
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::resolve_module_graph;

    /// Creates a temporary directory tree from a list of `(relative_path, content)` pairs.
    fn setup_temp_project(pairs: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        for (path, content) in pairs {
            let full = dir.path().join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).expect("failed to create directory");
            }
            fs::write(&full, content).expect("failed to write file");
        }
        dir
    }

    /// Resolves and compiles a multi-file project, returning the compiled modules.
    fn compile_project(pairs: &[(&str, &str)]) -> ModuleResult<CompiledModules> {
        let dir = setup_temp_project(pairs);
        let mut graph = resolve_module_graph(&dir.path().join("main.mt"))?;
        check_and_compile(&mut graph)
    }

    #[test]
    fn single_module_compiles() {
        let result = compile_project(&[("main.mt", "let x: i64 = 42;")]);
        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert_eq!(compiled.bytecodes.len(), 1);
    }

    #[test]
    fn import_pub_function() {
        let result = compile_project(&[
            (
                "main.mt",
                "mod math;\nuse math::add;\nlet result: i64 = add(1, 2);",
            ),
            ("math.mt", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
        ]);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        assert_eq!(result.unwrap().bytecodes.len(), 2);
    }

    #[test]
    fn try_import_private_function() {
        let result = compile_project(&[
            (
                "main.mt",
                "mod math;\nuse math::secret;\nlet x: i64 = secret();",
            ),
            ("math.mt", "fn secret() -> i64 { 42 }"),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn import_grouped_pub_items() {
        let result = compile_project(&[
            (
                "main.mt",
                "mod math;\nuse math::{add, sub};\nlet x: i64 = add(1, 2);\nlet y: i64 = sub(5, 3);",
            ),
            (
                "math.mt",
                "pub fn add(a: i64, b: i64) -> i64 { a + b }\npub fn sub(a: i64, b: i64) -> i64 { a - b }",
            ),
        ]);
        assert!(result.is_ok());
    }

    #[test]
    fn bare_use_module_is_noop() {
        // `use math;` without qualified path support is silently ignored.
        // Items remain inaccessible without explicit import paths.
        let result = compile_project(&[
            ("main.mt", "mod math;\nuse math;\nlet x: i64 = add(1, 2);"),
            ("math.mt", "pub fn add(a: i64, b: i64) -> i64 { a + b }"),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn import_specific_items_from_group() {
        let result = compile_project(&[
            (
                "main.mt",
                "mod math;\nuse math::{add, sub};\nlet x: i64 = add(1, 2);\nlet y: i64 = sub(5, 3);",
            ),
            (
                "math.mt",
                "pub fn add(a: i64, b: i64) -> i64 { a + b }\npub fn sub(a: i64, b: i64) -> i64 { a - b }\nfn internal() -> i64 { 0 }",
            ),
        ]);
        assert!(result.is_ok());
    }

    #[test]
    fn import_pub_custom_types() {
        let result = compile_project(&[
            (
                "main.mt",
                "mod types;\nuse types::Point;\nlet p = Point { x: 1, y: 2 };\nlet px: i64 = p.x;",
            ),
            (
                "types.mt",
                "pub struct Point {\n    pub x: i64,\n    pub y: i64,\n}",
            ),
        ]);
        assert!(result.is_ok());

        let result = compile_project(&[
            (
                "main.mt",
                "mod types;\nuse types::Color;\nlet c = Color::Red;",
            ),
            (
                "types.mt",
                "pub enum Color {\n    Red,\n    Green,\n    Blue,\n}",
            ),
        ]);
        assert!(result.is_ok());
    }

    #[test]
    fn type_error_in_dependency_module() {
        let result = compile_project(&[
            ("main.mt", "mod bad;"),
            ("bad.mt", "pub fn broken() -> i64 { true }"),
        ]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("type error"),
            "expected type error, got: {}",
            err
        );
    }

    #[test]
    fn diamond_dependency_compiles() {
        let result = compile_project(&[
            (
                "main.mt",
                "mod a;\nmod b;\nuse a::from_a;\nuse b::from_b;\nlet x: i64 = from_a();\nlet y: i64 = from_b();",
            ),
            ("a.mt", "pub fn from_a() -> i64 { 1 }"),
            ("b.mt", "pub fn from_b() -> i64 { 2 }"),
        ]);
        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert_eq!(compiled.bytecodes.len(), 3);
    }

    #[test]
    fn reexport_pub_use() {
        let result = compile_project(&[
            (
                "main.mt",
                "mod facade;\nuse facade::helper;\nlet x: i64 = helper();",
            ),
            ("facade.mt", "mod utils;\npub use utils::helper;"),
            ("facade/utils.mt", "pub fn helper() -> i64 { 42 }"),
        ]);
        assert!(result.is_ok());
    }

    #[test]
    fn exports_only_pub_items() {
        let dir = setup_temp_project(&[
            ("main.mt", "mod lib;"),
            (
                "lib.mt",
                "pub fn visible() -> i64 { 1 }\nfn hidden() -> i64 { 2 }",
            ),
        ]);
        let mut graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
        let compiled = check_and_compile(&mut graph).unwrap();

        // Find the lib module's exports (not root).
        let lib_exports = compiled
            .exports
            .iter()
            .find(|(id, _)| **id != ModuleId::ROOT)
            .map(|(_, e)| e)
            .unwrap();

        assert_eq!(lib_exports.bindings.len(), 1);
        assert_eq!(lib_exports.bindings[0].0, "visible");
    }

    #[test]
    fn impl_blocks_export_only_pub_methods() {
        let dir = setup_temp_project(&[
            ("main.mt", "mod shapes;"),
            (
                "shapes.mt",
                "pub struct Circle {\n    pub radius: i64,\n}\n\nimpl Circle {\n    pub fn area(self) -> i64 { self.radius }\n    fn secret(self) -> i64 { self.radius }\n}",
            ),
        ]);
        let mut graph = resolve_module_graph(&dir.path().join("main.mt")).unwrap();
        let compiled = check_and_compile(&mut graph).unwrap();

        let shapes_exports = compiled
            .exports
            .iter()
            .find(|(id, _)| **id != ModuleId::ROOT)
            .map(|(_, e)| e)
            .unwrap();

        assert_eq!(shapes_exports.impls.len(), 1);
        let imp = &shapes_exports.impls[0];
        assert_eq!(imp.methods.len(), 1, "only pub methods should be exported");
        assert_eq!(imp.methods[0].0, "area");
    }

    #[test]
    fn nested_module_import() {
        let result = compile_project(&[
            (
                "main.mt",
                "mod outer;\nuse outer::greet;\nlet msg: i64 = greet();",
            ),
            (
                "outer.mt",
                "mod inner;\nuse inner::value;\npub fn greet() -> i64 { value() }",
            ),
            ("outer/inner.mt", "pub fn value() -> i64 { 99 }"),
        ]);
        assert!(result.is_ok());
    }
}

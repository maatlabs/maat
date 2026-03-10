//! Module dependency graph (DAG) with topological ordering.

use std::path::PathBuf;

use maat_ast::Program;

/// A unique identifier for a module within the dependency graph.
///
/// The root module (entry point) always has `ModuleId(0)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId(pub u32);

impl ModuleId {
    /// The root module identifier.
    pub const ROOT: Self = Self(0);
}

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModuleId({})", self.0)
    }
}

/// A single node in the module dependency graph.
///
/// Stores the parsed AST, file path, and fully qualified module path
/// for a single source file.
#[derive(Debug)]
pub struct ModuleNode {
    /// The unique identifier for this module.
    pub id: ModuleId,
    /// The parsed AST of this module.
    pub program: Program,
    /// The absolute path to the source file.
    pub path: PathBuf,
    /// The fully qualified module path segments (e.g., `["math", "ops"]`).
    pub qualified_path: Vec<String>,
}

/// A directed acyclic graph of module dependencies.
///
/// Nodes are [`ModuleNode`]s indexed by [`ModuleId`]. Edges represent
/// `mod` declarations (parent depends on child). The graph stores a
/// precomputed topological order (leaves first) suitable for compilation.
#[derive(Debug)]
pub struct ModuleGraph {
    /// All modules in the graph, indexed by `ModuleId.0`.
    nodes: Vec<ModuleNode>,
    /// Adjacency list: `edges[i]` contains the `ModuleId`s that module `i`
    /// depends on (its declared submodules).
    edges: Vec<Vec<ModuleId>>,
    /// Modules in topological order (leaves first, root last).
    topo_order: Vec<ModuleId>,
}

impl ModuleGraph {
    /// Creates an empty module graph.
    pub(crate) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            topo_order: Vec::new(),
        }
    }

    /// Adds a module node to the graph and returns its assigned [`ModuleId`].
    pub(crate) fn add_node(
        &mut self,
        program: Program,
        path: PathBuf,
        qualified_path: Vec<String>,
    ) -> ModuleId {
        let id = ModuleId(self.nodes.len() as u32);
        self.nodes.push(ModuleNode {
            id,
            program,
            path,
            qualified_path,
        });
        self.edges.push(Vec::new());
        id
    }

    /// Records a dependency edge from `parent` to `child`.
    ///
    /// Both identifiers must have been returned by a prior `add_node` call.
    pub(crate) fn add_edge(&mut self, parent: ModuleId, child: ModuleId) {
        debug_assert!(
            (parent.0 as usize) < self.edges.len(),
            "parent {parent} out of bounds"
        );
        self.edges[parent.0 as usize].push(child);
    }

    /// Sets the precomputed topological order.
    pub(crate) fn set_topo_order(&mut self, order: Vec<ModuleId>) {
        self.topo_order = order;
    }

    /// Returns the module node for the given identifier.
    ///
    /// # Panics
    ///
    /// Panics if `id` was not returned by `add_node`
    pub fn node(&self, id: ModuleId) -> &ModuleNode {
        &self.nodes[id.0 as usize]
    }

    /// Returns a mutable reference to the module node for the given identifier.
    ///
    /// # Panics
    ///
    /// Panics if `id` was not returned by `add_node`.
    pub fn node_mut(&mut self, id: ModuleId) -> &mut ModuleNode {
        &mut self.nodes[id.0 as usize]
    }

    /// Returns the root module node (the entry point file).
    ///
    /// # Panics
    ///
    /// Panics if the graph is empty.
    pub fn root(&self) -> &ModuleNode {
        self.node(ModuleId::ROOT)
    }

    /// Returns an iterator over all module nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &ModuleNode> {
        self.nodes.iter()
    }

    /// Returns the total number of modules in the graph.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` if the graph contains no modules.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns the direct submodule dependencies of a module.
    ///
    /// # Panics
    ///
    /// Panics if `id` was not returned by `add_node`.
    pub fn dependencies(&self, id: ModuleId) -> &[ModuleId] {
        &self.edges[id.0 as usize]
    }

    /// Returns module identifiers in topological order (leaves first, root last).
    ///
    /// This is the order in which modules should be compiled: each module
    /// is compiled only after all its dependencies have been compiled.
    pub fn topo_order(&self) -> &[ModuleId] {
        &self.topo_order
    }
}

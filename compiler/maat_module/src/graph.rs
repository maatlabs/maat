use std::path::PathBuf;

use maat_ast::Program;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId(pub u32);

impl ModuleId {
    pub const ROOT: Self = Self(0);
}

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModuleId({})", self.0)
    }
}

#[derive(Debug)]
pub struct ModuleNode {
    pub id: ModuleId,
    pub program: Program,
    pub path: PathBuf,
    pub qualified_path: Vec<String>,
}

#[derive(Debug)]
pub struct ModuleGraph {
    nodes: Vec<ModuleNode>,
    edges: Vec<Vec<ModuleId>>,
    topo_order: Vec<ModuleId>,
}

impl ModuleGraph {
    pub(crate) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            topo_order: Vec::new(),
        }
    }

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

    pub(crate) fn add_edge(&mut self, parent: ModuleId, child: ModuleId) {
        debug_assert!(
            (parent.0 as usize) < self.edges.len(),
            "parent {parent} out of bounds"
        );
        self.edges[parent.0 as usize].push(child);
    }

    pub(crate) fn set_topo_order(&mut self, order: Vec<ModuleId>) {
        self.topo_order = order;
    }

    pub fn node(&self, id: ModuleId) -> &ModuleNode {
        &self.nodes[id.0 as usize]
    }

    pub fn node_mut(&mut self, id: ModuleId) -> &mut ModuleNode {
        &mut self.nodes[id.0 as usize]
    }

    pub fn root(&self) -> &ModuleNode {
        self.node(ModuleId::ROOT)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &ModuleNode> {
        self.nodes.iter()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` if the graph contains no modules.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn dependencies(&self, id: ModuleId) -> &[ModuleId] {
        &self.edges[id.0 as usize]
    }

    pub fn topo_order(&self) -> &[ModuleId] {
        &self.topo_order
    }
}

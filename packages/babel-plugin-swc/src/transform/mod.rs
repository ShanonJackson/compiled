use swc_core::ecma::ast::Program;
use swc_core::ecma::visit::FoldWith;

use crate::{
    options::PluginConfig,
    state::TransformState,
    types::{TransformMetadata, TransformResult},
};

mod visitor;

use visitor::RootFolder;

pub struct Transform {
    state: TransformState,
}

impl Transform {
    pub fn new(config: PluginConfig, metadata: TransformMetadata) -> Self {
        Self {
            state: TransformState::new(config, metadata),
        }
    }

    pub fn apply(&mut self, program: Program) -> TransformResult {
        // The heavy lifting lives inside the visitor which currently performs a
        // mostly identity transformation. This structure mirrors the layout of
        // the Babel implementation and allows us to progressively port features
        // while keeping the external API stable.
        let transformed = match program {
            Program::Module(module) => {
                let mut folder = RootFolder::new(&mut self.state);
                Program::Module(module.fold_with(&mut folder))
            }
            Program::Script(script) => {
                let mut folder = RootFolder::new(&mut self.state);
                Program::Script(script.fold_with(&mut folder))
            }
        };

        self.state.finalize(transformed)
    }
}

use std::cell::RefCell;
use std::rc::Rc;

use swc_core::ecma::ast::Program;
use swc_core::ecma::visit::{noop_visit_mut_type, VisitMut};

use crate::types::{
    PluginOptions, SharedTransformState, TransformFile, TransformMetadata, TransformState,
};

/// Primary SWC transform that will eventually mirror `@compiled/babel-plugin`.
pub struct CompiledBabelTransform {
    state: SharedTransformState,
    metadata: TransformMetadata,
}

impl CompiledBabelTransform {
    pub fn new(options: PluginOptions) -> Self {
        let state = Rc::new(RefCell::new(TransformState::new(
            TransformFile::default(),
            options,
        )));

        Self {
            state,
            metadata: TransformMetadata::default(),
        }
    }

    pub fn into_metadata(self) -> TransformMetadata {
        let mut metadata = self.metadata;
        let state = self.state.borrow();

        if metadata.included_files.is_empty() {
            metadata.included_files = state.included_files.clone();
        }

        metadata
    }

    pub fn state(&self) -> SharedTransformState {
        Rc::clone(&self.state)
    }

    pub fn metadata_mut(&mut self) -> &mut TransformMetadata {
        &mut self.metadata
    }
}

impl VisitMut for CompiledBabelTransform {
    noop_visit_mut_type!();

    fn visit_mut_program(&mut self, _: &mut Program) {
        // The real implementation will mirror the Babel visitors. For now, we simply retain the
        // original program without mutations so the scaffold can be used in tests while we port
        // features incrementally.
    }
}

use std::path::{Path, PathBuf};

use oxc_resolver::{ResolveOptions, Resolver};

use crate::{constants::DEFAULT_CODE_EXTENSIONS, options::ResolverConfig};

pub fn create_resolver(config: Option<&ResolverConfig>, root_dir: Option<&Path>) -> Resolver {
    let mut options = ResolveOptions::default();

    if let Some(root) = root_dir {
        options.cwd = Some(root.to_path_buf());
    }

    if let Some(config) = config {
        if let Some(cwd) = &config.working_directory {
            options.cwd = Some(PathBuf::from(cwd));
        }

        if !config.conditions.is_empty() {
            options.condition_names = config.conditions.clone();
        }

        if !config.extensions.is_empty() {
            options.extensions = config.extensions.clone();
        }

        if !config.alias_fields.is_empty() {
            options.alias_fields = config.alias_fields.clone();
        }

        if !config.exports_fields.is_empty() {
            options.exports_fields = config.exports_fields.clone();
        }

        if !config.main_fields.is_empty() {
            options.main_fields = config.main_fields.clone();
        }

        if !config.main_files.is_empty() {
            options.main_files = config.main_files.clone();
        }
    }

    if options.extensions == [".js", ".json", ".node"] {
        options.extensions = DEFAULT_CODE_EXTENSIONS
            .iter()
            .map(|ext| ext.to_string())
            .collect();
    } else {
        for ext in DEFAULT_CODE_EXTENSIONS {
            if !options.extensions.iter().any(|existing| existing == ext) {
                options.extensions.push((*ext).to_string());
            }
        }
    }

    Resolver::new(options)
}

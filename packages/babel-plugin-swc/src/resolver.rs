use std::path::{Path, PathBuf};

use oxc_resolver::{
    Alias, AliasValue, ResolveOptions, Resolver, Restriction, TsconfigDiscovery, TsconfigOptions,
    TsconfigReferences,
};
use serde_json::Value;

use crate::{constants::DEFAULT_CODE_EXTENSIONS, options::ResolverConfig};

pub fn create_resolver(config: Option<&ResolverConfig>, root_dir: Option<&Path>) -> Resolver {
    let mut options = ResolveOptions::default();

    let working_directory = config
        .and_then(|cfg| cfg.working_directory.as_ref())
        .map(PathBuf::from);

    if let Some(root) = root_dir {
        options.cwd = Some(root.to_path_buf());
    }

    if let Some(ref cwd) = working_directory {
        options.cwd = Some(cwd.clone());
    }

    let base_dir = working_directory.as_deref().or(root_dir);

    if let Some(config) = config {
        if let Some(alias) = parse_alias_config(config.alias.as_ref(), base_dir) {
            options.alias = alias;
        }

        if !config.alias_fields.is_empty() {
            options.alias_fields = config.alias_fields.clone();
        }

        if !config.condition_names.is_empty() {
            options.condition_names = config.condition_names.clone();
        }

        if let Some(extension_alias) = parse_extension_alias(config.extension_alias.as_ref()) {
            options.extension_alias = extension_alias;
        }

        if !config.extensions.is_empty() {
            options.extensions = config.extensions.clone();
        }

        if !config.exports_fields.is_empty() {
            options.exports_fields = config.exports_fields.clone();
        }

        if !config.imports_fields.is_empty() {
            options.imports_fields = config.imports_fields.clone();
        }

        if let Some(fallback) = parse_alias_config(config.fallback.as_ref(), base_dir) {
            options.fallback = fallback;
        }

        if let Some(fully_specified) = config.fully_specified {
            options.fully_specified = fully_specified;
        }

        if !config.main_fields.is_empty() {
            options.main_fields = config.main_fields.clone();
        }

        if !config.main_files.is_empty() {
            options.main_files = config.main_files.clone();
        }

        if !config.modules.is_empty() {
            options.modules = config
                .modules
                .iter()
                .map(|value| resolve_string_path(value, base_dir))
                .collect();
        }

        if let Some(value) = config.prefer_relative {
            options.prefer_relative = value;
        }

        if let Some(value) = config.prefer_absolute {
            options.prefer_absolute = value;
        }

        if !config.restrictions.is_empty() {
            options.restrictions = config
                .restrictions
                .iter()
                .map(|value| Restriction::Path(resolve_path_buf(value, base_dir)))
                .collect();
        }

        if !config.roots.is_empty() {
            options.roots = config
                .roots
                .iter()
                .map(|value| resolve_path_buf(value, base_dir))
                .collect();
        }

        if let Some(value) = config.symlinks {
            options.symlinks = value;
        }

        if let Some(value) = config.builtin_modules {
            options.builtin_modules = value;
        }

        if let Some(tsconfig) = parse_tsconfig(config.tsconfig.as_ref(), base_dir) {
            options.tsconfig = Some(tsconfig);
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

fn parse_alias_config(value: Option<&Value>, base_dir: Option<&Path>) -> Option<Alias> {
    let Some(value) = value else {
        return None;
    };

    let mut entries: Alias = Vec::new();

    match value {
        Value::Object(map) => {
            for (key, raw) in map {
                let alias_values = parse_alias_values(raw, base_dir);
                if !alias_values.is_empty() {
                    entries.push((key.clone(), alias_values));
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                if let Value::Object(obj) = item {
                    if let Some(name) = obj.get("name").and_then(|value| value.as_str()) {
                        let target = obj
                            .get("alias")
                            .or_else(|| obj.get("path"))
                            .or_else(|| obj.get("paths"));
                        if let Some(target) = target {
                            let alias_values = parse_alias_values(target, base_dir);
                            if !alias_values.is_empty() {
                                entries.push((name.to_string(), alias_values));
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }

    if entries.is_empty() {
        None
    } else {
        Some(entries)
    }
}

fn parse_alias_values(value: &Value, base_dir: Option<&Path>) -> Vec<AliasValue> {
    match value {
        Value::String(target) => {
            vec![AliasValue::Path(resolve_string_path(target, base_dir))]
        }
        Value::Bool(false) => vec![AliasValue::Ignore],
        Value::Bool(true) => Vec::new(),
        Value::Array(items) => {
            let mut values = Vec::new();
            for item in items {
                values.extend(parse_alias_values(item, base_dir));
            }
            values
        }
        Value::Object(map) => {
            if let Some(alias) = map.get("alias").or_else(|| map.get("path")) {
                parse_alias_values(alias, base_dir)
            } else if let Some(paths) = map.get("paths") {
                parse_alias_values(paths, base_dir)
            } else if let Some(ignore) = map.get("ignore").and_then(|value| value.as_bool()) {
                if ignore {
                    vec![AliasValue::Ignore]
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

fn parse_extension_alias(value: Option<&Value>) -> Option<Vec<(String, Vec<String>)>> {
    let Some(Value::Object(map)) = value else {
        return None;
    };

    let mut entries = Vec::new();
    for (ext, raw) in map {
        let mut values = Vec::new();
        match raw {
            Value::String(v) => values.push(v.to_string()),
            Value::Array(items) => {
                for item in items {
                    if let Some(v) = item.as_str() {
                        values.push(v.to_string());
                    }
                }
            }
            _ => {}
        }

        if !values.is_empty() {
            entries.push((ext.clone(), values));
        }
    }

    if entries.is_empty() {
        None
    } else {
        Some(entries)
    }
}

fn parse_tsconfig(value: Option<&Value>, base_dir: Option<&Path>) -> Option<TsconfigDiscovery> {
    let Some(value) = value else {
        return None;
    };

    match value {
        Value::String(mode) => {
            if mode == "auto" {
                Some(TsconfigDiscovery::Auto)
            } else {
                Some(TsconfigDiscovery::Manual(TsconfigOptions {
                    config_file: resolve_path_buf(mode, base_dir),
                    references: TsconfigReferences::Auto,
                }))
            }
        }
        Value::Object(map) => {
            let config_file = map
                .get("configFile")
                .and_then(|value| value.as_str())
                .map(|path| resolve_path_buf(path, base_dir));

            let references = match map.get("references") {
                Some(Value::Bool(false)) => TsconfigReferences::Disabled,
                Some(Value::Bool(true)) => TsconfigReferences::Auto,
                Some(Value::String(value)) if value == "auto" => TsconfigReferences::Auto,
                Some(Value::String(value)) if value == "disabled" => TsconfigReferences::Disabled,
                Some(Value::Array(items)) => {
                    let mut paths = Vec::new();
                    for item in items {
                        if let Some(path) = item.as_str() {
                            paths.push(resolve_path_buf(path, base_dir));
                        }
                    }
                    TsconfigReferences::Paths(paths)
                }
                _ => TsconfigReferences::Auto,
            };

            config_file.map(|config_file| {
                TsconfigDiscovery::Manual(TsconfigOptions {
                    config_file,
                    references,
                })
            })
        }
        _ => None,
    }
}

fn resolve_string_path(value: &str, base_dir: Option<&Path>) -> String {
    if should_join_relative(value) {
        if let Some(base) = base_dir {
            return base.join(value).to_string_lossy().into_owned();
        }
    }

    value.to_string()
}

fn resolve_path_buf(value: &str, base_dir: Option<&Path>) -> PathBuf {
    if should_join_relative(value) {
        if let Some(base) = base_dir {
            return base.join(value);
        }
    }

    PathBuf::from(value)
}

fn should_join_relative(value: &str) -> bool {
    matches!(value, "." | "..")
        || value.starts_with("./")
        || value.starts_with(".\\")
        || value.starts_with("../")
        || value.starts_with("..\\")
}

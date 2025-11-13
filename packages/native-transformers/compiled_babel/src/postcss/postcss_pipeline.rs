#[cfg(feature = "postcss_engine")]
use postcss as pc;
#[cfg(feature = "postcss_engine")]
use postcss::ast::NodeAccess;
use postcss::ast::nodes::{as_declaration, as_rule, Declaration as PcDeclaration, Rule as PcRule};

use super::transform::{TransformCssOptions, TransformCssResult, CssTransformError};
use std::sync::{Arc, Mutex};

#[cfg(feature = "postcss_engine")]
#[derive(Clone, Default)]
struct AtomicCollector {
    sheets: Arc<Mutex<Vec<String>>>,
    class_names: Arc<Mutex<Vec<String>>>,
}

#[cfg(feature = "postcss_engine")]
impl AtomicCollector {
    fn push_sheet(&self, css: String) {
        self.sheets.lock().unwrap().push(css);
    }

    fn push_class(&self, class: String) {
        self.class_names.lock().unwrap().push(class);
    }

    fn take(self) -> (Vec<String>, Vec<String>) {
        let sheets = Arc::try_unwrap(self.sheets)
            .unwrap_or_else(|arc| (*arc.lock().unwrap()).clone().into())
            .into_inner()
            .unwrap_or_default();
        let classes = Arc::try_unwrap(self.class_names)
            .unwrap_or_else(|arc| (*arc.lock().unwrap()).clone().into())
            .into_inner()
            .unwrap_or_default();
        (sheets, classes)
    }
}

#[cfg(feature = "postcss_engine")]
fn is_empty_value(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty() || trimmed == "undefined" || trimmed == "null"
}

#[cfg(feature = "postcss_engine")]
fn discard_empty_in_container(container: &pc::RootLike) {
    // Remove empty-valued declarations within every rule/at-rule, then
    // remove empty rules/at-rules recursively.
    match container {
        pc::RootLike::Root(root) => {
            // Clean declarations in all rules under root.
            root.walk_rules(|rule_ref, _| {
                let rule: PcRule = match as_rule(&rule_ref) {
                    Some(r) => r,
                    None => return true,
                };
                let children = rule.nodes();
                for child in children {
                    if let Some(decl) = as_declaration(&child) {
                        let decl: PcDeclaration = decl;
                        let value = decl.value();
                        if is_empty_value(&value) {
                            rule.remove_child(child);
                        }
                    }
                }
                true
            });

            // Remove rules that no longer have any children.
            let mut remove_rules: Vec<pc::ast::NodeRef> = Vec::new();
            root.walk_rules(|rule_ref, _| {
                if let Some(rule) = as_rule(&rule_ref) {
                    if rule.nodes().is_empty() {
                        remove_rules.push(rule_ref.clone());
                    }
                }
                true
            });
            for r in remove_rules {
                root.remove_child(r);
            }

            // Note: removing empty rules requires parent access; for now we
            // only prune empty-valued declarations across the full tree.
        }
        pc::RootLike::Document(document) => {
            // Clean under document similarly
            document.walk_rules(|rule_ref, _| {
                let rule: PcRule = match as_rule(&rule_ref) {
                    Some(r) => r,
                    None => return true,
                };
                let children = rule.nodes();
                for child in children {
                    if let Some(decl) = as_declaration(&child) {
                        let decl: PcDeclaration = decl;
                        let value = decl.value();
                        if is_empty_value(&value) {
                            rule.remove_child(child);
                        }
                    }
                }
                true
            });

            let mut remove_rules: Vec<pc::ast::NodeRef> = Vec::new();
            document.walk_rules(|rule_ref, _| {
                if let Some(rule) = as_rule(&rule_ref) {
                    if rule.nodes().is_empty() {
                        remove_rules.push(rule_ref.clone());
                    }
                }
                true
            });
            for r in remove_rules {
                document.remove_child(r);
            }

            // Note: at-rule empty removal will be added once public helpers are exposed.
        }
    }
}

#[cfg(feature = "postcss_engine")]
fn discard_empty_rules_plugin() -> pc::BuiltPlugin {
    pc::plugin("discard-empty-rules").once_exit(|root, _result| {
        discard_empty_in_container(root);
        Ok(())
    }).build()
}

#[cfg(feature = "postcss_engine")]
fn build_processor(options: &TransformCssOptions, collector: &AtomicCollector) -> pc::Processor {
    // Map the JS pipeline order with placeholder no-op plugins for now.
    // Each will be replaced with behaviourally identical implementations.
    let mut plugins = vec![
        pc::plugin("discard-duplicates").build(),
        discard_empty_rules_plugin(),
        pc::plugin("parent-orphaned-pseudos").build(),
        pc::plugin("postcss-nested").build(),
        // Pre-atomicify base normalizers used by the original pipeline.
        super::plugins::normalize_css_engine::minify_selectors::plugin(),
        super::plugins::normalize_css_engine::minify_params::plugin(),
        pc::plugin("normalize-css").build(),
    ];

    // OptimizeCss plugins (cssnano preset subset) enabled when optimize_css is true (default).
    if options.optimize_css.unwrap_or(true) {
        use super::plugins::normalize_css_engine as nce;
        plugins.push(nce::ordered_values::plugin());
        plugins.push(nce::reduce_initial::plugin());
        plugins.push(nce::convert_values::plugin());
        plugins.push(nce::colormin::plugin());
        plugins.push(nce::normalize_current_color::plugin());
        plugins.push(nce::discard_comments::plugin());
        plugins.push(nce::normalize_url::plugin());
        plugins.push(nce::normalize_string::plugin());
        plugins.push(nce::normalize_positions::plugin());
        plugins.push(nce::normalize_timing_functions::plugin());
        plugins.push(nce::minify_gradients::plugin());
        plugins.push(nce::calc::plugin());
    }

    // Continue pipeline.
    let tail = vec![
        pc::plugin("expand-shorthands").build(),
        atomicify_rules_plugin(options.clone(), collector.clone()),
        pc::plugin("flatten-multiple-selectors").build(),
        pc::plugin("discard-duplicates-2").build(),
        pc::plugin("increase-specificity").build(),
        pc::plugin("sort-atomic-style-sheet").build(),
        pc::plugin("autoprefixer").build(),
        normalize_whitespace_plugin(),
        pc::plugin("extract-stylesheets").build(),
    ];

    plugins.extend(tail);

    pc::postcss_with_plugins(plugins)
}

#[cfg(feature = "postcss_engine")]
fn normalize_whitespace_plugin() -> pc::BuiltPlugin {
    // Light whitespace normalization similar to postcss-normalize-whitespace.
    // We leverage the stringifier defaults by cleaning raws on exit.
    pc::plugin("normalize-whitespace")
        .once_exit(|root, _result| {
            match root {
                pc::RootLike::Root(r) => r.clean_raws(true),
                pc::RootLike::Document(d) => d.clean_raws(true),
            }
            Ok(())
        })
        .build()
}

// minify_selectors_plugin and minify_params_plugin now live under plugins::normalize_css

#[cfg(feature = "postcss_engine")]
fn atomicify_rules_plugin(options: TransformCssOptions, collector: AtomicCollector) -> pc::BuiltPlugin {
    use postcss::list::comma;
    use crate::utils_hash::hash;

    #[derive(Clone)]
    struct Ctx<'a> {
        at_chain: Vec<(String, String)>, // (name, params)
        selectors: Vec<String>,          // combined selectors at this depth
        opts: &'a TransformCssOptions,
        collector: AtomicCollector,
    }

    fn can_atomicify_at_rule(name: &str) -> bool {
        matches!(name,
            "container" | "-moz-document" | "else" | "layer" | "media" | "starting-style" | "supports" | "when"
        )
    }

    fn combine_selectors(parent: &[String], child: &str) -> Vec<String> {
        let child_parts = comma(child);
        let parents = if parent.is_empty() { vec!["&".to_string()] } else { parent.to_vec() };
        let mut out = Vec::new();
        for p in parents {
            for c in &child_parts {
                let trimmed = c.trim();
                if trimmed.contains('&') {
                    out.push(trimmed.replace('&', &p));
                } else if p == "&" {
                    out.push(trimmed.to_string());
                } else if trimmed.is_empty() {
                    out.push(p.clone());
                } else {
                    out.push(format!("{} {}", p, trimmed));
                }
            }
        }
        out
    }

    fn normalized_selector(selector: &str) -> String {
        let trimmed = selector.trim();
        if trimmed.contains('&') { trimmed.to_string() } else { format!("& {}", trimmed) }
    }

    fn at_chain_label(at_chain: &[(String, String)]) -> String {
        let mut s = String::new();
        for (n, p) in at_chain {
            s.push_str(n);
            s.push_str(p);
        }
        s
    }

    fn wrap_in_at_rules(rule_css: &str, at_chain: &[(String, String)]) -> String {
        if at_chain.is_empty() { return rule_css.to_string(); }
        let mut out = String::new();
        for (n, p) in at_chain {
            if p.is_empty() {
                out.push_str(&format!("@{}{{", n));
            } else {
                out.push_str(&format!("@{} {}{{", n, p));
            }
        }
        out.push_str(rule_css);
        for _ in at_chain { out.push('}'); }
        out
    }

    fn process_rule(rule: &PcRule, ctx: &mut Ctx) {
        // Emit atomic rules for each declaration in this rule
        let children = rule.nodes();
        for child in children {
            if let Some(decl) = as_declaration(&child) {
                let prop = decl.prop();
                let mut value = decl.value();
                if decl.important() { value.push_str("!important"); }

                // For each combined selector, compute class and output rule
                let mut replaced_selectors: Vec<String> = Vec::new();
                for sel in &ctx.selectors {
                    let norm = normalized_selector(sel);
                    let mut group_seed = String::new();
                    if let Some(prefix) = &ctx.opts.class_hash_prefix { group_seed.push_str(prefix); }
                    group_seed.push_str(&at_chain_label(&ctx.at_chain));
                    group_seed.push_str(&norm);
                    group_seed.push_str(&prop);
                    let group = hash(&group_seed).chars().take(4).collect::<String>();
                    let value_hash = hash(&value).chars().take(4).collect::<String>();
                    let class = format!("_{}{}", group, value_hash);
                    ctx.collector.push_class(class.clone());
                    // Replace '&' with class selector
                    let replaced = norm.replace('&', &format!(".{}", class));
                    replaced_selectors.push(replaced);
                }

                let selector_joined = replaced_selectors.join(", ");
                let rule_css = format!("{}{{{}:{}}}", selector_joined, prop, decl.value());
                let wrapped = wrap_in_at_rules(&rule_css, &ctx.at_chain);
                ctx.collector.push_sheet(wrapped);
            } else if let Some(nested) = as_rule(&child) {
                // Recurse nested rules
                let sels = combine_selectors(&ctx.selectors, &nested.selector());
                let mut next = ctx.clone();
                next.selectors = sels;
                process_rule(&nested, &mut next);
            }
        }
    }

    let at_stack = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
    let sel_stack = Arc::new(Mutex::new(vec![vec!["&".to_string()]]));

    postcss::plugin("atomicify-rules")
        .at_rule_filter("*", {
            let at_stack = at_stack.clone();
            move |at, _| {
                if can_atomicify_at_rule(&at.name()) {
                    at_stack.lock().unwrap().push((at.name(), at.params()));
                }
                Ok(())
            }
        })
        .at_rule_filter_exit("*", {
            let at_stack = at_stack.clone();
            move |_, _| {
                let _ = at_stack.lock().unwrap().pop();
                Ok(())
            }
        })
        .rule_filter("*", {
            let sel_stack = sel_stack.clone();
            move |rule, _| {
                let mut stack = sel_stack.lock().unwrap();
                let parent = stack.last().cloned().unwrap_or_else(|| vec!["&".to_string()]);
                let combined = combine_selectors(&parent, &rule.selector());
                stack.push(combined);
                Ok(())
            }
        })
        .rule_filter_exit("*", {
            let sel_stack = sel_stack.clone();
            let at_stack = at_stack.clone();
            let collector = collector.clone();
            let opts = options.clone();
            move |rule, _| {
                let selectors = {
                    let stack = sel_stack.lock().unwrap();
                    stack.last().cloned().unwrap_or_else(|| vec!["&".to_string()])
                };

                let at_chain = at_stack.lock().unwrap().clone();
                let at_label = at_chain_label(&at_chain);

                for child in rule.nodes() {
                    if let Some(decl) = as_declaration(&child) {
                        let prop = decl.prop();
                        let mut value_full = decl.value();
                        if decl.important() { value_full.push_str("!important"); }

                        let mut replaced_selectors: Vec<String> = Vec::new();
                        for sel in &selectors {
                            let norm = normalized_selector(sel);
                            let mut group_seed = String::new();
                            if let Some(prefix) = &opts.class_hash_prefix { group_seed.push_str(prefix); }
                            group_seed.push_str(&at_label);
                            group_seed.push_str(&norm);
                            group_seed.push_str(&prop);
                            let group = hash(&group_seed).chars().take(4).collect::<String>();
                            let value_hash = hash(&value_full).chars().take(4).collect::<String>();
                            let full_class = format!("_{}{}", group, value_hash);
                            collector.push_class(full_class.clone());
                            // Replace using compressed class if map provided.
                            let used_class = if let Some(map) = &opts.class_name_compression_map {
                                let key = full_class.trim_start_matches('_');
                                if let Some(compressed) = map.get(key) { compressed.clone() } else { full_class.clone() }
                            } else {
                                full_class.clone()
                            };
                            let replaced = norm.replace('&', &format!(".{}", used_class));
                            replaced_selectors.push(replaced);
                        }

                        let selector_joined = replaced_selectors.join(", ");
                        let rule_css = format!("{}{{{}:{}}}", selector_joined, prop, decl.value());
                        let wrapped = wrap_in_at_rules(&rule_css, &at_chain);
                        collector.push_sheet(wrapped);
                    }
                }

                let mut stack = sel_stack.lock().unwrap();
                let _ = stack.pop();
                Ok(())
            }
        })
        .build()
}

/// Experimental PostCSS-engine-backed pipeline.
/// Currently parses and serializes CSS via the vendored PostCSS crate
/// without plugins, returning a single sheet and no class names.
/// This is a staging point to wire the original plugin chain identically.
pub fn transform_css_via_postcss(
    css: &str,
    _options: TransformCssOptions,
)
-> Result<TransformCssResult, CssTransformError> {
    // Shared collector for atomic outputs.
    let collector = AtomicCollector::default();
    // Create a processor with the staged plugin chain.
    let processor = build_processor(&_options, &collector);

    // Process input CSS into a Result, using default options.
    let mut result = processor
        .process(css)
        .map_err(|e| CssTransformError::from_message(format!("postcss error: {e}")))?;

    // Collect atomic outputs from the plugin.
    let (sheets, mut class_names) = collector.take();
    // Deduplicate classes preserving order.
    let mut seen = std::collections::HashSet::new();
    class_names.retain(|c| seen.insert(c.clone()));
    // Order classes by first appearance in sheets to match runtime expectations.
    fn extract_first_class_from_sheet(sheet: &str) -> Option<String> {
        if let Some(dot) = sheet.find('.') {
            let rest = &sheet[dot + 1..];
            let end = rest.find(|c: char| c == '{' || c == ' ' || c == ',').unwrap_or(rest.len());
            let name = &rest[..end];
            if !name.is_empty() { return Some(name.to_string()); }
        }
        None
    }
    use std::collections::HashMap;
    let mut order: HashMap<String, usize> = HashMap::new();
    for (i, sheet) in sheets.iter().enumerate() {
        if let Some(class_name) = extract_first_class_from_sheet(sheet) {
            order.entry(class_name).or_insert(i);
        }
    }
    class_names.sort_by_key(|name| order.get(name).copied().unwrap_or(usize::MAX));

    Ok(TransformCssResult { sheets, class_names })
}


#[cfg(feature = "postcss_engine")]
use postcss as pc;
#[cfg(feature = "postcss_engine")]
use postcss::ast::NodeAccess;
use postcss::ast::nodes::{as_declaration, as_rule, Declaration as PcDeclaration, Rule as PcRule};

use super::transform::{transform_css_via_swc_pipeline, CssTransformError, TransformCssOptions, TransformCssResult};
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
        // Do not rely on Arc::try_unwrap since plugin closures may still
        // hold references while the processor struct is alive. Instead,
        // extract contents under the mutex and leave the Arc in place.
        let sheets = {
            let mut guard = self.sheets.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        let classes = {
            let mut guard = self.class_names.lock().unwrap();
            std::mem::take(&mut *guard)
        };
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
    // Step 2 of bisect: add a small batch of light plugins
    // Keep known-problematic normalizers (minify-params, normalize-string, normalize-url) disabled for now.
    let mut plugins: Vec<pc::BuiltPlugin> = vec![
        pc::plugin("discard-duplicates").build(),
        discard_empty_rules_plugin(),
        pc::plugin("parent-orphaned-pseudos").build(),
        pc::plugin("postcss-nested").build(),
        super::plugins::normalize_css_engine::minify_selectors::plugin(),
        super::plugins::normalize_css_engine::minify_params::plugin(),
        // cssnano-like optimizers (safe subset first)
        // Keep normalize-url/normalize-string/minify-params disabled until last.
        {
            use super::plugins::normalize_css_engine as nce;
            nce::ordered_values::plugin()
        },
        {
            use super::plugins::normalize_css_engine as nce;
            nce::reduce_initial::plugin()
        },
        {
            use super::plugins::normalize_css_engine as nce;
            nce::convert_values::plugin()
        },
        {
            use super::plugins::normalize_css_engine as nce;
            nce::colormin::plugin()
        },
        {
            use super::plugins::normalize_css_engine as nce;
            nce::normalize_current_color_plugin()
        },
        {
            use super::plugins::normalize_css_engine as nce;
            nce::discard_comments_plugin()
        },
        // Add normalize-url next in the bisect sequence
        {
            use super::plugins::normalize_css_engine as nce;
            nce::normalize_url::plugin()
        },
        // Add normalize-string after normalize-url
        {
            use super::plugins::normalize_css_engine as nce;
            nce::normalize_string::plugin()
        },
        pc::plugin("expand-shorthands").build(),
        // Start emitting atomic rules; keep remaining optimizers disabled for now.
        atomicify_rules_plugin(options.clone(), collector.clone()),
        pc::plugin("flatten-multiple-selectors").build(),
        pc::plugin("discard-duplicates-2").build(),
        pc::plugin("increase-specificity").build(),
        pc::plugin("sort-atomic-style-sheet").build(),
        normalize_whitespace_plugin(),
        pc::plugin("extract-stylesheets").build(),
    ];
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
                let mut value_full = decl.value();
                // Normalize color values before hashing to match Babel
                fn minify_color_value(value: &str) -> String {
                    let trimmed = value.trim();
                    if trimmed.is_empty() { return value.to_string(); }
                    let opts = super::plugins::normalize_css_engine::colormin::add_plugin_defaults();
                    let min = super::plugins::normalize_css_engine::colormin::transform_value(trimmed, &opts);
                    if min.len() < trimmed.len() { min } else { trimmed.to_lowercase() }
                }
                value_full = minify_color_value(&value_full);
                if decl.important() { value_full.push_str("!important"); }

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
                    let value_hash = hash(&value_full).chars().take(4).collect::<String>();
                    let class = format!("_{}{}", group, value_hash);
                    ctx.collector.push_class(class.clone());
                    // Replace '&' with class selector
                    let replaced = norm.replace('&', &format!(".{}", class));
                    replaced_selectors.push(replaced);
                }

                let selector_joined = replaced_selectors.join(", ");
                let rule_css = format!("{}{{{}:{}}}", selector_joined, prop, value_full);
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
        // Handle declarations that appear directly under Root or AtRule trees.
        .decl({
            let sel_stack = sel_stack.clone();
            let at_stack = at_stack.clone();
            let collector = collector.clone();
            let opts = options.clone();
            move |decl, _| {
                // Skip if this declaration lives under a normal Rule; the rule_exit
                // hook will handle those to avoid double emission.
                let parent = decl.to_node().borrow().parent();
                if let Some(p) = parent {
                    if as_rule(&p).is_some() {
                        return Ok(());
                    }
                }

                let selectors = {
                    let stack = sel_stack.lock().unwrap();
                    stack.last().cloned().unwrap_or_else(|| vec!["&".to_string()])
                };
                let at_chain = at_stack.lock().unwrap().clone();
                let at_label = at_chain_label(&at_chain);

                let prop = decl.prop();
                let mut value_full = decl.value();
                // Normalize color values pre-hash like the original pipeline.
                fn minify_color_value(value: &str) -> String {
                    let trimmed = value.trim();
                    if trimmed.is_empty() { return value.to_string(); }
                    let opts = super::plugins::normalize_css_engine::colormin::add_plugin_defaults();
                    let min = super::plugins::normalize_css_engine::colormin::transform_value(trimmed, &opts);
                    if min.len() < trimmed.len() { min } else { trimmed.to_lowercase() }
                }
                value_full = minify_color_value(&value_full);
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
                let rule_css = format!("{}{{{}:{}}}", selector_joined, prop, value_full);
                let wrapped = wrap_in_at_rules(&rule_css, &at_chain);
                collector.push_sheet(wrapped);
                Ok(())
            }
        })
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
            let opts = options.clone();
            move |rule, _| {
                let mut stack = sel_stack.lock().unwrap();
                let parent = stack.last().cloned().unwrap_or_else(|| vec!["&".to_string()]);
                let raw_selector = rule.selector();
                let combined = if let Some(ph) = &opts.declaration_placeholder {
                    if raw_selector == *ph { parent } else { combine_selectors(&parent, &raw_selector) }
                } else {
                    combine_selectors(&parent, &raw_selector)
                };
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

                // Minimal color minifier to mirror cssnano before hashing.
                fn minify_color_value(value: &str) -> String {
                    // Only attempt for simple identifiers; leave complex values untouched here.
                    let trimmed = value.trim();
                    if trimmed.is_empty() { return value.to_string(); }
                    // Delegate to the same colormin transformer used by the plugin to ensure 1:1.
                    // Use default options (modern defaults), consistent with our plugin defaults.
                    let opts = super::plugins::normalize_css_engine::colormin::add_plugin_defaults();
                    let min = super::plugins::normalize_css_engine::colormin::transform_value(trimmed, &opts);
                    let out = if min.len() < trimmed.len() { min } else { trimmed.to_lowercase() };
                    if std::env::var("COMPILED_DEBUG_COLORMIN").is_ok() {
                        eprintln!("[atomicify] colormin: '{}' -> '{}'", trimmed, out);
                    }
                    out
                }

                for child in rule.nodes() {
                    if let Some(decl) = as_declaration(&child) {
                        let prop = decl.prop();
                        let mut value_full = decl.value();
                        // Ensure hashing sees normalized color values.
                        value_full = minify_color_value(&value_full);
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
                        let rule_css = format!("{}{{{}:{}}}", selector_joined, prop, value_full);
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
    mut options: TransformCssOptions,
)
-> Result<TransformCssResult, CssTransformError> {
    if std::env::var("COMPILED_DEBUG_COLORMIN").is_ok() {
        eprintln!("[postcss-pipeline] input css: {}", css.replace('\n', "\\n"));
    }
    if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] via-postcss begin"); }
    // Shared collector for atomic outputs.
    let collector = AtomicCollector::default();
    // Create a processor with the staged plugin chain.
    let mut processor = build_processor(&options, &collector);

    // First attempt to process the CSS directly.
    if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] process initial"); }
    let mut result = match processor.process(css) {
        Ok(res) => res,
        Err(err) => {
            // Mirror Babel/JS fallback: wrap declarations in a placeholder rule and retry.
            const PLACEHOLDER: &str = "__compiled_declaration_wrapper__";
            let wrapped = format!(".{PLACEHOLDER} {{{}}}", css);
            options.declaration_placeholder = Some(format!(".{PLACEHOLDER}"));
            // Rebuild the processor to pass updated options through to plugins.
            let collector = AtomicCollector::default();
            processor = build_processor(&options, &collector);
            // Retry with wrapped input; if this fails, surface the original error.
            if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] process wrapped"); }
            match processor.process(&wrapped) {
                Ok(res) => res,
                Err(_) => return Err(CssTransformError::from_message(format!("postcss error: {err}"))),
            }
        }
    };
    // Force evaluation so plugin visitors run (PostCSS is lazy),
    // but avoid full stringification for performance.
    if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] ensure visitors run"); }
    let _ = result.result();

    // Collect atomic outputs from the plugin.
    if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] take collector"); }
    let (mut sheets, mut class_names) = collector.take();
    // eprintln!("[postcss-pipeline] after first pass, sheets={}", sheets.len());
    // If PostCSS parsed the input as declarations (no rules) successfully,
    // the pipeline will emit no sheets. To mirror Babel, retry by wrapping
    // the declarations in a placeholder rule and reprocessing.
    if sheets.is_empty() && options.declaration_placeholder.is_none() {
        const PLACEHOLDER: &str = "__compiled_declaration_wrapper__";
        let wrapped = format!(".{PLACEHOLDER} {{{}}}", css);
        options.declaration_placeholder = Some(format!(".{PLACEHOLDER}"));
        let collector2 = AtomicCollector::default();
        let mut processor2 = build_processor(&options, &collector2);
        if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] process wrapped 2"); }
        match processor2.process(&wrapped) {
            Ok(mut res2) => {
                // Force evaluation without stringifying output
                if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] ensure visitors run 2"); }
                let _ = res2.result();
                if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] take collector 2"); }
                let (s2, mut c2) = collector2.take();
                if !s2.is_empty() {
                    sheets = s2;
                    // Prefer classes from the second pass when present.
                    class_names.append(&mut c2);
                }
            }
            Err(_) => {}
        }
    }
    // eprintln!("[postcss-pipeline] final sheets={}", sheets.len());
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

    if sheets.is_empty() {
        // Final fallback: run the SWC-backed pipeline to mirror Babel output exactly.
        if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] fallback to swc"); }
        return transform_css_via_swc_pipeline(css, options);
    }

    if std::env::var("COMPILED_CLI_TRACE").is_ok() { eprintln!("[postcss] via-postcss end"); }
    Ok(TransformCssResult { sheets, class_names })
}


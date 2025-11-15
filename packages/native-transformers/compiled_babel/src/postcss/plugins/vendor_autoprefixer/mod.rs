use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use swc_core::css::ast::{AtRuleName, ComponentValue, Declaration, DeclarationName, QualifiedRule, Rule, Stylesheet};

use crate::postcss::plugins::vendor_prefixing_lite::make_ident;
use super::super::transform::{Plugin, TransformContext};

pub(crate) static PREFIXES_JSON: Lazy<Option<&'static str>> = Lazy::new(|| {
    Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/autoprefixer_data/prefixes.json")))
});

pub(crate) static AGENTS_JSON: Lazy<Option<&'static str>> = Lazy::new(|| {
    Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/autoprefixer_data/agents.json")))
});

#[derive(Debug, Deserialize)]
struct PrefixEntry {
    #[serde(default)]
    browsers: Vec<String>,
    #[serde(default)]
    mistakes: Vec<String>,
    #[serde(default)]
    feature: Option<String>,
    #[serde(default)]
    props: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AgentsMap(HashMap<String, Agent>);

#[derive(Debug, Deserialize)]
struct Agent {
    prefix: String,
    #[serde(default, rename = "prefix_exceptions")]
    prefix_exceptions: Option<HashMap<String, String>>,
}

#[derive(Debug, Default)]
pub struct PrefixDB {
    // name -> entry
    entries: HashMap<String, PrefixEntry>,
    agents: HashMap<String, Agent>,
}

impl PrefixDB {
    pub fn load() -> Option<Self> {
        let pjson = PREFIXES_JSON.as_ref().and_then(|s| Some(*s))?;
        let aj = AGENTS_JSON.as_ref().and_then(|s| Some(*s))?;
        let entries: HashMap<String, PrefixEntry> = match serde_json::from_str(pjson) {
            Ok(v) => v,
            Err(_) => return None,
        };
        let agents: HashMap<String, Agent> = match serde_json::from_str::<AgentsMap>(aj) {
            Ok(v) => v.0,
            Err(_) => return None,
        };
        Some(PrefixDB { entries, agents })
    }

    #[allow(dead_code)]
    pub fn load_from_str(prefixes_json: &str, agents_json: &str) -> Option<Self> {
        let entries: HashMap<String, PrefixEntry> = serde_json::from_str(prefixes_json).ok()?;
        let agents: HashMap<String, Agent> = serde_json::from_str::<AgentsMap>(agents_json).ok()?.0;
        Some(PrefixDB { entries, agents })
    }

    fn prefix_for_browser(&self, browser: &str) -> Option<String> {
        let (name, version) = browser.split_once(' ')?;
        let agent = self.agents.get(name)?;
        if let Some(map) = &agent.prefix_exceptions {
            if let Some(p) = map.get(version) {
                return Some(format!("-{}-", p));
            }
        }
        Some(format!("-{}-", agent.prefix))
    }

    pub fn select_add_remove(&self, selected_browsers: &[String]) -> (HashMap<String, Vec<String>>, HashMap<String, Vec<String>>) {
        let mut add: HashMap<String, Vec<String>> = HashMap::new();
        let mut remove: HashMap<String, Vec<String>> = HashMap::new();
        let sel: BTreeSet<String> = selected_browsers.iter().cloned().collect();

        for (name, data) in &self.entries {
            let all_prefixes: BTreeSet<String> = {
                let mut s: BTreeSet<String> = BTreeSet::new();
                for br in &data.browsers { if let Some(pref) = self.prefix_for_browser(br) { s.insert(pref); } }
                for m in &data.mistakes { s.insert(m.clone()); }
                s
            };

            let mut need: BTreeSet<String> = BTreeSet::new();
            for br in &data.browsers {
                let parts: Vec<&str> = br.split(' ').collect();
                let simple = if parts.len() >= 2 { format!("{} {}", parts[0], parts[1]) } else { br.clone() };
                if sel.contains(&simple) {
                    if let Some(pref) = self.prefix_for_browser(&simple) { need.insert(pref); }
                }
            }

            if !need.is_empty() {
                add.insert(name.clone(), need.iter().cloned().collect());
                let rem: Vec<String> = all_prefixes.difference(&need).cloned().collect();
                if !rem.is_empty() {
                    remove.insert(name.clone(), rem);
                }
            } else if !all_prefixes.is_empty() {
                remove.insert(name.clone(), all_prefixes.into_iter().collect());
            }
        }
        (add, remove)
    }
}

#[derive(Debug, Default)]
pub struct VendorAutoprefixer;

pub fn vendor_autoprefixer() -> VendorAutoprefixer { VendorAutoprefixer }

impl Plugin for VendorAutoprefixer {
    fn name(&self) -> &'static str { "autoprefixer" }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        let Some(db) = PrefixDB::load() else { return; };
        let targets = resolve_browserslist_targets();
        let (add, remove) = db.select_add_remove(&targets);
        let tracing = std::env::var("COMPILED_CLI_TRACE").is_ok();
        if tracing {
            eprintln!(
                "[autoprefixer] targets={} add_keys={} remove_keys={}",
                targets.join(", "), add.len(), remove.len()
            );
        }

        // Pass 1: @keyframes and @viewport
        let mut new_rules: Vec<Rule> = Vec::new();
        for rule in std::mem::take(&mut stylesheet.rules) {
            match rule {
                Rule::AtRule(at) => {
                    if let Some(name) = at_rule_name(&at.name) {
                        if name == "keyframes" {
                            if let Some(prefixes) = add.get("@keyframes") {
                                for pref in prefixes {
                                    let mut cloned = (*at.clone()).clone();
                                    cloned.name = at_rule_name_from_str(&format!("{}keyframes", pref));
                                    new_rules.push(Rule::AtRule(Box::new(cloned)));
                                }
                            }
                        }
                    }
                    new_rules.push(Rule::AtRule(at));
                }
                Rule::QualifiedRule(mut qr) => {
                    apply_decl_prefixing(&mut qr, &add, &remove);
                    new_rules.push(Rule::QualifiedRule(qr));
                }
                other => new_rules.push(other),
            }
        }
        stylesheet.rules = new_rules;
    }
}

fn resolve_browserslist_targets() -> Vec<String> {
    // Use oxc_browserslist to resolve defaults (no repo-specific options requested)
    let mut out = Vec::new();
    let opts = oxc_browserslist::Opts::default();
    match oxc_browserslist::execute(&opts) {
        Ok(list) => {
            for item in list { out.push(item.to_string()); }
        }
        Err(_) => {}
    }
    out
}

fn at_rule_name(name: &AtRuleName) -> Option<&str> {
    match name { AtRuleName::Ident(i) => Some(&i.value), AtRuleName::DashedIdent(i) => Some(&i.value) }
}

fn at_rule_name_from_str(name: &str) -> AtRuleName {
    AtRuleName::Ident(swc_core::css::ast::Ident { value: name.into(), raw: None, span: Default::default() })
}

fn decl_prop(name: &DeclarationName) -> &str {
    match name { DeclarationName::Ident(i) => &i.value, DeclarationName::DashedIdent(i) => &i.value }
}

fn clone_decl_with_prop(decl: &Declaration, prop: String) -> Declaration {
    let mut d = decl.clone();
    d.name = DeclarationName::Ident(swc_core::css::ast::Ident { value: prop.into(), raw: None, span: Default::default() });
    d
}

fn apply_decl_prefixing(rule: &mut QualifiedRule, add: &HashMap<String, Vec<String>>, remove: &HashMap<String, Vec<String>>) {
    let mut new_block: Vec<ComponentValue> = Vec::new();
    for node in std::mem::take(&mut rule.block.value) {
        if let ComponentValue::Declaration(decl_box) = &node {
            let decl = &**decl_box;
            let prop = decl_prop(&decl.name).to_string();

            // Property-level: add prefixed properties
            if let Some(prefixes) = add.get(&prop) {
                for pref in prefixes {
                    let prefixed_prop = format!("{}{}", pref, prop);
                    if std::env::var("COMPILED_CLI_TRACE").is_ok() {
                        eprintln!("[autoprefixer] add-prop {}", prefixed_prop);
                    }
                    new_block.push(ComponentValue::Declaration(Box::new(clone_decl_with_prop(decl, prefixed_prop))));
                }
            }

            // Value-level: naive support for fit-content case and vendor functions
            let value_prefixed = maybe_prefix_value(&prop, &decl.value, add);
            for v in value_prefixed { new_block.push(ComponentValue::Declaration(Box::new(v))); }

            // Removal of old props (basic): skip emitting outdated prefixed properties
            if let Some(rem) = remove.get(&prop) {
                let n = decl_prop(&decl.name);
                if n.starts_with("-") && rem.iter().any(|p| n.starts_with(p)) {
                    if std::env::var("COMPILED_CLI_TRACE").is_ok() {
                        eprintln!("[autoprefixer] drop-old-prop {}", n);
                    }
                    // skip this node (old prefixed property slated for removal)
                    continue;
                }
            }
        }
        new_block.push(node);
    }
    rule.block.value = new_block;
}

fn maybe_prefix_value(prop: &str, value: &Vec<ComponentValue>, add: &HashMap<String, Vec<String>>) -> Vec<Declaration> {
    let mut out: Vec<Declaration> = Vec::new();
    // Special-case: width/min/max fit-content -> -moz-fit-content
    if matches!(prop, "width"|"min-width"|"max-width") {
        if value.len() == 1 { if let ComponentValue::Ident(i) = &value[0] {
            let low = i.value.to_ascii_lowercase();
            if i.value.trim().eq_ignore_ascii_case("fit-content") || low.contains("fit-content") {
                let d = Declaration { name: DeclarationName::Ident(swc_core::css::ast::Ident { value: prop.into(), raw: None, span: Default::default() }), value: vec![make_ident("-moz-fit-content")], important: None, span: Default::default() };
                out.push(d);
            }
        }}
    }

    // display:flex and inline-flex basic prefixes
    if prop == "display" && !value.is_empty() {
        if let ComponentValue::Ident(i) = &value[0] {
            let v = i.value.to_ascii_lowercase();
            if v == "flex" {
                let d = Declaration { name: DeclarationName::Ident(swc_core::css::ast::Ident { value: prop.into(), raw: None, span: Default::default() }), value: vec![make_ident("-webkit-flex")], important: None, span: Default::default() };
                out.push(d);
                let d2 = Declaration { name: DeclarationName::Ident(swc_core::css::ast::Ident { value: prop.into(), raw: None, span: Default::default() }), value: vec![make_ident("-ms-flexbox")], important: None, span: Default::default() };
                out.push(d2);
            } else if v == "inline-flex" {
                let d = Declaration { name: DeclarationName::Ident(swc_core::css::ast::Ident { value: prop.into(), raw: None, span: Default::default() }), value: vec![make_ident("-webkit-inline-flex")], important: None, span: Default::default() };
                out.push(d);
                let d2 = Declaration { name: DeclarationName::Ident(swc_core::css::ast::Ident { value: prop.into(), raw: None, span: Default::default() }), value: vec![make_ident("-ms-inline-flexbox")], important: None, span: Default::default() };
                out.push(d2);
            }
        }
    }

    // TODO: Full value-level prefixing (gradients, grid, imageset, cross-fade, etc.)
    let _ = add; // silence unused parameter until full port lands
    out
}

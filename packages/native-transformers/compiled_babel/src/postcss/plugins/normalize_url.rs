use super::super::transform::{Plugin, TransformContext};
use crate::postcss::plugins::expand_shorthands::types::{
    parse_value_to_components, serialize_component_values,
};
use crate::postcss::value_parser as vp;
use once_cell::sync::Lazy;
use regex::Regex;
use swc_core::css::ast::{
    AtRuleName, AtRulePrelude, ComponentValue, NamespacePrelude, NamespacePreludeUri, Rule, Str,
    Stylesheet, Url as AstUrl, UrlValue,
};
use url::Url;

fn is_absolute(url: &str) -> bool {
    static ABS: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-zA-Z][a-zA-Z\d+\-.]*?:").unwrap());
    static WINDOWS: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[a-zA-Z]:\\\\|^[a-zA-Z]:/").unwrap());

    if WINDOWS.is_match(url) {
        return false;
    }
    ABS.is_match(url)
}

#[derive(Clone)]
struct NormalizeOptions {
    default_protocol: String,
    normalize_protocol: bool,
    force_http: bool,
    force_https: bool,
    strip_authentication: bool,
    strip_hash: bool,
    strip_text_fragment: bool,
    strip_www: bool,
    remove_query_parameters_default: bool,
    remove_query_parameters_all: bool,
    remove_trailing_slash: bool,
    remove_single_slash: bool,
    remove_directory_index: bool,
    sort_query_parameters: bool,
    strip_protocol: bool,
}

impl Default for NormalizeOptions {
    fn default() -> Self {
        Self {
            default_protocol: "http:".to_string(),
            normalize_protocol: true,
            force_http: false,
            force_https: false,
            strip_authentication: true,
            strip_hash: false,
            strip_text_fragment: true,
            strip_www: true,
            remove_query_parameters_default: true,
            remove_query_parameters_all: false,
            remove_trailing_slash: true,
            remove_single_slash: true,
            remove_directory_index: false,
            sort_query_parameters: true,
            strip_protocol: false,
        }
    }
}

fn plugin_default_options() -> NormalizeOptions {
    let mut o = NormalizeOptions::default();
    o.normalize_protocol = false;
    o.sort_query_parameters = false;
    o.strip_hash = false;
    o.strip_www = false;
    o.strip_text_fragment = false;
    o
}

fn normalize_data_url(url: &str, strip_hash: bool) -> Result<String, ()> {
    let re =
        Regex::new(r"(?i)^data:(?P<type>[^,]*?),(?P<data>[^#]*?)(?:#(?P<hash>.*))?$").unwrap();
    let caps = re.captures(url).ok_or(())?;
    let typ = caps.name("type").map(|m| m.as_str()).unwrap_or("").to_string();
    let data = caps.name("data").map(|m| m.as_str()).unwrap_or("").to_string();
    let mut hash = caps
        .name("hash")
        .map(|m| m.as_str())
        .unwrap_or("")
        .to_string();
    if strip_hash {
        hash.clear();
    }

    let mut media: Vec<String> = if typ.is_empty() {
        vec![]
    } else {
        typ.split(';').map(|s| s.to_string()).collect()
    };
    let mut is_base64 = false;
    if media.last().map(|s| s.as_str()) == Some("base64") {
        media.pop();
        is_base64 = true;
    }
    let mime_type = media
        .get(0)
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "".to_string());
    let mut attrs: Vec<String> = media
        .into_iter()
        .skip(1)
        .map(|attribute| {
            let mut parts = attribute.splitn(2, '=').map(|s| s.trim().to_string());
            let key = parts.next().unwrap_or_default();
            let mut value = parts.next().unwrap_or_default();
            if key == "charset" {
                value = value.to_lowercase();
                if value == "us-ascii" {
                    return String::new();
                }
            }
            if value.is_empty() {
                key
            } else {
                format!("{}={}", key, value)
            }
        })
        .filter(|s| !s.is_empty())
        .collect();
    if is_base64 {
        attrs.push("base64".to_string());
    }
    if !attrs.is_empty() || (!mime_type.is_empty() && mime_type != "text/plain") {
        attrs.insert(0, mime_type);
    }
    let hash_part = if hash.is_empty() {
        String::new()
    } else {
        format!("#{}", hash)
    };
    Ok(format!(
        "data:{},{}{}",
        attrs.join(";"),
        if is_base64 {
            data.trim().to_string()
        } else {
            data
        },
        hash_part
    ))
}

fn remove_duplicate_slashes_not_after_protocol(pathname: &str) -> String {
    let mut out = String::with_capacity(pathname.len());
    let mut prev_was_slash = false;
    let mut i = 0usize;
    while i < pathname.len() {
        let ch = pathname.as_bytes()[i] as char;
        if ch == '/' {
            if prev_was_slash {
                i += 1;
                continue;
            }
            prev_was_slash = true;
        } else {
            prev_was_slash = false;
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn collapse_dots(parts: &[&str], keep_root: bool) -> Vec<String> {
    let mut stack: Vec<String> = Vec::new();
    for part in parts {
        if part.is_empty() || *part == "." {
            continue;
        }
        if *part == ".." {
            if !stack.is_empty() && stack.last().map(|s| s.as_str()) != Some("..") {
                stack.pop();
            } else if !keep_root {
                stack.push("..".to_string());
            }
        } else {
            stack.push((*part).to_string());
        }
    }
    stack
}

fn remove_directory_index(path: &str) -> String {
    let default = Regex::new(r"(?i)^([\\s\\S]*?)index(?:\.[a-z]+)?$").unwrap();
    default.replace(path, "$1").into_owned()
}

fn normalize_url_impl(url: &str, opts: &NormalizeOptions) -> Result<String, ()> {
    if url.starts_with("data:") {
        return normalize_data_url(url, opts.strip_hash);
    }

    static UTM: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^utm_\w+").unwrap());
    static TEXT_FRAGMENT: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)(^|:)~:text=(?:[^&]+)").unwrap());

    let target = if url.starts_with("//") {
        format!("{}{}", opts.default_protocol, url.trim_start_matches("//"))
    } else {
        url.to_string()
    };

    let mut parsed = Url::parse(&target).map_err(|_| ())?;

    if opts.force_http && opts.force_https {
        return Err(());
    }
    if opts.force_http && parsed.scheme() == "https" {
        parsed.set_scheme("http").map_err(|_| ())?;
    }
    if opts.force_https && parsed.scheme() == "http" {
        parsed.set_scheme("https").map_err(|_| ())?;
    }

    if opts.strip_authentication {
        let _ = parsed.set_username("");
        let _ = parsed.set_password(None);
    }

    if opts.strip_hash {
        parsed.set_fragment(None);
    } else if opts.strip_text_fragment {
        if let Some(frag) = parsed.fragment() {
            if TEXT_FRAGMENT.is_match(frag) {
                parsed.set_fragment(None);
            }
        }
    }

    if opts.strip_www {
        if let Some(host) = parsed.host_str().map(|h| h.to_string()) {
            if let Some(stripped) = host.strip_prefix("www.") {
                let _ = parsed.set_host(Some(stripped));
            }
        }
    }

    let mut pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if opts.remove_query_parameters_all {
        pairs.clear();
    }
    if opts.remove_query_parameters_default {
        pairs.retain(|(k, _)| !UTM.is_match(k));
    }
    if opts.sort_query_parameters {
        pairs.sort();
    }

    if opts.remove_directory_index {
        let cleaned = remove_directory_index(parsed.path());
        parsed.set_path(&cleaned);
    }

    let mut path = parsed.path().to_string();
    if opts.remove_single_slash {
        while path.starts_with("//") {
            path.remove(0);
        }
    }
    if opts.remove_trailing_slash && path.len() > 1 {
        while path.ends_with('/') {
            path.pop();
        }
        if path.is_empty() {
            path.push('/');
        }
    }
    parsed.set_path(&path);

    let query = if pairs.is_empty() {
        None
    } else {
        Some(url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .finish())
    };
    parsed.set_query(query.as_deref());

    if opts.normalize_protocol {
        parsed
            .set_scheme(&parsed.scheme().to_lowercase())
            .map_err(|_| ())?;
    }

    Ok(parsed.to_string())
}

fn path_normalize_like_node(url: &str) -> String {
    let replaced = url.replace('\\', "/");
    let parts: Vec<&str> = replaced.split('/').collect();
    let collapsed = collapse_dots(&parts, replaced.starts_with('/'));
    let mut joined = collapsed.join("/");

    if replaced.starts_with('/') {
        joined.insert(0, '/');
    }

    let had_trailing_slash = replaced.ends_with('/');
    let mut cleaned = remove_duplicate_slashes_not_after_protocol(&joined);
    if cleaned.is_empty() {
        cleaned = if replaced.starts_with('/') {
            "/".to_string()
        } else {
            "".to_string()
        };
    }
    if had_trailing_slash && !cleaned.ends_with('/') {
        cleaned.push('/');
    }

    cleaned
}

fn convert(url: &str, opts: &NormalizeOptions) -> String {
    if is_absolute(url) || url.starts_with("//") {
        match normalize_url_impl(url, opts) {
            Ok(s) => s,
            Err(_) => url.to_string(),
        }
    } else {
        path_normalize_like_node(url)
    }
}

fn normalize_url_value(value: &str) -> String {
    static MULTILINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\\[\\r\\n]").unwrap());
    static ESCAPE_CHARS: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"([\\s\\(\\)"'])"#).unwrap());
    static DATA_URL: Lazy<Regex> = Lazy::new(|| Regex::new(r"^data:(.*)?,").unwrap());
    static EXT_SCHEME: Lazy<Regex> = Lazy::new(|| Regex::new(r"^.+-extension:/").unwrap());

    let opts = plugin_default_options();
    let mut nodes = vp::parse(value).nodes;

    vp::walk(
        &mut nodes[..],
        &mut |n| match n {
            vp::Node::Function {
                value,
                nodes: inner,
                before,
                after,
                ..
            } => {
                if !value.eq_ignore_ascii_case("url") {
                    return true;
                }

                *before = String::new();
                *after = String::new();

                if inner.is_empty() {
                    return true;
                }

                let mut skip = false;
                match &mut inner[0] {
                    vp::Node::String { value, quote, .. } => {
                        *value = value.trim().to_string();
                        *value = MULTILINE.replace_all(value, "").to_string();

                        if value.is_empty() {
                            *n = vp::Node::Word {
                                value: String::new(),
                            };
                            return true;
                        }

                        if value.starts_with('#') || DATA_URL.is_match(value) {
                            skip = true;
                        }

                        if !skip && !EXT_SCHEME.is_match(value) {
                        let next = convert(value, &opts);
                        if ESCAPE_CHARS.is_match(&next) {
                            let escaped = ESCAPE_CHARS.replace_all(&next, r"\\$1");
                            if escaped.len() < next.len() + 2 {
                                *n = vp::Node::Word {
                                    value: escaped.into_owned(),
                                    };
                                    return true;
                                }
                            }
                            *value = next;
                            *quote = '"';
                        }
                    }
                    vp::Node::Word { value } => {
                        *value = value.trim().to_string();
                        *value = MULTILINE.replace_all(value, "").to_string();

                        if value.is_empty() {
                            return true;
                        }

                        if value.starts_with('#') || DATA_URL.is_match(value) {
                            skip = true;
                        }

                        if !skip && !EXT_SCHEME.is_match(value) {
                            let next = convert(value, &opts);
                            if ESCAPE_CHARS.is_match(&next) {
                                let escaped = ESCAPE_CHARS.replace_all(&next, r"\\$1");
                                if escaped.len() < next.len() + 2 {
                                    *n = vp::Node::Word {
                                        value: escaped.into_owned(),
                                    };
                                    return true;
                                }
                            }
                            *value = next;
                        }
                    }
                    _ => {}
                }

                true
            }
            _ => true,
        },
        false,
    );

    vp::stringify(&nodes)
}

fn transform_namespace_prelude(prelude: &mut NamespacePrelude) {
    prelude.prefix = prelude.prefix.take().map(|mut ident| {
        ident.value = ident.value.trim().into();
        ident
    });

    match prelude.uri.as_mut() {
        NamespacePreludeUri::Url(AstUrl { value, .. }) => {
            if let Some(val) = value {
                match val.as_mut() {
                    UrlValue::Str(str_node) => {
                        str_node.value = str_node.value.trim().into();
                        str_node.raw = None;
                    }
                    UrlValue::Raw(raw) => {
                        let value = raw.value.as_ref().trim();
                        *prelude.uri = NamespacePreludeUri::Str(Str {
                            span: raw.span,
                            value: value.into(),
                            raw: Some("\"".into()),
                        })
                        .into();
                    }
                }
            }
        }
        NamespacePreludeUri::Str(str_node) => {
            str_node.value = str_node.value.trim().into();
            str_node.raw = None;
        }
    }
}

fn walk_components(values: &mut [ComponentValue]) {
    for value in values.iter_mut() {
        match value {
            ComponentValue::Declaration(decl) => {
                let Some(current) = serialize_component_values(&decl.value) else { continue; };
                let next = normalize_url_value(&current);
                if next != current {
                    decl.value = parse_value_to_components(&next);
                }
            }
            ComponentValue::QualifiedRule(rule) => walk_components(&mut rule.block.value),
            ComponentValue::AtRule(at) => {
                if let Some(block) = &mut at.block {
                    walk_components(&mut block.value);
                }
            }
            ComponentValue::SimpleBlock(block) => walk_components(&mut block.value),
            ComponentValue::ListOfComponentValues(list) => walk_components(&mut list.children),
            ComponentValue::Function(fun) => walk_components(&mut fun.value),
            ComponentValue::KeyframeBlock(block) => walk_components(&mut block.block.value),
            _ => {}
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NormalizeUrl;

pub fn normalize_url() -> NormalizeUrl {
    NormalizeUrl
}

impl Plugin for NormalizeUrl {
    fn name(&self) -> &'static str {
        "postcss-normalize-url"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        for rule in &mut stylesheet.rules {
            match rule {
                Rule::QualifiedRule(rule) => walk_components(&mut rule.block.value),
                Rule::AtRule(at) => {
                    if let Some(block) = &mut at.block {
                        walk_components(&mut block.value);
                    }
                    if matches!(&at.name, AtRuleName::Ident(ident) if ident.value.eq_ignore_ascii_case("namespace")) {
                        if let Some(prelude) = at.prelude.as_deref_mut() {
                            if let AtRulePrelude::NamespacePrelude(ns) = prelude {
                                transform_namespace_prelude(ns);
                            }
                        }
                    }
                }
                Rule::ListOfComponentValues(list) => walk_components(&mut list.children),
            }
        }
    }
}

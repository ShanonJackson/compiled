use once_cell::sync::Lazy;
use std::collections::HashMap;
use swc_core::ecma::ast::{CallExpr, Callee, Expr, Lit, MemberExpr, MemberProp};
use swc_design_system_tokens::generated::{
  LIGHT_VALUES, SHAPE_VALUES, SPACING_VALUES, TOKEN_NAMES, TYPOGRAPHY_VALUES,
};

static TOKEN_NAME_MAP: Lazy<HashMap<&'static str, &'static str>> =
  Lazy::new(|| TOKEN_NAMES.iter().copied().collect());

static TOKEN_FALLBACK_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
  let mut map = HashMap::new();
  for (name, value) in LIGHT_VALUES.iter() {
    map.insert(*name, *value);
  }
  for (name, value) in SHAPE_VALUES.iter() {
    map.insert(*name, *value);
  }
  for (name, value) in SPACING_VALUES.iter() {
    map.insert(*name, *value);
  }
  for (name, value) in TYPOGRAPHY_VALUES.iter() {
    map.insert(*name, *value);
  }
  map
});

pub fn resolve_token_expression(expr: &Expr) -> Option<String> {
  match expr {
    Expr::Call(call) => resolve_token_call(call),
    Expr::Member(member) => resolve_tokens_member(member),
    _ => None,
  }
}

fn resolve_token_call(call: &CallExpr) -> Option<String> {
  let callee_ident = match &call.callee {
    Callee::Expr(callee) => match &**callee {
      Expr::Ident(ident) => ident,
      _ => return None,
    },
    _ => return None,
  };

  if callee_ident.sym.as_ref() != "token" {
    return None;
  }

  let first_arg = call.args.get(0)?;
  let token_name = match &*first_arg.expr {
    Expr::Lit(Lit::Str(str_lit)) => str_lit.value.to_string(),
    _ => return None,
  };
  let css_token = TOKEN_NAME_MAP.get(token_name.as_str())?;

  let default_fallback = TOKEN_FALLBACK_MAP
    .get(token_name.as_str())
    .map(|value| value.to_string());

  let fallback = if let Some(value) = default_fallback {
    Some(value)
  } else if let Some(second) = call.args.get(1) {
    match &*second.expr {
      Expr::Lit(Lit::Str(lit)) => {
        let value = lit.value.to_string();
        if value.is_empty() { None } else { Some(value) }
      }
      _ => None,
    }
  } else {
    None
  };

  let css = match fallback {
    Some(fallback_value) => format!("var({}, {})", css_token, fallback_value),
    None => format!("var({})", css_token),
  };
  if std::env::var_os("COMPILED_DEBUG_TOKENS").is_some() {
    eprintln!("[compiled-token] expr='{}' -> '{}'", token_name, css);
  }
  Some(css)
}

fn resolve_tokens_member(member: &MemberExpr) -> Option<String> {
  let obj_ident = match &*member.obj {
    Expr::Ident(ident) => ident,
    _ => return None,
  };

  if obj_ident.sym.as_ref() != "Tokens" {
    return None;
  }

  let prop_name = match &member.prop {
    MemberProp::Ident(ident) => ident.sym.as_ref().to_string(),
    MemberProp::Computed(comp) => match &*comp.expr {
      Expr::Lit(Lit::Str(str_lit)) => str_lit.value.as_ref().to_string(),
      _ => return None,
    },
    _ => return None,
  };

  let token_path = prop_name.to_ascii_lowercase().replace('_', ".");
  let css_token = TOKEN_NAME_MAP.get(token_path.as_str())?;
  Some(format!("var({})", css_token))
}

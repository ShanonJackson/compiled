use crate::postcss::value_parser as vp;
use postcss as pc;

fn normalize_layer(raw: &str) -> String {
  let parsed = vp::parse(raw);
  let mut words: Vec<String> = Vec::new();
  for n in parsed.nodes {
    match n {
      vp::Node::Space { .. } => continue,
      vp::Node::Word { value } => words.push(value.to_ascii_lowercase()),
      _ => {
        // Bail out on anything we don't handle (functions, numbers, etc.)
        return raw.to_string();
      }
    }
  }

  if words.is_empty() {
    return raw.to_string();
  }

  // Mappings mirror postcss-normalize-positions.
  let horiz = |w: &str| match w {
    "left" => Some("0"),
    "right" => Some("100%"),
    "center" => Some("50%"),
    _ => None,
  };
  let vert = |w: &str| match w {
    "top" => Some("0"),
    "bottom" => Some("100%"),
    "center" => Some("50%"),
    _ => None,
  };

  let mut out: Option<String> = None;
  if words.len() == 1 {
    if let Some(h) = horiz(&words[0]) {
      out = Some(h.to_string());
    }
  } else if words.len() == 2 {
    let first = words[0].as_str();
    let second = words[1].as_str();
    if second == "center" {
      if let Some(h) = horiz(first) {
        out = Some(h.to_string());
      }
    } else if first == "center" {
      if let Some(h) = horiz(second) {
        out = Some(h.to_string());
      }
    } else if let (Some(h), Some(v)) = (horiz(first), vert(second)) {
      out = Some(format!("{h} {v}"));
    } else if let (Some(v), Some(h)) = (vert(first), horiz(second)) {
      out = Some(format!("{h} {v}"));
    }
  }

  out.unwrap_or_else(|| raw.to_string())
}

pub fn plugin() -> pc::BuiltPlugin {
  pc::plugin("postcss-normalize-positions")
    .decl_filter("background-position", |decl, _| {
      let current = decl.value();
      // Split on commas to handle multiple backgrounds; normalize each.
      let parts: Vec<String> = current
        .split(',')
        .map(|p| normalize_layer(p.trim()))
        .collect();
      let next = parts.join(",");
      if next != current {
        decl.set_value(next);
      }
      Ok(())
    })
    .build()
}

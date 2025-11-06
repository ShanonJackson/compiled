use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::HashMap;

static FROM_INITIAL: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let raw: Value =
        serde_json::from_str(include_str!("data/from_initial.json")).expect("valid JSON");
    let mut map = HashMap::new();

    let Value::Object(entries) = raw else {
        return map;
    };

    for (key, value) in entries {
        if let Value::String(value) = value {
            let key = key.into_boxed_str();
            let value = value.into_boxed_str();
            map.insert(Box::leak(key), Box::leak(value));
        }
    }

    map
});

pub fn from_initial(property: &str) -> Option<&'static str> {
    if property.bytes().any(|byte| byte.is_ascii_uppercase()) {
        let lowered = property.to_ascii_lowercase();
        FROM_INITIAL.get(lowered.as_str()).copied()
    } else {
        FROM_INITIAL.get(property).copied()
    }
}

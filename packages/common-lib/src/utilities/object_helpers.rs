use serde_json::{Map, Value};

/// Returns a new object containing only the entries of `obj` whose key is in
/// `keys`. Keys absent from `obj` are skipped.
pub fn pick(obj: &Map<String, Value>, keys: &[&str]) -> Map<String, Value> {
    keys.iter()
        .filter_map(|&key| obj.get_key_value(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// Returns a new object containing only the entries of `obj` whose value
/// satisfies `condition`.
pub fn filter(obj: &Map<String, Value>, condition: impl Fn(&Value) -> bool) -> Map<String, Value> {
    obj.iter()
        .filter(|(_, value)| condition(value))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// Returns `true` when `obj` has no entries.
pub fn is_empty(obj: &Map<String, Value>) -> bool {
    obj.is_empty()
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::{filter, is_empty, pick};

    fn object(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            other => panic!("expected an object, got {other}"),
        }
    }

    #[test]
    fn pick_selects_requested_keys() {
        let obj = object(json!({ "a": 1, "b": "two", "c": null }));
        let picked = pick(&obj, &["a", "c"]);
        assert_eq!(Value::Object(picked), json!({ "a": 1, "c": null }));
    }

    #[test]
    fn pick_skips_missing_keys() {
        let obj = object(json!({ "a": 1 }));
        let picked = pick(&obj, &["a", "missing"]);
        assert_eq!(Value::Object(picked), json!({ "a": 1 }));
    }

    #[test]
    fn filter_keeps_matching_values() {
        let obj = object(json!({ "a": 1, "b": "two", "c": 3 }));
        let filtered = filter(&obj, Value::is_number);
        assert_eq!(Value::Object(filtered), json!({ "a": 1, "c": 3 }));
    }

    #[test]
    fn filter_can_remove_everything() {
        let obj = object(json!({ "a": 1 }));
        assert!(filter(&obj, |_| false).is_empty());
    }

    #[test]
    fn is_empty_reports_entry_count() {
        assert!(is_empty(&Map::new()));
        assert!(!is_empty(&object(json!({ "a": 1 }))));
    }
}

use serde_yaml_ng::Value;

pub fn get_nested(data: &Value, path: &[&str]) -> Option<Value> {
    todo!()
}

pub fn get_nested_with_default(data: &Value, path: &[&str], default: Value) -> Value {
    todo!()
}

pub fn set_nested(data: &mut Value, path: &[&str], value: Value) {
    todo!()
}

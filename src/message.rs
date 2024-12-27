use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum ParsedMessage {
    Update(Update),
    Control(Control),
}

#[allow(dead_code)]
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum UpdateType {
    Insert,
    Delete,
    Update,
}

#[allow(dead_code)]
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Update {
    pub r#type: UpdateType,
    pub line: usize,
    pub column: usize,
    pub character: Option<char>,
}

#[allow(dead_code)]
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Control {
    message: String,
}

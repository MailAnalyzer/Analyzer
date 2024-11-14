use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Entity {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    #[serde(rename = "information")]
    pub additional_info: Vec<AdditionalInfo>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdditionalInfo {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String
}

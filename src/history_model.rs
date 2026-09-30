use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryNode {
    pub id: String,
    pub title: String,
    pub kind: i32,
    pub types: Vec<String>,
    pub tags: Vec<String>,
    pub topic: String,
    pub notes: String,
    pub raw: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryLink {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: i32,
    pub meaning: i32,
    pub direction: i32,
    pub name: String,
    pub kind: String,
    pub meaning_label: String,
    pub direction_label: String,
    pub raw: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HistoryAttachment {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub status: String,
    pub location: String,
    pub text: String,
    pub asset: Option<String>,
    pub raw: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDataset {
    pub schema_version: u32,
    pub version: String,
    pub brain_id: String,
    pub root_id: String,
    pub nodes: Vec<HistoryNode>,
    pub links: Vec<HistoryLink>,
    pub attachments: Vec<HistoryAttachment>,
    pub quality: Value,
    pub meta: Value,
}

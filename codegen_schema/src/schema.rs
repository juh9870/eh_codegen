#![allow(dead_code)]
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SchemaVersion {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@major")]
    major: String,
    #[serde(rename = "@minor")]
    minor: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaItem {
    Schema { version: SchemaVersion },
    Data(SchemaData),
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchemaData {
    #[serde(rename = "@type")]
    pub ty: SchemaDataType,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@switch")]
    pub switch: Option<String>,
    #[serde(rename = "@typeid")]
    pub typeid: Option<String>,
    pub member: Option<Vec<SchemaStructMember>>,
    pub param: Option<Vec<SchemaExpressionParam>>,
    pub item: Option<Vec<SchemaEnumItem>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaDataType {
    Enum,
    Expression,
    Struct,
    Settings,
    Object,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchemaStructMember {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@type")]
    pub ty: SchemaStructMemberType,
    #[serde(rename = "@minvalue")]
    pub minvalue: Option<f32>,
    #[serde(rename = "@maxvalue")]
    pub maxvalue: Option<f32>,
    #[serde(rename = "@typeid")]
    pub typeid: Option<String>,
    #[serde(rename = "@options")]
    pub options: Option<String>,
    #[serde(rename = "@case")]
    pub case: Option<String>,
    #[serde(rename = "@alias")]
    pub alias: Option<String>,
    #[serde(rename = "@default")]
    pub default: Option<String>,
    #[serde(rename = "@arguments")]
    pub arguments: Option<String>,
    #[serde(rename = "$value")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaStructMemberType {
    Struct,
    StructList,
    Object,
    ObjectList,
    Enum,
    EnumFlags,
    Expression,
    Vector,
    Float,
    Int,
    Color,
    Bool,
    String,
    Image,
    #[serde(rename = "audioclip")]
    AudioClip,
    Prefab,
    Layout,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchemaExpressionParam {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@type")]
    pub ty: SchemaExpressionParamType,
    #[serde(rename = "@typeid")]
    pub typeid: Option<String>,
    #[serde(rename = "$value")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaExpressionParamType {
    Float,
    Int,
    Enum,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchemaEnumItem {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@value")]
    pub value: Option<String>,
    #[serde(rename = "$value")]
    pub description: Option<String>,
}

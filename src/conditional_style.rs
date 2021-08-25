use crate::context::Context;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct StarshipConditionalStyle<'a> {
    pub env: Option<&'a str>,
    pub equals: Option<&'a str>,
    pub value: &'a str,
}

impl<'a> Default for StarshipConditionalStyle<'a> {
    fn default() -> Self {
        StarshipConditionalStyle {
            env: None,
            equals: None,
            value: "",
        }
    }
}

impl<'a> From<&'a str> for StarshipConditionalStyle<'a> {
    fn from(value: &'a str) -> Self {
        StarshipConditionalStyle {
            env: None,
            equals: None,
            value,
        }
    }
}

impl<'a> From<&'a toml::value::Table> for StarshipConditionalStyle<'a> {
    fn from(value: &'a toml::value::Table) -> Self {
        let get_value = |key: &str| value.get(key)?.as_str();

        StarshipConditionalStyle {
            env: get_value("env"),
            equals: get_value("equals"),
            value: value
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        }
    }
}

pub fn get_style<'a>(context: &Context, items: &Vec<StarshipConditionalStyle<'a>>) -> &'a str {
    items.iter().for_each(|s| log::warn!("{:?}", s));
    "red"
}

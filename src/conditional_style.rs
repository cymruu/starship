use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct StarshipConditionalStyle<'a> {
    pub env: &'a str,
    pub equals: &'a str,
    pub value: &'a str,
}

impl<'a> Default for StarshipConditionalStyle<'a> {
    fn default() -> Self {
        StarshipConditionalStyle {
            env: "",
            equals: "",
            value: "",
        }
    }
}

impl<'a> From<&'a str> for StarshipConditionalStyle<'a> {
    fn from(value: &'a str) -> Self {
        StarshipConditionalStyle {
            env: "",
            equals: "",
            value,
        }
    }
}

impl<'a> From<&'a toml::value::Table> for StarshipConditionalStyle<'a> {
    fn from(value: &'a toml::value::Table) -> Self {
        let get_value = |key: &str| value.get(key).and_then(|v| v.as_str()).unwrap_or_default();

        StarshipConditionalStyle {
            env: get_value("env"),
            equals: get_value("equals"),
            value: get_value("value"),
        }
    }
}

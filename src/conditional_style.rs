use crate::context::Context;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum StarshipConditionalOperator {
    Equal,
}

impl TryFrom<&toml::value::Value> for StarshipConditionalOperator {
    type Error = &'static str;

    fn try_from(value: &toml::value::Value) -> Result<Self, Self::Error> {
        match value.as_str() {
            Some(str_val) => match str_val {
                "equal" => Ok(Self::Equal),
                _ => Err("Invalid value for operator"),
            },
            None => Err("Operator value is not a string"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct StarshipConditionalStyle<'a> {
    pub env: Option<&'a str>,
    pub operator: Option<StarshipConditionalOperator>,
    pub equals: Option<&'a str>,
    pub value: &'a str,
}

impl<'a> StarshipConditionalStyle<'a> {
    fn should_display(&self, context: &Context) -> bool {
        match self.env {
            Some(env_variable) => match self.equals {
                Some(_) => {
                    let env_variable_value = context.get_env(env_variable);
                    env_variable_value.as_deref() == self.equals
                }
                None => true,
            },
            None => false,
        }
    }
}

impl<'a> Default for StarshipConditionalStyle<'a> {
    fn default() -> Self {
        StarshipConditionalStyle {
            env: None,
            operator: None,
            equals: None,
            value: "",
        }
    }
}

impl<'a> From<&'a str> for StarshipConditionalStyle<'a> {
    fn from(value: &'a str) -> Self {
        StarshipConditionalStyle {
            env: None,
            operator: None,
            equals: None,
            value,
        }
    }
}

impl<'a> From<&'a toml::value::Table> for StarshipConditionalStyle<'a> {
    fn from(value: &'a toml::value::Table) -> Self {
        let get_value = |key: &str| value.get(key)?.as_str();
        let operator = match value.get("operator") {
            Some(v) => StarshipConditionalOperator::try_from(v).ok(),
            None => None,
        };

        StarshipConditionalStyle {
            env: get_value("env"),
            operator,
            equals: get_value("equals"),
            value: value
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        }
    }
}

pub fn get_style<'a>(context: &Context, items: &[StarshipConditionalStyle<'a>]) -> &'a str {
    let found = items.iter().find(|s| s.should_display(context));

    if let Some(v) = found {
        v.value
    } else if let Some(last) = items.iter().last() {
        last.value
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{Context, Shell};
    use std::path::PathBuf;

    fn create_context<'a>() -> Context<'a> {
        Context::new_with_shell_and_path(
            clap::ArgMatches::default(),
            Shell::Unknown,
            PathBuf::new(),
            PathBuf::new(),
        )
    }

    #[test]
    fn should_display_env_set_matching() {
        let style = StarshipConditionalStyle {
            env: Some("env"),
            operator: None,
            equals: Some("value"),
            value: "",
        };
        let mut context = create_context();
        context.env.insert("env", "value".into());

        assert!(style.should_display(&context));
    }

    #[test]
    fn should_display_if_env_is_set_and_equals_is_none() {
        let style = StarshipConditionalStyle {
            env: Some("env"),
            operator: None,
            equals: None,
            value: "",
        };
        let mut context = create_context();
        context.env.insert("env", "value".into());

        assert!(style.should_display(&context));
    }

    #[test]
    fn should_not_display_if_not_equal() {
        let style = StarshipConditionalStyle {
            env: Some("env"),
            operator: None,
            equals: Some("different"),
            value: "",
        };
        let mut context = create_context();
        context.env.insert("env", "value".into());

        assert!(!style.should_display(&context));
    }

    #[test]
    fn get_style_fallback() {
        let context = create_context();
        let items: Vec<StarshipConditionalStyle> = vec![];
        assert_eq!(get_style(&context, &items), "");
    }

    #[test]
    fn get_style_no_match() {
        let context = create_context();
        let items: Vec<StarshipConditionalStyle> = vec![
            StarshipConditionalStyle::default(),
            StarshipConditionalStyle::from("style"),
        ];
        assert_eq!(get_style(&context, &items), "style");
    }

    #[test]
    fn get_style_match() {
        let mut context = create_context();
        context.env.insert("env", "value".into());
        let items: Vec<StarshipConditionalStyle> = vec![
            StarshipConditionalStyle {
                env: Some("env"),
                operator: None,
                equals: Some("value"),
                value: "red",
            },
            StarshipConditionalStyle::from("style"),
        ];
        assert_eq!(get_style(&context, &items), "red");
    }
}

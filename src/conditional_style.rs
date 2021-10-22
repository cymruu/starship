use crate::context::Context;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum StarshipConditionalOperator {
    Equal,
    Exists,
}

impl StarshipConditionalOperator {
    fn should_display(
        &self,
        parameter_left: Option<String>,
        parameter_right: Option<&str>,
    ) -> bool {
        match self {
            StarshipConditionalOperator::Equal => parameter_left.as_deref() == parameter_right,
            StarshipConditionalOperator::Exists => parameter_left.is_some(),
        }
    }
}

impl TryFrom<&toml::value::Value> for StarshipConditionalOperator {
    type Error = &'static str;

    fn try_from(value: &toml::value::Value) -> Result<Self, Self::Error> {
        match value.as_str() {
            Some(str_val) => match str_val.to_lowercase().as_str() {
                "equal" => Ok(Self::Equal),
                "exists" => Ok(Self::Exists),
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
        let env_value = if let Some(env_variable_name) = self.env {
            context.get_env(env_variable_name)
        } else {
            None
        };

        match &self.operator {
            Some(operator) => operator.should_display(env_value, self.equals),
            None => true, //display if element has no operator
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
        let operator = if let Some(value) = value.get("operator") {
            StarshipConditionalOperator::try_from(value).ok()
        } else {
            None
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
    fn conditional_style_should_apply_if_operator_matches() {
        let style = StarshipConditionalStyle {
            env: Some("env"),
            operator: Some(StarshipConditionalOperator::Equal),
            equals: Some("value"),
            value: "",
        };
        let mut context = create_context();
        context.env.insert("env", "value".into());

        assert!(style.should_display(&context));
    }

    #[test]
    fn should_not_display_if_operator_doesnt_match() {
        let style = StarshipConditionalStyle {
            env: Some("env"),
            operator: Some(StarshipConditionalOperator::Equal),
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
            StarshipConditionalStyle {
                env: Some("env"),
                operator: Some(StarshipConditionalOperator::Equal),
                equals: Some("value"),
                value: "red",
            },
            StarshipConditionalStyle::from("style"),
        ];
        assert_eq!(get_style(&context, &items), "style");
    }

    #[test]
    fn conditional_style_exists_operator() {
        let items: Vec<StarshipConditionalStyle> = vec![
            StarshipConditionalStyle {
                env: Some("env"),
                operator: Some(StarshipConditionalOperator::Exists),
                equals: None,
                value: "red",
            },
            StarshipConditionalStyle::from("style"),
        ];
        let mut context = create_context();
        assert_eq!(get_style(&context, &items), "style");
        context.env.insert("env", "value".into());
        assert_eq!(get_style(&context, &items), "red");
    }
}

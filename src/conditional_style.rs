use crate::context::Context;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct StarshipConditionalStyle<'a> {
    pub env: Option<&'a str>,
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
    let found = items.iter().find(|s| {
        log::warn!("{:?} {}", s, s.should_display(context));
        s.should_display(context)
    });

    if let Some(v) = found {
        v.value
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use crate::conditional_style::StarshipConditionalStyle;
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
            equals: Some("value"),
            value: "",
        };
        let mut context = create_context();
        context.env.insert("env", "value".into());

        assert_eq!(style.should_display(&context), true);
    }
    #[test]
    fn should_display_if_env_is_set_and_equals_is_none() {
        let style = StarshipConditionalStyle {
            env: Some("env"),
            equals: None,
            value: "",
        };
        let mut context = create_context();
        context.env.insert("env", "value".into());

        assert_eq!(style.should_display(&context), true);
    }
    #[test]
    fn should_not_display_if_not_equal() {
        let style = StarshipConditionalStyle {
            env: Some("env"),
            equals: Some("different"),
            value: "",
        };
        let mut context = create_context();
        context.env.insert("env", "value".into());

        assert_eq!(style.should_display(&context), false);
    }
}

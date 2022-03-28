use crate::context::Context;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum StarshipConditionalStyleOperator {
    Equal,
    Exists,
}

impl StarshipConditionalStyleOperator {
    fn invoke(&self, left_hand_side: Option<String>, right_hand_side: Option<String>) -> bool {
        match self {
            StarshipConditionalStyleOperator::Equal => left_hand_side == right_hand_side,
            StarshipConditionalStyleOperator::Exists => left_hand_side.is_some(),
        }
    }
}

#[derive(Clone, Serialize, Debug, PartialEq, Default)]
pub struct StarshipConditionalStyle<'a> {
    pub env: Option<&'a str>,
    pub operator: Option<StarshipConditionalStyleOperator>,
    pub expected_value: Option<&'a str>,
    pub style: &'a str,
}

impl<'a> StarshipConditionalStyle<'a> {
    fn should_apply(&self, context: &Context) -> bool {
        let env_value = self.env.and_then(|x| context.get_env(x));

        match &self.operator {
            Some(operator) => operator.invoke(env_value, self.expected_value.map(String::from)),
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{Context, Shell, Target};
    use std::path::PathBuf;

    fn create_context<'a>() -> Context<'a> {
        Context::new_with_shell_and_path(
            Default::default(),
            Shell::Unknown,
            Target::Main,
            PathBuf::new(),
            PathBuf::new(),
        )
    }

    #[test]
    fn test_exists_operator() {
        let mut context = create_context();
        let style = StarshipConditionalStyle {
            env: Some("test"),
            operator: Some(StarshipConditionalStyleOperator::Exists),
            expected_value: None,
            style: "",
        };
        assert_eq!(style.should_apply(&context), false);
        context.env.insert("test", String::default());
        assert_eq!(style.should_apply(&context), true);
    }

    #[test]
    fn test_equal_operator() {
        let mut context = create_context();
        let style = StarshipConditionalStyle {
            env: Some("test"),
            operator: Some(StarshipConditionalStyleOperator::Equal),
            expected_value: Some("expected"),
            style: "",
        };
        assert_eq!(style.should_apply(&context), false);
        context.env.insert("test", String::from("not_expected"));
        assert_eq!(style.should_apply(&context), false);
        context.env.insert("test", String::from("expected"));
        assert_eq!(style.should_apply(&context), true);
    }
}

use std::{fmt, str::FromStr};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FlowName(String);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct InstanceName(String);

macro_rules! impl_name {
    ($name:ident, $kind:literal) => {
        impl $name {
            pub(crate) fn placeholder() -> Self {
                Self("placeholder".to_owned())
            }

            pub fn parse(value: impl Into<String>) -> Result<Self, String> {
                let value = value.into();
                let mut characters = value.chars();
                let valid_first = characters.next().is_some_and(|c| c.is_ascii_alphanumeric());
                if value.contains("..")
                    || !valid_first
                    || !characters.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                {
                    return Err(format!(
                        "invalid {} name {:?}: use ASCII letters, digits, '-', '_' or '.', starting with a letter or digit, without consecutive dots",
                        $kind, value
                    ));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.as_str() == other
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.as_str() == *other
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

impl_name!(FlowName, "flow");
impl_name!(InstanceName, "instance");

#[cfg(test)]
mod tests {
    use super::{FlowName, InstanceName};

    #[test]
    fn accepts_safe_slugs() {
        for value in ["workflow", "a-b_c.d", "0start"] {
            assert_eq!(FlowName::parse(value).unwrap().as_str(), value);
            assert_eq!(InstanceName::parse(value).unwrap().as_str(), value);
        }
    }

    #[test]
    fn rejects_invalid_slugs() {
        for value in [
            "", "-start", "_start", ".", "..", "a/b", "a\\b", "a b", "é", "a..b",
        ] {
            assert!(FlowName::parse(value).is_err());
            assert!(InstanceName::parse(value).is_err());
        }
    }
}

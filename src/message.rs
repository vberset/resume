use std::fmt;
use std::fmt::Formatter;
use std::str::FromStr;

use pest::iterators::Pairs;
use pest::Parser;
use pest_derive::Parser;
use serde::Serialize;

#[derive(Debug, Eq, PartialEq, Clone, Hash, Serialize)]
pub struct CommitScope(String);

impl CommitScope {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for CommitScope {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl FromStr for CommitScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_owned()))
    }
}

impl AsRef<str> for CommitScope {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl fmt::Display for CommitScope {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// Parsed commit message following [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
/// convention.
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct ConventionalMessage {
    pub ctype: CommitType,
    pub scope: Option<CommitScope>,
    pub is_breaking: bool,
    pub summary: String,
    pub body: Option<String>,
    pub trailers: Vec<(String, String)>,
}

#[derive(Debug, Eq, PartialEq, Clone, Hash, Serialize)]
pub enum CommitType {
    ContinuousIntegration,
    Build,
    BugFix,
    Documentation,
    Feature,
    Performance,
    Refactoring,
    Style,
    Test,
    Other(String),
}

/// PEG parser based on Pest definition
#[derive(Parser)]
#[grammar = "conventional_message.pest"]
struct ConventionalMessageParser;

impl FromStr for ConventionalMessage {
    type Err = pest::error::Error<Rule>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parser = ConventionalMessageParser::parse(Rule::message, s)?;
        let mut message = ConventionalMessage {
            ctype: CommitType::Other("".to_owned()),
            scope: None,
            is_breaking: false,
            summary: "".to_string(),
            body: None,
            trailers: vec![],
        };

        let pairs = parser.next().unwrap().into_inner();

        // Parse headline/body/trailers
        for pair in pairs.clone() {
            match pair.as_rule() {
                Rule::headline => {
                    let pairs = pair.clone().into_inner();

                    // Parse ctype/scope/break_mark/summary
                    for pair in pairs {
                        match pair.as_rule() {
                            Rule::ctype => {
                                message.ctype = pair.as_str().parse().expect("unfailable")
                            }
                            Rule::scope => {
                                message.scope = Some(pair.as_str().parse().expect("unfailable"))
                            }
                            Rule::summary => message.summary = pair.as_str().to_owned(),
                            Rule::break_mark => message.is_breaking = true,
                            _ => unreachable!(),
                        }
                    }
                }
                Rule::body => message.body = Some(pair.as_str().trim().to_owned()),
                Rule::trailers => message.trailers = parse_trailers(pair.clone().into_inner()),
                _ => unreachable!(),
            }
        }

        Ok(message)
    }
}

fn parse_trailers(pairs: Pairs<Rule>) -> Vec<(String, String)> {
    let mut trailers = Vec::new();
    for pair in pairs {
        if pair.as_rule() == Rule::EOI {
            break;
        }

        let mut pairs = pair.clone().into_inner();
        let token = pairs
            .next()
            .expect("broken parser: MUST have token")
            .as_str()
            .trim()
            .to_owned();
        let value = pairs
            .next()
            .expect("broken parser: MUST have value")
            .as_str()
            .trim()
            .to_owned();
        trailers.push((token, value));
    }
    trailers
}

impl CommitType {
    pub fn as_str(&self) -> &str {
        match self {
            CommitType::ContinuousIntegration => "ci",
            CommitType::Build => "build",
            CommitType::BugFix => "fix",
            CommitType::Documentation => "docs",
            CommitType::Feature => "feat",
            CommitType::Performance => "perf",
            CommitType::Refactoring => "refactor",
            CommitType::Style => "style",
            CommitType::Test => "test",
            CommitType::Other(s) => s.as_str(),
        }
    }
}

impl FromStr for CommitType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "build" => Self::Build,
            "ci" => Self::ContinuousIntegration,
            "docs" => Self::Documentation,
            "feat" => Self::Feature,
            "fix" => Self::BugFix,
            "perf" => Self::Performance,
            "refactor" => Self::Refactoring,
            "style" => Self::Style,
            "test" => Self::Test,
            s => Self::Other(s.to_owned()),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_simple_message() {
        let expected = ConventionalMessage {
            ctype: CommitType::Feature,
            scope: None,
            is_breaking: false,
            summary: "new feature".to_string(),
            body: None,
            trailers: vec![],
        };

        let input = format!("feat: {}", &expected.summary);
        let message = input.parse().unwrap();
        assert_eq!(expected, message);
    }

    #[test]
    fn test_parse_message_with_trailers() {
        let expected = ConventionalMessage {
            ctype: CommitType::Feature,
            scope: None,
            is_breaking: false,
            summary: "new feature".to_string(),
            body: None,
            trailers: vec![
                ("Team".to_string(), "X functional".to_string()),
                ("foo".to_string(), "bar metal".to_string()),
            ],
        };

        let input = format!(
            "feat: {}\n\n{}: {}\n{}: {}",
            &expected.summary,
            &expected.trailers[0].0,
            &expected.trailers[0].1,
            &expected.trailers[1].0,
            &expected.trailers[1].1,
        );
        let message = input.parse().unwrap();
        assert_eq!(expected, message);
    }

    #[test]
    fn test_parse_message_with_all_syntaxes() {
        let expected = ConventionalMessage {
            ctype: CommitType::BugFix,
            scope: Some("scope".parse().unwrap()),
            is_breaking: true,
            summary: "the summary".to_string(),
            body: Some("Some body content\n\n\nmultiple\nlines\nblock".to_string()),
            trailers: vec![("Key".to_string(), "Value".to_string())],
        };

        let input = format!(
            "fix({})!: {}\n\n{}\n\n{}: {} \n",
            expected.scope.as_ref().unwrap(),
            &expected.summary,
            expected.body.as_ref().unwrap(),
            &expected.trailers[0].0,
            &expected.trailers[0].1,
        );

        let message = input.parse().unwrap();
        assert_eq!(expected, message);
    }
}

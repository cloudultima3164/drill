use std::convert::TryFrom;

use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
  Include,
  Assert,
  Assign,
  DbQuery,
  Delay,
  Exec,
  Request,
}

impl TryFrom<&str> for Op {
  type Error = ();
  fn try_from(value: &str) -> Result<Self, Self::Error> {
    serde_json::from_value(serde_json::Value::String(
      value.to_owned(),
    ))
    .map_err(|_| ())
  }
}

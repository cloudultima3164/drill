use std::convert::TryFrom;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
  Include,
  Assert,
  Assign,
  // DbQuery,
  Delay,
  Exec,
  Request,
}

impl TryFrom<&str> for Op {
  type Error = ();
  fn try_from(value: &str) -> Result<Self, Self::Error> {
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
      .map_err(|_| ())
  }
}

// pub enum OpParser<'a> {
//   Include(Parser<'a, IncludeParser<'a>, IncludeArgs>),
//   Assert(Parser<'a, AssertParser, AssertArgs>),
//   Assign(Parser<'a, AssignParser, AssignArgs>),
//   // DbQuery,
//   Delay(Parser<'a, DelayParser, DelayArgs>),
//   Exec(Parser<'a, ExecParser, ExecArgs>),
//   Request(Parser<'a, RequestParser, RequestArgs>),
// }

// impl<'a> OpParser<'a> {

// }

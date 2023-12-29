use std::{marker::PhantomData, path::Path};

use yaml_rust::Yaml;

use super::{Parse, ParserArgs};
use crate::{
  actions::{Assert, Assign, Delay, Exec, Request},
  benchmark::Benchmark,
  tags::Tags,
};

use super::walk;

macro_rules! impl_action_parser {
  ($parser:ident, $action:ident, $args:ident) => {
    pub struct $parser;

    pub struct $args {
      name: String,
    }

    impl<'a> From<ParserArgs<'a>> for $args {
      fn from(args: ParserArgs<'a>) -> Self {
        $args {
          name: args.name,
        }
      }
    }

    impl Parse for $parser {
      type Args = $args;

      fn parse(
        &self,
        item: &Yaml,
        benchmark: &mut crate::benchmark::Benchmark,
        args: Self::Args,
      ) {
        benchmark
          .push(Box::new($action::new(args.name, item)));
      }
    }
  };
}

macro_rules! impl_action_parsers {
        ($($parser:ident : $action:ident : $args:ident),*) => {
          $(impl_action_parser!($parser, $action, $args);)*
        };
    }

impl_action_parsers! {
  AssertParser : Assert : AssertArgs,
  AssignParser : Assign : AssignArgs,
  DelayParser : Delay : DelayArgs
}
pub struct RequestParser<'a>(
  pub PhantomData<RequestArgs<'a>>,
);

pub struct RequestArgs<'a> {
  name: String,
  assign: Option<String>,
  parent_path: &'a str,
}

impl<'a> From<ParserArgs<'a>> for RequestArgs<'a> {
  fn from(value: ParserArgs<'a>) -> Self {
    Self {
      name: value.name,
      assign: value.assign,
      parent_path: value.parent_path,
    }
  }
}

impl<'a> Parse for RequestParser<'a> {
  type Args = RequestArgs<'a>;

  fn parse(
    &self,
    item: &Yaml,
    benchmark: &mut crate::benchmark::Benchmark,
    args: Self::Args,
  ) {
    benchmark.push(Box::new(Request::new(
      args.name.clone(),
      args.assign.clone(),
      item,
      args.parent_path,
    )));
  }
}

pub struct ExecParser;

pub struct ExecArgs {
  name: String,
  assign: Option<String>,
}

impl<'a> From<ParserArgs<'a>> for ExecArgs {
  fn from(value: ParserArgs<'a>) -> Self {
    Self {
      name: value.name,
      assign: value.assign,
    }
  }
}

impl Parse for ExecParser {
  type Args = ExecArgs;

  fn parse(
    &self,
    item: &Yaml,
    benchmark: &mut crate::benchmark::Benchmark,
    args: Self::Args,
  ) {
    benchmark.push(Box::new(Exec::new(
      args.name.clone(),
      args.assign.clone(),
      item,
    )));
  }
}

pub struct IncludeParser<'a>(&'a Tags);

impl<'a> IncludeParser<'a> {
  pub fn with_state(tags: &'a Tags) -> Self {
    Self(tags)
  }
}

pub struct IncludeArgs<'a> {
  parent_path: &'a str,
}

#[cfg(test)]
impl<'a> IncludeArgs<'a> {
  pub fn new(parent_path: &'a str) -> Self {
    Self {
      parent_path,
    }
  }
}

impl<'a> From<ParserArgs<'a>> for IncludeArgs<'a> {
  fn from(value: ParserArgs<'a>) -> Self {
    Self {
      parent_path: value.parent_path,
    }
  }
}

const INCLUDE: &str = "include";
impl<'a> Parse for IncludeParser<'a> {
  type Args = IncludeArgs<'a>;

  fn parse(
    &self,
    item: &Yaml,
    benchmark: &mut Benchmark,
    args: Self::Args,
  ) {
    let include_path = item[INCLUDE].as_str().unwrap();

    let include_filepath = Path::new(&args.parent_path)
      .with_file_name(include_path);
    let final_path = include_filepath.to_str().unwrap();

    walk(final_path, benchmark, None, self.0);
  }
}

use serde::Deserialize;
use yaml_rust::Yaml;

use crate::{
  actions::{Assert, Assign, Delay, Exec, Request},
  benchmark::Benchmark,
};

pub trait Expand: Send + Sync + 'static {
  type Args;
  fn create_args(name: String, assign: Option<String>, parent_path: Option<&str>) -> Self::Args;
  fn expand(item: &Yaml, benchmark: &mut Benchmark, args: Self::Args);

  fn invoke(item: &Yaml, benchmark: &mut Benchmark, name: String, assign: Option<String>, parent_path: Option<&str>) {
    Self::expand(item, benchmark, Self::create_args(name, assign, parent_path))
  }
}

macro_rules! impl_action_expandable {
  ($expander:ident, $action:ident, $args:ident) => {
    pub struct $expander;

    pub struct $args {
      name: String,
    }

    impl Expand for $expander {
      type Args = $args;
      fn create_args(name: String, _assign: Option<String>, _parent_path: Option<&str>) -> Self::Args {
        $args {
          name,
        }
      }
      fn expand(item: &Yaml, benchmark: &mut crate::benchmark::Benchmark, args: Self::Args) {
        benchmark.push(Box::new($action::new(args.name, item)));
      }
    }
  };
}

macro_rules! impl_action_expandables {
      ($($expander:ident : $action:ident : $args:ident),*) => {
        $(impl_action_expandable!($expander, $action, $args);)*
      };
  }

impl_action_expandables! {
  AssertExpander : Assert : AssertArgs,
  AssignExpander : Assign : AssignArgs,
  DelayExpander : Delay : DelayArgs
}
pub struct RequestExpander;

pub struct RequestArgs {
  name: String,
  assign: Option<String>,
  parent_path: String,
}

impl Expand for RequestExpander {
  type Args = RequestArgs;

  fn create_args(name: String, assign: Option<String>, parent_path: Option<&str>) -> Self::Args {
    RequestArgs {
      name,
      assign,
      parent_path: parent_path.unwrap().to_owned(),
    }
  }

  fn expand(item: &Yaml, benchmark: &mut crate::benchmark::Benchmark, args: Self::Args) {
    benchmark.push(Box::new(Request::new(args.name.clone(), args.assign.clone(), item, &args.parent_path)));
  }
}
pub struct ExecExpander;

pub struct ExecArgs {
  name: String,
  assign: Option<String>,
}

impl Expand for ExecExpander {
  type Args = ExecArgs;

  fn create_args(name: String, assign: Option<String>, _parent_path: Option<&str>) -> Self::Args {
    ExecArgs {
      name,
      assign,
    }
  }

  fn expand(item: &Yaml, benchmark: &mut crate::benchmark::Benchmark, args: Self::Args) {
    benchmark.push(Box::new(Exec::new(args.name.clone(), args.assign.clone(), item)));
  }
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IncludeOp {
  Include,
  Assert,
  Assign,
  // DbQuery,
  Delay,
  Exec,
  Request,
}

impl From<&str> for IncludeOp {
  fn from(value: &str) -> Self {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).unwrap()
  }
}

impl IncludeOp {
  pub fn expand(&self, item: &Yaml, benchmark: &mut Benchmark, name: String, assign: Option<String>, parent_path: Option<&str>) {
    match self {
      IncludeOp::Include => todo!(),
      IncludeOp::Assert => AssertExpander::invoke(item, benchmark, name, assign, parent_path),
      IncludeOp::Assign => AssignExpander::invoke(item, benchmark, name, assign, parent_path),
      IncludeOp::Delay => DelayExpander::invoke(item, benchmark, name, assign, parent_path),
      IncludeOp::Exec => ExecExpander::invoke(item, benchmark, name, assign, parent_path),
      IncludeOp::Request => RequestExpander::invoke(item, benchmark, name, assign, parent_path),
    }
  }
}

#[cfg(test)]
mod test {
  use crate::expandable::expand::IncludeOp;

  #[test]
  fn test() {
    assert_eq!(serde_json::from_value::<IncludeOp>(serde_json::Value::String("request".to_owned())).unwrap(), IncludeOp::Request)
  }
}

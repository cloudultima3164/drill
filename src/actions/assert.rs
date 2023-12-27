use async_trait::async_trait;
use colored::*;
use serde_json::json;
use yaml_rust::Yaml;

use crate::actions::extract;
use crate::actions::Runnable;
use crate::benchmark::{Context, Pool, Reports};
use crate::config::Config;
use crate::interpolator;

#[derive(Clone)]
pub struct Assert {
  name: String,
  key: String,
  value: String,
}

impl Assert {
  pub fn is_that_you(item: &Yaml) -> bool {
    item["assert"].as_hash().is_some()
  }

  pub fn new(item: &Yaml, _with_item: Option<Yaml>) -> Assert {
    let name = extract(item, "name");
    let key = extract(&item["assert"], "key");
    let value = extract(&item["assert"], "value");

    Assert {
      name,
      key,
      value,
    }
  }
}

#[async_trait]
impl Runnable for Assert {
  async fn execute(&self, context: &mut Context, _reports: &mut Reports, _pool: &Pool, config: &Config) {
    let interpolator = interpolator::Interpolator::new(context);
    let eval = format!("{{{{ {} }}}}", &self.key);
    let lhs = interpolator.resolve(&eval);
    let comparable = interpolator.resolve(&self.value);
    let rhs = json!(comparable);

    if !config.quiet {
      println!("{:width$} {}={}", self.name.green(), self.key.cyan().bold(), comparable.magenta(), width = 25);
    }

    if !lhs.eq(&rhs) {
      panic!("Assertion mismatched: {} != {}", lhs, rhs);
    }

    if !config.quiet {
      println!("{:width$}", "Assertion successful".red(), width = 25);
    }
  }
}

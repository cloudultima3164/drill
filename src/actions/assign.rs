use async_trait::async_trait;
use colored::*;

use crate::actions::Runnable;
use crate::benchmark::{Context, Pool, Reports};
use crate::config::Config;

#[derive(Clone)]
pub struct Assign {
  name: String,
  key: String,
  value: serde_json::Value,
}

impl Assign {
  pub fn new(name: String, key: String, value: serde_json::Value) -> Self {
    Self {
      name,
      key,
      value,
    }
  }
}

#[async_trait]
impl Runnable for Assign {
  async fn execute(
    &self,
    context: &mut Context,
    _reports: &mut Reports,
    _pool: &Pool,
    config: &Config,
  ) {
    if !config.quiet {
      println!(
        "{:width$} {}={}",
        self.name.green(),
        self.key.cyan().bold(),
        serde_json::to_string(&self.value).unwrap().magenta(),
        width = 25
      );
    }

    context.insert(self.key.to_owned(), self.value.to_owned());
  }
}

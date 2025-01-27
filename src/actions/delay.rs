use async_trait::async_trait;
use colored::*;
use tokio::time::sleep;

use crate::actions::Runnable;
use crate::benchmark::{Context, Pool, Reports};
use crate::config::Config;

use std::time::Duration;

#[derive(Clone)]
pub struct Delay {
  name: String,
  seconds: u64,
}

impl Delay {
  pub fn new(name: String, seconds: u64) -> Self {
    Self {
      name,
      seconds,
    }
  }
}

#[async_trait]
impl Runnable for Delay {
  async fn execute(
    &self,
    _context: &mut Context,
    _reports: &mut Reports,
    _pool: &Pool,
    config: &Config,
  ) {
    sleep(Duration::from_secs(self.seconds)).await;

    if !config.quiet {
      println!(
        "{:width$} {}{}",
        self.name.green(),
        self.seconds.to_string().cyan().bold(),
        "s".magenta(),
        width = 25
      );
    }
  }
}

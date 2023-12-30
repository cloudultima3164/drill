use async_trait::async_trait;
use serde::Deserialize;

mod assert;
mod assign;
mod db_query;
mod delay;
mod exec;
mod request;

pub use self::assert::Assert;
pub use self::assign::Assign;
pub use self::db_query::DbQuery;
pub use self::delay::Delay;
pub use self::exec::Exec;
pub use self::request::Request;

use crate::benchmark::{Context, Pool, Reports};
use crate::config::Config;

use std::fmt;

#[async_trait]
pub trait Runnable {
  async fn execute(
    &self,
    context: &mut Context,
    reports: &mut Reports,
    pool: &Pool,
    config: &Config,
  );
}

#[derive(Deserialize)]
pub enum WithOps {
  #[serde(rename(deserialize = "with_items"))]
  Items,
  #[serde(rename(deserialize = "with_items_range"))]
  Range,
  #[serde(rename(deserialize = "with_items_from_csv"))]
  Csv,
  #[serde(rename(deserialize = "with_items_from_file"))]
  File,
}

impl From<&str> for WithOps {
  fn from(value: &str) -> Self {
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
      .map_err(|_| format!("Unknown 'with' attribute, {value}"))
      .unwrap()
  }
}

#[derive(Clone)]
pub struct Report {
  pub name: String,
  pub duration: f64,
  pub status: u16,
}

impl fmt::Debug for Report {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "\n- name: {}\n  duration: {}\n", self.name, self.duration)
  }
}

impl fmt::Display for Report {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "\n- name: {}\n  duration: {}\n  status: {}\n",
      self.name, self.duration, self.status
    )
  }
}

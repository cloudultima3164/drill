use crate::actions::extract;
use crate::benchmark::{Context, Pool, Reports};
use crate::config::Config;
use crate::db::DB;
use crate::interpolator;
use async_trait::async_trait;
use colored::Colorize;
use serde::ser::{SerializeMap, SerializeSeq};
use serde::Serialize;
use serde_json::json;
use sqlx::postgres::PgRow;
use sqlx::{Column, Executor, PgPool, Row, ValueRef};
use yaml_rust::Yaml;

use super::Runnable;

#[derive(Clone)]
pub struct DbQuery {
  name: String,
  assign: Option<String>,
  target: String,
  query: String,
}

impl DbQuery {
  pub fn new(
    name: String,
    assign: Option<String>,
    item: &Yaml,
    _with_item: Option<Yaml>,
  ) -> DbQuery {
    let target = extract(item, "target");
    let query = extract(item, "query");

    DbQuery {
      name,
      target,
      query,
      assign,
    }
  }
}

#[async_trait]
impl Runnable for DbQuery {
  async fn execute(
    &self,
    context: &mut Context,
    _reports: &mut Reports,
    _pool: &Pool,
    config: &Config,
  ) {
    let interpolator =
      interpolator::Interpolator::new(context);
    let db = config
      .dbs
      .get(&self.target)
      .unwrap_or_else(|| {
        panic!("No such DB: {}", self.target)
      })
      .to_db(&interpolator);
    if !config.quiet {
      println!(
        "{:width$} {} <= {}...",
        self.name.green(),
        self.target.cyan().bold(),
        self
          .query
          .split_at(if self.query.len() < 25 {
            self.query.len()
          } else {
            25
          })
          .0
          .bright_purple(),
        width = 25
      );
    }

    let final_query = interpolator.resolve(&self.query);

    let results = match db {
      DB::Postgres(pool) => QueryResults::Postgres(
        execute_postgres_query(&final_query, &pool).await,
      ),
    };

    if let Some(ref key) = self.assign {
      context.insert(key.to_owned(), json!(results));
    }
  }
}

async fn execute_postgres_query(
  query: &str,
  pool: &PgPool,
) -> Vec<PgRow> {
  pool.fetch_all(query).await.unwrap_or_else(|_| {
    panic!(
      "Query execution failed ({})",
      query.split_at(10).0
    )
  })
}

pub enum QueryResults {
  Postgres(Vec<PgRow>),
}

impl Serialize for QueryResults {
  fn serialize<S>(
    &self,
    serializer: S,
  ) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    match self {
      QueryResults::Postgres(v) => {
        let mut seq =
          serializer.serialize_seq(Some(v.len()))?;
        for e in v {
          seq.serialize_element(&PostgresRow(e))?;
        }
        seq.end()
      }
    }
  }
}

struct PostgresRow<'a>(&'a PgRow);

impl<'a> Serialize for PostgresRow<'a> {
  fn serialize<S>(
    &self,
    serializer: S,
  ) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let columns_len = self.0.columns().len();
    let mut map =
      serializer.serialize_map(Some(columns_len))?;
    for col in 0..columns_len {
      let key = self.0.column(col).name();
      let val = self
        .0
        .try_get_raw(col)
        .map(|val| {
          if val.is_null() {
            "null"
          } else {
            val.as_str().unwrap()
          }
        })
        .unwrap_or_else(|_| {
          panic!("Failed to get value from column {}", col)
        });
      map.serialize_entry(key, val)?;
    }
    map.end()
  }
}

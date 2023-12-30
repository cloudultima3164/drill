use std::{convert::TryFrom, time::Duration};

use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::interpolator::Interpolator;

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged, rename_all = "snake_case")]
pub enum YamlDbDefinition {
  ConnectionString {
    connection_string: String,
  },
  Parameterized {
    #[serde(rename = "type")]
    typ: String,
    host: String,
    port: String,
    user: String,
    password: String,
    dbname: String,
  },
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum DbType {
  Postgres,
}

impl TryFrom<&str> for DbType {
  type Error = ();
  fn try_from(value: &str) -> Result<Self, Self::Error> {
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
      .map_err(|_| ())
  }
}

#[derive(Clone)]
pub enum DB {
  Postgres(PgPool),
}

#[derive(Serialize, Debug, Clone)]
pub struct DbDefinition {
  typ: DbType,
  connection_string: String,
}

impl From<YamlDbDefinition> for DbDefinition {
  fn from(value: YamlDbDefinition) -> Self {
    match value {
      YamlDbDefinition::ConnectionString {
        connection_string,
      } => {
        let typ = connection_string.split_once("://").unwrap().0;
        let typ = DbType::try_from(typ)
          .unwrap_or_else(|_| panic!("Invalid DB type '{}'.", typ));
        Self {
          typ,
          connection_string: connection_string.to_string(),
        }
      }
      YamlDbDefinition::Parameterized {
        typ,
        host,
        port,
        user,
        password,
        dbname,
      } => Self {
        typ: DbType::try_from(typ.as_str())
          .unwrap_or_else(|_| panic!("Invalid DB type '{}'.", typ)),
        connection_string: build_connection_string(
          &serde_yaml::to_string(&typ).unwrap(),
          &host,
          &port,
          &user,
          &password,
          &dbname,
        ),
      },
    }
  }
}

fn build_connection_string(
  typ: &str,
  host: &str,
  port: &str,
  user: &str,
  password: &str,
  dbname: &str,
) -> String {
  format!("{typ}://{user}:{password}@{host}:{port}/{dbname}")
}

impl DbDefinition {
  pub fn to_db(&self, interpolator: &Interpolator) -> DB {
    match &self.typ {
      DbType::Postgres => {
        DB::Postgres(connect_postgres(&self.connection_string, interpolator))
      }
    }
  }
}

const MAX_CONNECTIONS: u32 = 4;
const TIMEOUT: u64 = 30;
fn connect_postgres(
  connection_string: &str,
  interpolator: &Interpolator,
) -> PgPool {
  let resolved_con_str = interpolator.resolve(connection_string);
  PgPoolOptions::new()
    .max_connections(MAX_CONNECTIONS)
    .idle_timeout(Duration::from_secs(TIMEOUT))
    .connect_lazy(&resolved_con_str)
    .expect("Failed to connect to database")
}

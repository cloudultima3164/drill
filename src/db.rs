use std::{convert::TryFrom, time::Duration};

use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use yaml_rust::Yaml;

use crate::interpolator::Interpolator;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DbType {
  Postgres,
}

impl TryFrom<&str> for DbType {
  type Error = ();
  fn try_from(value: &str) -> Result<Self, Self::Error> {
    serde_json::from_value(serde_json::Value::String(
      value.to_owned(),
    ))
    .map_err(|_| ())
  }
}

#[derive(Clone)]
pub enum DB {
  Postgres(PgPool),
}

#[derive(Serialize)]
pub struct DbDefinition {
  typ: DbType,
  connection_string: String,
}

impl From<&Yaml> for DbDefinition {
  fn from(def: &Yaml) -> Self {
    let def = def.as_hash().unwrap();
    let get =
      |s: &str| def.get(&Yaml::String(s.to_owned()));
    if let Some(typ) = get("type") {
      let typ = typ.as_str().unwrap();
      let host = get("host").unwrap().as_str().unwrap();
      let port = get("port").unwrap().as_str().unwrap();
      let user = get("user").unwrap().as_str().unwrap();
      let password =
        get("password").unwrap().as_str().unwrap();
      let dbname = get("dbname").unwrap().as_str().unwrap();
      Self {
        typ: DbType::try_from(typ).unwrap_or_else(|_| {
          panic!("Invalid DB type '{}'.", typ)
        }),
        connection_string: build_connection_string(
          typ, host, port, user, password, dbname,
        ),
      }
    } else if let Some(con_str) = get("connection_string") {
      let con_str = con_str.as_str().unwrap();
      let typ = con_str.split_once("://").unwrap().0;
      let typ =
        DbType::try_from(typ).unwrap_or_else(|_| {
          panic!("Invalid DB type '{}'.", typ)
        });
      Self {
        typ,
        connection_string: con_str.to_owned(),
      }
    } else {
      panic!("Neither \"type\" nor \"connection_string\" given for database. Can't determine connection method.");
    }
  }
}

impl DbDefinition {
  pub fn to_db(&self, interpolator: &Interpolator) -> DB {
    match &self.typ {
      DbType::Postgres => DB::Postgres(connect_postgres(
        &self.connection_string,
        interpolator,
      )),
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
  format!(
    "{typ}://{user}:{password}@{host}:{port}/{dbname}"
  )
}

const MAX_CONNECTIONS: u32 = 4;
const TIMEOUT: u64 = 30;
fn connect_postgres(
  connection_string: &str,
  interpolator: &Interpolator,
) -> PgPool {
  let resolved_con_str =
    interpolator.resolve(connection_string);
  PgPoolOptions::new()
    .max_connections(MAX_CONNECTIONS)
    .idle_timeout(Duration::from_secs(TIMEOUT))
    .connect_lazy(&resolved_con_str)
    .expect("Failed to connect to database")
}

use std::{
  collections::{BTreeMap, HashMap},
  fs::File,
  io::Read,
  path::PathBuf,
  str::FromStr,
};

use serde::{Deserialize, Deserializer};

use crate::{
  db::YamlDbDefinition,
  reader::{get_file, read_csv_file_as_yml, read_file_as_yml_array},
};

const NITERATIONS: u64 = 1;
const NRAMPUP: u64 = 0;

fn default_iterations() -> u64 {
  NITERATIONS
}

fn default_rampup() -> u64 {
  NRAMPUP
}

#[derive(Debug, Deserialize, Clone)]
pub struct BenchmarkDoc {
  #[serde(default = "default_iterations")]
  pub iterations: u64,
  #[serde(default = "default_rampup")]
  pub rampup: u64,
  #[serde(default = "Default::default", deserialize_with = "get_env")]
  pub env: BTreeMap<String, String>,
  #[serde(default = "num_cpus::get")]
  pub concurrency: usize,
  #[serde(deserialize_with = "get_databases", flatten)]
  pub databases: BTreeMap<String, YamlDbDefinition>,
  #[serde(default = "BTreeMap::new")]
  pub urls: BTreeMap<String, String>,
  #[serde(default = "BTreeMap::new")]
  pub global: BTreeMap<String, String>,
  pub plan: Vec<PlanItem>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PlanItem {
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub assign: Option<String>,
  #[serde(flatten)]
  pub action: Action,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
  Assert {
    key: String,
    value: serde_json::Value,
  },
  Assign {
    key: String,
    value: serde_json::Value,
  },
  DbQuery {
    target: String,
    query: String,
    #[serde(default = "Default::default", deserialize_with = "with_items")]
    with_items: Option<WithItems>,
  },
  Delay {
    seconds: u64,
  },
  Exec {
    command: String,
  },
  Request {
    #[serde(skip_serializing_if = "Option::is_none")]
    base: Option<String>,
    url: String,
    #[serde(default = "Default::default")]
    time: f64,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "Default::default")]
    headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(default = "Default::default", deserialize_with = "with_items")]
    with_items: Option<WithItems>,
  },
  #[serde(deserialize_with = "include_doc_deser")]
  Include(BenchmarkDoc),
}

#[derive(Debug, Clone)]
pub struct WithItems {
  pub shuffle: bool,
  pub pick: Pick,
  pub items: Vec<serde_yaml::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum WithItemsType {
  File {
    path: String,
    #[serde(default = "Default::default")]
    shuffle: bool,
    #[serde(default = "Default::default")]
    pick: Pick,
  },
  Range {
    start: usize,
    stop: usize,
    step: usize,
    #[serde(default = "Default::default")]
    shuffle: bool,
    #[serde(default = "Default::default")]
    pick: Pick,
  },
  Direct {
    items: Vec<BTreeMap<String, serde_yaml::Value>>,
    #[serde(default = "Default::default")]
    shuffle: bool,
    #[serde(default = "Default::default")]
    pick: Pick,
  },
}

fn with_items<'de, D>(de: D) -> Result<Option<WithItems>, D::Error>
where
  D: Deserializer<'de>,
{
  let items: WithItemsType =
    serde_yaml::from_value(Deserialize::deserialize(de)?).unwrap();
  match items {
    WithItemsType::File {
      path,
      shuffle,
      pick,
    } => {
      let path = PathBuf::from_str(&path).unwrap();
      let items = match serde_yaml::from_str::<FileType>(
        path.extension().unwrap().to_str().unwrap(),
      )
      .unwrap()
      {
        FileType::Csv => read_csv_file_as_yml(&path),
        FileType::Yaml | FileType::Yml => read_file_as_yml_array(&path),
      };
      pick.validate(&items);
      Ok(Some(WithItems {
        items,
        pick,
        shuffle,
      }))
    }
    WithItemsType::Range {
      start,
      stop,
      step,
      shuffle,
      pick,
    } => {
      let items: Vec<serde_yaml::Value> = (start..stop)
        .step_by(step)
        .map(|n| serde_yaml::Value::Number(serde_yaml::Number::from(n)))
        .collect();
      pick.validate(&items);
      Ok(Some(WithItems {
        items,
        pick,
        shuffle,
      }))
    }
    WithItemsType::Direct {
      items,
      shuffle,
      pick,
    } => {
      let items: Vec<serde_yaml::Value> =
        serde_json::from_str(&serde_json::to_string(&items).unwrap()).unwrap();
      pick.validate(&items);
      Ok(Some(WithItems {
        items,
        pick,
        shuffle,
      }))
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename = "lowercase")]
enum FileType {
  Csv,
  Yml,
  Yaml,
}

/// Parses "pick" option, which tells the app how many rows of data
/// it should take from the data source.
#[derive(Default, Debug, Clone, Copy, Deserialize)]
pub struct Pick(i64);

impl Pick {
  pub fn validate(&self, with_items: &[serde_yaml::Value]) {
    if self.0.is_negative() {
      panic!("pick option should not be negative, but was {}", self.0);
    } else if self.0 as usize > with_items.len() {
      panic!(
        "pick option should not be greater than the provided items, but was {}",
        self.0
      );
    }
  }

  pub fn inner(&self) -> usize {
    self.0 as usize
  }
}

fn include_doc_deser<'de, D>(de: D) -> Result<BenchmarkDoc, D::Error>
where
  D: Deserializer<'de>,
{
  let path: String = Deserialize::deserialize(de)?;
  Ok(include_doc(&path))
}

pub fn include_doc(path: &str) -> BenchmarkDoc {
  serde_yaml::from_reader(get_file(&path)).unwrap()
}

fn get_env<'de, D>(de: D) -> Result<BTreeMap<String, String>, D::Error>
where
  D: Deserializer<'de>,
{
  let path: String = Deserialize::deserialize(de)?;
  let env_file = PathBuf::from(&path);
  let env = if let Ok(true) = env_file.try_exists() {
    let mut buffer = String::new();
    if let Ok(mut file) = File::open(env_file) {
      if file.read_to_string(&mut buffer).is_err() {
        return Ok(BTreeMap::new());
      }
    }
    buffer
      .lines()
      .map(|s| {
        s.split_once('=')
          .map(|(k, v)| (k.to_owned(), v.to_owned()))
          .or_else(|| {
            let mut split = s.split_whitespace();
            Some((
              split.next().expect(".env key before whitespace").to_owned(),
              split.next().expect(".env value after whitespace").to_owned(),
            ))
          })
          .unwrap()
      })
      .collect::<BTreeMap<String, String>>()
  } else {
    BTreeMap::new()
  };
  Ok(env)
}

fn get_databases<'de, D>(
  de: D,
) -> Result<BTreeMap<String, YamlDbDefinition>, D::Error>
where
  D: Deserializer<'de>,
{
  let map: HashMap<String, HashMap<String, serde_yaml::Value>> =
    Deserialize::deserialize(de)?;

  Ok(
    map
      .get("database")
      .cloned()
      .unwrap_or_else(HashMap::new)
      .iter()
      .map(|(k, v)| (k.clone(), serde_yaml::from_value(v.clone()).unwrap()))
      .collect(),
  )
}

fn default_method() -> String {
  "GET".into()
}

#[cfg(test)]
mod test {}

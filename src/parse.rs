mod op;
// mod parsers;

use std::{
  collections::{BTreeMap, HashMap},
  fs::File,
  io::Read,
  path::PathBuf,
  str::FromStr,
};

use serde::{Deserialize, Deserializer};

use crate::{
  // actions::{extract, extract_optional},
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
  pub database: BTreeMap<String, YamlDbDefinition>,
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

// pub fn walk(
//   parent_path: &str,
//   benchmark: &mut Benchmark,
//   accessor: Option<&str>,
//   tags: &Tags,
// ) {
//   let docs = reader::read_file_as_yml(parent_path);
//   let items =
//     reader::read_yaml_doc_accessor(&docs[0], accessor)
//       .as_sequence()
//       .unwrap();

//   for item in items {
//     if tags.should_skip_item(item) {
//       continue;
//     }

//     let name = extract(item, "name");
//     let assign = extract_optional(item, "assign");

//     let parser_args =
//       ParserArgs::new(name, assign, parent_path);

//     let mut op_iter = item
//       .as_hash()
//       .unwrap()
//       .into_iter()
//       .filter_map(|(key, val)| {
//         Op::try_from(key.as_str().unwrap())
//           .ok()
//           .map(|parsed_key| (parsed_key, val))
//       })
//       .rev();

//     let (op, target_value) = if op_iter.clone().count() > 1
//     {
//       op_iter.find_map(|(op, val)| {
//         op.ne(&Op::Assign).then_some((op, val))
//       })
//     } else {
//       op_iter.next()
//     }
//     .expect("No actionable keys found in item");

//     parse(op, target_value, benchmark, parser_args, tags)
//   }
// }

// fn parse(
//   op: Op,
//   item: &'a Yaml,
//   benchmark: &'a mut Benchmark,
//   args: ParserArgs,
//   tags: &'a Tags,
// ) {
//   match op {
//     Op::Include => Parser::new(
//       IncludeParser::with_state(tags),
//       IncludeArgs::from(args),
//     )
//     .parse(item, benchmark),
//     Op::Assert => {
//       Parser::new(AssertParser, AssertArgs::from(args))
//         .parse(item, benchmark)
//     }
//     Op::Assign => {
//       Parser::new(AssignParser, AssignArgs::from(args))
//         .parse(item, benchmark)
//     }
//     Op::Delay => {
//       Parser::new(DelayParser, DelayArgs::from(args))
//         .parse(item, benchmark)
//     }
//     Op::Exec => {
//       Parser::new(ExecParser, ExecArgs::from(args))
//         .parse(item, benchmark)
//     }
//     Op::Request => Parser::new(
//       RequestParser(PhantomData),
//       RequestArgs::from(args),
//     )
//     .parse(item, benchmark),
//     Op::DbQuery => {
//       Parser::new(DbQueryParser, DbQueryArgs::from(args))
//         .parse(item, benchmark)
//     }
//   }
// }

// pub trait Parse: Send + Sync {
//   type Args;

//   fn parse(
//     &self,
//     item: &Yaml,
//     benchmark: &mut Benchmark,
//     args: Self::Args,
//   );
// }

// struct Parser<'a, P: Parse> {
//   inner: P,
//   args: <P as Parse>::Args,
//   _marker: PhantomData<ParserArgs>,
// }

// impl<'a, P: Parse> Parser<'a, P> {
//   fn new(inner: P, args: <P as Parse>::Args) -> Self {
//     Self {
//       inner,
//       args,
//       _marker: PhantomData,
//     }
//   }

//   fn parse(self, item: &Yaml, benchmark: &mut Benchmark) {
//     self.inner.parse(item, benchmark, self.args)
//   }
// }

// pub struct ParserArgs {
//   name: String,
//   assign: Option<String>,
//   parent_path: String,
// }

// impl ParserArgs {
//   fn new(
//     name: String,
//     assign: Option<String>,
//     parent_path: String,
//   ) -> Self {
//     Self {
//       name,
//       assign,
//       parent_path,
//     }
//   }
// }

#[cfg(test)]
mod test {
  // use super::include_doc;

  // #[test]
  // fn parse_include() {
  //   let doc =
  //     include_doc("/Users/ak_lo/programming/rust/drill/test/test_request.yml");
  //   assert_eq!(
  //     doc.urls.into_iter().next().unwrap().1,
  //     serde_yaml::Value::String("http://jisho.org/api/v1/search/".into())
  //   );
  //   assert_eq!(doc.plan[1].assign, Some("echo_result".into()));
  // }

  // #[test]
  // #[should_panic]
  // fn invalid_parse() {
  //   let text = "---\nname: Include comment\ninclude: {{ memory }}.yml";
  //   let docs = yaml_rust::YamlLoader::load_from_str(text).unwrap();
  //   let doc = &docs[0];
  //   let mut benchmark: Benchmark = Benchmark::new();

  //   walk(doc, &mut benchmark, &Tags::new(vec![], vec![]));
  // }
}

// #[cfg(test)]
// mod tests {
//   use super::*;

//   mod pick {
//     use super::*;

//     #[test]
//     fn should_return_the_configured_value() {
//       let text = "---\nname: foobar\nrequest:\n  url: /api/{{ item }}\npick: 2\nwith_items:\n  - 1\n  - 2\n  - 3";
//       let item = &yaml_rust::YamlLoader::load_from_str(text).unwrap()[0];
//       let pick = validate_pick(item, item["with_items"].as_vec().unwrap());

//       assert_eq!(pick, 2);
//     }

//     #[test]
//     fn should_return_the_with_items_length_if_unset() {
//       let text = "---\nname: foobar\nrequest:\n  url: /api/{{ item }}\nwith_items:\n  - 1\n  - 2\n  - 3";
//       let item = &yaml_rust::YamlLoader::load_from_str(text).unwrap()[0];
//       let pick = validate_pick(item, item["with_items"].as_vec().unwrap());

//       assert_eq!(pick, 3);
//     }

//     #[test]
//     #[should_panic(expected = "pick option should not be negative, but was -1")]
//     fn should_panic_for_negative_values() {
//       let text = "---\nname: foobar\nrequest:\n  url: /api/{{ item }}\npick: -1\nwith_items:\n  - 1\n  - 2\n  - 3";
//       let item = &yaml_rust::YamlLoader::load_from_str(text).unwrap()[0];
//       validate_pick(item, item["with_items"].as_vec().unwrap());
//     }

//     #[test]
//     #[should_panic(expected = "pick option should not be greater than the provided items, but was 4")]
//     fn should_panic_for_values_greater_than_the_items_list() {
//       let text = "---\nname: foobar\nrequest:\n  url: /api/{{ item }}\npick: 4\nwith_items:\n  - 1\n  - 2\n  - 3";
//       let item = &yaml_rust::YamlLoader::load_from_str(text).unwrap()[0];
//       validate_pick(item, item["with_items"].as_vec().unwrap());
//     }
//   }
// }

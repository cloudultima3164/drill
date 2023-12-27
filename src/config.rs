use std::collections::BTreeMap;

use yaml_rust::{Yaml, YamlLoader};

use crate::benchmark::Context;
use crate::interpolator;
use crate::reader;

const NITERATIONS: i64 = 1;
const NRAMPUP: i64 = 0;

pub struct Config {
  pub urls: BTreeMap<String, String>,
  pub global: BTreeMap<String, String>,
  pub concurrency: i64,
  pub iterations: i64,
  pub relaxed_interpolations: bool,
  pub no_check_certificate: bool,
  pub rampup: i64,
  pub quiet: bool,
  pub nanosec: bool,
  pub timeout: u64,
  pub verbose: bool,
}

impl Config {
  pub fn new(path: &str, relaxed_interpolations: bool, no_check_certificate: bool, quiet: bool, nanosec: bool, timeout: u64, verbose: bool) -> Config {
    let config_file = reader::read_file(path);

    let config_docs = YamlLoader::load_from_str(config_file.as_str()).unwrap();
    let config_doc = &config_docs[0];

    let context: Context = Context::new();
    let interpolator = interpolator::Interpolator::new(&context);

    let iterations = read_i64_configuration(config_doc, &interpolator, "iterations", NITERATIONS);
    let concurrency = read_i64_configuration(config_doc, &interpolator, "concurrency", iterations);
    let rampup = read_i64_configuration(config_doc, &interpolator, "rampup", NRAMPUP);
    let urls = read_hash_configuration(config_doc, &interpolator, "urls");
    let global = read_hash_configuration(config_doc, &interpolator, "global");

    if concurrency > iterations {
      panic!("The concurrency can not be higher than the number of iterations")
    }

    Config {
      urls,
      global,
      concurrency,
      iterations,
      relaxed_interpolations,
      no_check_certificate,
      rampup,
      quiet,
      nanosec,
      timeout,
      verbose,
    }
  }
}

#[allow(dead_code)]
fn read_str_configuration(config_doc: &Yaml, interpolator: &interpolator::Interpolator, name: &str, default: &str) -> String {
  match config_doc[name].as_str() {
    Some(value) => {
      if value.contains('{') {
        interpolator.resolve(value)
      } else {
        value.to_owned()
      }
    }
    None => {
      // Won't this always be false?
      if config_doc[name].as_str().is_some() {
        println!("Invalid {name} value!");
      }

      default.to_owned()
    }
  }
}

fn read_i64_configuration(config_doc: &Yaml, interpolator: &interpolator::Interpolator, name: &str, default: i64) -> i64 {
  let value = if let Some(value) = config_doc[name].as_i64() {
    Some(value)
  } else if let Some(key) = config_doc[name].as_str() {
    interpolator.resolve(key).parse::<i64>().ok()
  } else {
    None
  };

  match value {
    Some(value) => {
      if value < 0 {
        println!("Invalid negative {name} value!");

        default
      } else {
        value
      }
    }
    None => {
      if config_doc[name].as_str().is_some() {
        println!("Invalid {name} value!");
      }

      default
    }
  }
}

fn read_hash_configuration(config_doc: &Yaml, interpolator: &interpolator::Interpolator, name: &str) -> BTreeMap<String, String> {
  match config_doc[name].as_hash() {
    Some(map) => map
      .iter()
      .map(|(key, val)| {
        let key = key.as_str().unwrap().to_owned();
        let mut val = val.as_str().unwrap().to_owned();
        if val.contains('{') {
          val = interpolator.resolve(&val);
        }
        (key, val)
      })
      .collect(),
    None => BTreeMap::new(),
  }
}

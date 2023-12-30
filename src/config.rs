use crate::args::FlattenedCli;
use crate::db::DbDefinition;
use crate::parse::BenchmarkDoc;
use std::collections::BTreeMap;

// const NITERATIONS: i64 = 1;
// const NRAMPUP: i64 = 0;
const TIMEOUT: u64 = 30;

#[derive(Debug, Default)]
pub struct Config {
  pub urls: BTreeMap<String, String>,
  pub global: BTreeMap<String, String>,
  pub dbs: BTreeMap<String, DbDefinition>,
  pub concurrency: u64,
  pub iterations: u64,
  pub relaxed_interpolations: bool,
  pub no_check_certificate: bool,
  pub rampup: u64,
  pub quiet: bool,
  pub nanosec: bool,
  pub timeout: u64,
  pub verbose: bool,
}

impl From<&BenchmarkDoc> for Config {
  fn from(doc: &BenchmarkDoc) -> Self {
    Config {
      urls: doc.urls.clone(),
      global: {
        let mut global = doc.global.clone();
        global.append(&mut doc.env.clone());
        global
      },
      dbs: doc
        .database
        .clone()
        .into_iter()
        .map(|(k, v)| (k, DbDefinition::from(v)))
        .collect(),
      concurrency: doc.concurrency.min(doc.iterations as usize) as u64,
      iterations: doc.iterations,
      relaxed_interpolations: false,
      no_check_certificate: false,
      rampup: doc.rampup,
      quiet: false,
      nanosec: false,
      timeout: TIMEOUT,
      verbose: false,
    }
  }
}

impl Config {
  pub fn with_args(mut self, args: &FlattenedCli) -> Config {
    self.quiet = args.quiet;
    self.nanosec = args.nanosec;
    self.timeout =
      args.timeout.as_ref().map_or(10, |t| t.parse().unwrap_or(10));
    self.verbose = args.verbose;
    self.relaxed_interpolations = args.relaxed_interpolations;
    self.no_check_certificate = args.no_check_certificate;
    self
  }
}

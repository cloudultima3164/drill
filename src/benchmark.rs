use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt};

use serde_json::{json, Map, Value};
use tokio::{runtime, time::sleep};

use crate::actions::{
  Assert, Assign, DbQuery, Delay, Exec, Report, Request, Runnable,
};
use crate::args::FlattenedCli;
use crate::config::Config;

use crate::parse::BenchmarkDoc;
use crate::reader::read_file_as_yml;
use crate::writer;

use reqwest::Client;

use colored::*;

pub type Runner = Box<(dyn Runnable + Sync + Send)>;
pub type Benchmark = Vec<Runner>;
pub type Context = Map<String, Value>;
pub type Reports = Vec<Report>;
pub type PoolStore = HashMap<String, Client>;
pub type Pool = Arc<Mutex<PoolStore>>;

impl<'a> From<&'a BenchmarkDoc> for Benchmark {
  fn from(doc: &'a BenchmarkDoc) -> Self {
    doc
      .plan
      .iter()
      .map(|plan| {
        let name = plan.name.clone();
        let assign = plan.assign.clone();
        match plan.action.clone() {
          crate::parse::Action::Assert {
            key,
            value,
          } => Box::new(Assert::new(name, key, value)) as Runner,
          crate::parse::Action::Assign {
            key,
            value,
          } => Box::new(Assign::new(name, key, value)) as Runner,
          crate::parse::Action::DbQuery {
            target,
            query,
            with_items,
          } => Box::new(DbQuery::new(name, assign, target, query, with_items))
            as Runner,
          crate::parse::Action::Delay {
            seconds,
          } => Box::new(Delay::new(name, seconds)) as Runner,
          crate::parse::Action::Exec {
            command,
          } => Box::new(Exec::new(name, assign, command)) as Runner,
          crate::parse::Action::Request {
            base,
            url,
            time,
            method,
            headers,
            body,
            with_items,
          } => Box::new(Request::new(
            name, base, url, time, method, headers, body, with_items, assign,
          )),
          crate::parse::Action::Include(_) => todo!(),
        }
      })
      .collect()
  }
}

pub struct BenchmarkResult {
  pub reports: Vec<Reports>,
  pub duration: f64,
}

async fn run_iteration(
  benchmark: Arc<Benchmark>,
  pool: Pool,
  config: Arc<Config>,
  iteration: u64,
) -> Vec<Report> {
  if config.rampup > 0 {
    let delay = config.rampup / config.iterations;
    sleep(Duration::new(delay * iteration, 0)).await;
  }

  let mut context: Context = Context::new();
  let mut reports: Vec<Report> = Vec::new();

  context.insert("iteration".to_string(), json!(iteration.to_string()));
  context.insert("urls".to_string(), json!(config.urls));
  context.insert("global".to_string(), json!(config.global));

  for item in benchmark.iter() {
    item.execute(&mut context, &mut reports, &pool, &config).await;
  }

  reports
}

fn join<S: ToString>(l: Vec<S>, sep: &str) -> String {
  l.iter().fold(
    "".to_string(),
    |a,b| if !a.is_empty() {a+sep} else {a} + &b.to_string()
  )
}

pub fn execute(args: &FlattenedCli) -> BenchmarkResult {
  // let config = Arc::new(Config::new(args));

  let benchmark_doc: BenchmarkDoc =
    serde_yaml::from_value(read_file_as_yml(&args.benchmark_file)).unwrap();
  let config = Arc::new(Config::from(&benchmark_doc).with_args(args));

  if args.verbose {
    if args.report_path_option.is_some() {
      println!(
        "{}: {}. Ignoring {} and {} properties...",
        "Report mode".yellow(),
        "on".purple(),
        "concurrency".yellow(),
        "iterations".yellow()
      );
    } else {
      println!(
        "{} {}",
        "Concurrency".yellow(),
        config.concurrency.to_string().purple()
      );
      println!(
        "{} {}",
        "Iterations".yellow(),
        config.iterations.to_string().purple()
      );
      println!("{} {}", "Rampup".yellow(), config.rampup.to_string().purple());
    }

    println!("{}", "URLs".yellow());
    for (key, val) in config.urls.iter() {
      println!("  {}: {}", key.purple(), val.green());
    }

    println!("{}", "Global Variables".yellow());
    for (key, val) in config.global.iter() {
      println!("  {}: {}", key.purple(), val.green());
    }
    println!();
  }

  let threads = std::cmp::min(num_cpus::get(), config.concurrency as usize);
  let rt = runtime::Builder::new_multi_thread()
    .enable_all()
    .worker_threads(threads)
    .build()
    .unwrap();

  rt.block_on(async {
    let benchmark: Benchmark = Benchmark::from(&benchmark_doc);
    let pool_store: PoolStore = PoolStore::new();

    if benchmark.is_empty() {
      eprintln!("Empty benchmark. Exiting.");
      std::process::exit(1);
    }

    let benchmark = Arc::new(benchmark);
    let pool = Arc::new(Mutex::new(pool_store));

    if let Some(ref report_path) = args.report_path_option {
      let reports =
        run_iteration(benchmark.clone(), pool.clone(), config, 0).await;

      writer::write_file(report_path, join(reports, ""));

      BenchmarkResult {
        reports: vec![],
        duration: 0.0,
      }
    } else {
      let children = (0..config.iterations).map(|iteration| {
        run_iteration(
          benchmark.clone(),
          pool.clone(),
          config.clone(),
          iteration,
        )
      });

      let buffered =
        stream::iter(children).buffer_unordered(config.concurrency as usize);

      let begin = Instant::now();
      let reports: Vec<Vec<Report>> = buffered.collect::<Vec<_>>().await;
      let duration = begin.elapsed().as_secs_f64();

      BenchmarkResult {
        reports,
        duration,
      }
    }
  })
}

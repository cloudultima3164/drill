use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt};

use serde_json::{json, Map, Value};
use tokio::{runtime, time::sleep};

use crate::actions::{Report, Runnable};
use crate::args::FlattenedCli;
use crate::config::Config;
use crate::parse::walk;
use crate::tags::Tags;
use crate::writer;

use reqwest::Client;

use colored::*;

pub type Benchmark = Vec<Box<(dyn Runnable + Sync + Send)>>;
pub type Context = Map<String, Value>;
pub type Reports = Vec<Report>;
pub type PoolStore = HashMap<String, Client>;
pub type Pool = Arc<Mutex<PoolStore>>;

pub struct BenchmarkResult {
  pub reports: Vec<Reports>,
  pub duration: f64,
}

async fn run_iteration(
  benchmark: Arc<Benchmark>,
  pool: Pool,
  config: Arc<Config>,
  iteration: i64,
) -> Vec<Report> {
  if config.rampup > 0 {
    let delay = config.rampup / config.iterations;
    sleep(Duration::new((delay * iteration) as u64, 0))
      .await;
  }

  let mut context: Context = Context::new();
  let mut reports: Vec<Report> = Vec::new();

  context.insert(
    "iteration".to_string(),
    json!(iteration.to_string()),
  );
  context.insert("urls".to_string(), json!(config.urls));
  context
    .insert("global".to_string(), json!(config.global));

  for item in benchmark.iter() {
    item
      .execute(&mut context, &mut reports, &pool, &config)
      .await;
  }

  reports
}

fn join<S: ToString>(l: Vec<S>, sep: &str) -> String {
  l.iter().fold(
    "".to_string(),
    |a,b| if !a.is_empty() {a+sep} else {a} + &b.to_string()
  )
}

pub fn execute(
  args: &FlattenedCli,
  tags: &Tags,
) -> BenchmarkResult {
  let env_contents = get_env_file(&args.benchmark_file);
  let config = Arc::new(Config::new(args, env_contents));

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
      println!(
        "{} {}",
        "Rampup".yellow(),
        config.rampup.to_string().purple()
      );
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

  let threads = std::cmp::min(
    num_cpus::get(),
    config.concurrency as usize,
  );
  let rt = runtime::Builder::new_multi_thread()
    .enable_all()
    .worker_threads(threads)
    .build()
    .unwrap();

  rt.block_on(async {
    let mut benchmark: Benchmark = Benchmark::new();
    let pool_store: PoolStore = PoolStore::new();

    walk(
      &args.benchmark_file,
      &mut benchmark,
      Some("plan"),
      tags,
    );

    if benchmark.is_empty() {
      eprintln!("Empty benchmark. Exiting.");
      std::process::exit(1);
    }

    let benchmark = Arc::new(benchmark);
    let pool = Arc::new(Mutex::new(pool_store));

    if let Some(ref report_path) = args.report_path_option {
      let reports = run_iteration(
        benchmark.clone(),
        pool.clone(),
        config,
        0,
      )
      .await;

      writer::write_file(report_path, join(reports, ""));

      BenchmarkResult {
        reports: vec![],
        duration: 0.0,
      }
    } else {
      let children =
        (0..config.iterations).map(|iteration| {
          run_iteration(
            benchmark.clone(),
            pool.clone(),
            config.clone(),
            iteration,
          )
        });

      let buffered = stream::iter(children)
        .buffer_unordered(config.concurrency as usize);

      let begin = Instant::now();
      let reports: Vec<Vec<Report>> =
        buffered.collect::<Vec<_>>().await;
      let duration = begin.elapsed().as_secs_f64();

      BenchmarkResult {
        reports,
        duration,
      }
    }
  })
}

fn get_env_file(
  benchmark_file: &str,
) -> BTreeMap<String, String> {
  let env_file =
    Path::new(benchmark_file).with_file_name(".env");
  if let Ok(true) = env_file.try_exists() {
    let mut buffer = String::new();
    if let Ok(mut file) = File::open(env_file) {
      if file.read_to_string(&mut buffer).is_err() {
        return BTreeMap::new();
      };
    }
    buffer
      .lines()
      .map(|s| {
        s.split_once('=')
          .map(|(k, v)| (k.to_owned(), v.to_owned()))
          .or_else(|| {
            let mut split = s.split_whitespace();
            Some((
              split
                .next()
                .expect(".env key before whitespace")
                .to_owned(),
              split
                .next()
                .expect(".env value after whitespace")
                .to_owned(),
            ))
          })
          .unwrap()
      })
      .collect::<BTreeMap<String, String>>()
  } else {
    BTreeMap::new()
  }
}

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use colored::Colorize;
use rand::seq::SliceRandom;
use rand::thread_rng;
use reqwest::{
  header::{self, HeaderMap, HeaderName, HeaderValue},
  ClientBuilder, Method, Response,
};
use std::fmt::Write;
use url::Url;
use yaml_rust::Yaml;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::actions::{extract, extract_optional};
use crate::benchmark::{Context, Pool, Reports};
use crate::config::Config;
use crate::parseable::Pick;
use crate::{interpolator, reader};

use crate::actions::{Report, Runnable};

use super::WithOps;

static USER_AGENT: &str = "drill";

#[derive(Clone)]
#[allow(dead_code)]
pub struct Request {
  name: String,
  base: Option<String>,
  url: String,
  time: f64,
  method: String,
  headers: HashMap<String, String>,
  pub body: Option<String>,
  pub with_items: Vec<Yaml>,
  pub shuffle: bool,
  pub pick: Pick,
  pub assign: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct AssignedRequest {
  status: u16,
  body: Value,
  headers: Map<String, Value>,
}

impl Request {
  pub fn new(name: String, assign: Option<String>, item: &Yaml, parent_path: &str) -> Request {
    let mut with_keys = item.as_hash().unwrap().keys().filter(|key| key.as_str().unwrap().contains("with"));
    if with_keys.clone().count() > 1 {
      panic!("{} 'with' attributes are mutually exclusive", "ERROR".yellow().bold());
    }

    let with_items = if let Some(key) = with_keys.next() {
      let op = WithOps::from(key.as_str().unwrap());
      match op {
        WithOps::Items => values(item),
        WithOps::Range => iter_values(item),
        WithOps::Csv => csv_values(parent_path, item),
        WithOps::File => file_values(parent_path, item),
      }
    } else {
      Vec::new()
    };

    let shuffle = item["shuffle"].as_bool().unwrap_or(false);
    let pick = Pick::new(item["pick"].as_i64().unwrap_or(with_items.len() as i64), &with_items);
    let base = extract_optional(&item, "base");
    let url = extract(&item, "url");

    let method = if let Some(v) = extract_optional(&item["request"], "method") {
      v.to_uppercase()
    } else {
      "GET".to_string()
    };

    let body_verbs = vec!["POST", "PATCH", "PUT"];
    let body = if body_verbs.contains(&method.as_str()) {
      Some(extract(&item["request"], "body"))
    } else {
      None
    };

    let mut headers = HashMap::new();

    if let Some(hash) = item["request"]["headers"].as_hash() {
      for (key, val) in hash.iter() {
        if let Some(vs) = val.as_str() {
          headers.insert(key.as_str().unwrap().to_string(), vs.to_string());
        } else {
          panic!("{} Headers must be strings!!", "WARNING!".yellow().bold());
        }
      }
    }

    Request {
      name,
      base,
      url,
      time: 0.0,
      method,
      headers,
      body,
      with_items,
      shuffle,
      pick,
      assign,
    }
  }

  fn format_time(tdiff: f64, nanosec: bool) -> String {
    if nanosec {
      (1_000_000.0 * tdiff).round().to_string() + "ns"
    } else {
      tdiff.round().to_string() + "ms"
    }
  }

  async fn send_request(&self, context: &mut Context, pool: &Pool, config: &Config, with_item: Option<&Yaml>) -> (Option<Response>, f64) {
    // Adding extra params as needed
    if let Some(val) = with_item {
      let map = val.as_hash().unwrap();
      for (key, val) in map {
        context.insert(key.clone().into_string().unwrap(), Value::String(val.clone().into_string().unwrap()));
      }
    }

    let interpolator = interpolator::Interpolator::new(context);

    // Resolve relative urls
    let interpolated_base_url = if let Some(base_url) = self.base.clone() {
      match context.get("urls") {
        Some(value) => {
          if let Some(url_map) = value.as_object() {
            let mut joined_url = PathBuf::from_str(url_map.get(&base_url).ok_or_else(|| format!("No such key in \"urls\" object: {}", base_url)).unwrap().as_str().unwrap()).unwrap();
            joined_url.push(self.url.clone());
            interpolator.resolve(joined_url.to_str().unwrap())
          } else {
            panic!("{} Wrong type for 'urls' variable.", "ERROR:".yellow().bold());
          }
        }
        _ => {
          panic!("{} Request '{}' references a non-existent base url named '{}'", "ERROR:".yellow().bold(), self.name.green(), base_url.magenta().bold());
        }
      }
    } else {
      interpolator.resolve(&self.url)
    };

    let url = Url::parse(&interpolated_base_url).expect("Invalid url");
    let domain = format!("{}://{}:{}", url.scheme(), url.host_str().unwrap(), url.port().unwrap_or(0)); // Unique domain key for keep-alive

    let interpolated_body;

    // Method
    let method = match self.method.to_uppercase().as_ref() {
      "GET" => Method::GET,
      "POST" => Method::POST,
      "PUT" => Method::PUT,
      "PATCH" => Method::PATCH,
      "DELETE" => Method::DELETE,
      "HEAD" => Method::HEAD,
      _ => panic!("Unknown method '{}'", self.method),
    };

    // Resolve the body
    let (client, request) = {
      let mut pool2 = pool.lock().unwrap();
      let client = pool2.entry(domain).or_insert_with(|| ClientBuilder::default().danger_accept_invalid_certs(config.no_check_certificate).build().unwrap());

      let request = if let Some(body) = self.body.as_ref() {
        interpolated_body = interpolator.resolve(body);

        client.request(method, interpolated_base_url.as_str()).body(interpolated_body)
      } else {
        client.request(method, interpolated_base_url.as_str())
      };

      (client.clone(), request)
    };

    // Headers
    let mut headers = HeaderMap::new();
    headers.insert(header::USER_AGENT, HeaderValue::from_str(USER_AGENT).unwrap());

    if let Some(cookies) = context.get("cookies") {
      let cookies: Map<String, Value> = serde_json::from_value(cookies.clone()).unwrap();
      let cookie = cookies.iter().map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join(";");

      headers.insert(header::COOKIE, HeaderValue::from_str(&cookie).unwrap());
    }

    // Resolve headers
    for (key, val) in self.headers.iter() {
      let interpolated_header = interpolator.resolve(val);
      headers.insert(HeaderName::from_bytes(key.as_bytes()).unwrap(), HeaderValue::from_str(&interpolated_header).unwrap());
    }

    let request_builder = request.headers(headers).timeout(Duration::from_secs(config.timeout));
    let request = request_builder.build().expect("Cannot create request");

    if config.verbose {
      log_request(&request);
    }

    let begin = Instant::now();
    let response_result = client.execute(request).await;
    let duration_ms = begin.elapsed().as_secs_f64() * 1000.0;

    match response_result {
      Err(e) => {
        if !config.quiet || config.verbose {
          println!("Error connecting '{}': {:?}", interpolated_base_url.as_str(), e);
        }
        (None, duration_ms)
      }
      Ok(response) => {
        if !config.quiet {
          let status = response.status();
          let status_text = if status.is_server_error() {
            status.to_string().red()
          } else if status.is_client_error() {
            status.to_string().purple()
          } else {
            status.to_string().yellow()
          };

          println!("{:width$} {} {} {}", self.name.green(), interpolated_base_url.blue().bold(), status_text, Request::format_time(duration_ms, config.nanosec).cyan(), width = 25);
        }

        (Some(response), duration_ms)
      }
    }
  }

  async fn execute_one_request(&self, context: &mut Context, pool: &Pool, config: &Config, reports: &mut Reports, with_item: Option<&Yaml>) {
    let (res, duration_ms) = self.send_request(context, pool, config, with_item).await;

    let log_message_response = if config.verbose {
      Some(log_message_response(&res, duration_ms))
    } else {
      None
    };

    match res {
      None => reports.push(Report {
        name: self.name.to_owned(),
        duration: duration_ms,
        status: 520u16,
      }),
      Some(response) => {
        let status = response.status().as_u16();

        reports.push(Report {
          name: self.name.to_owned(),
          duration: duration_ms,
          status,
        });

        for cookie in response.cookies() {
          let cookies = context.entry("cookies").or_insert_with(|| json!({})).as_object_mut().unwrap();
          cookies.insert(cookie.name().to_string(), json!(cookie.value().to_string()));
        }

        let data = if let Some(ref key) = self.assign {
          let mut headers = Map::new();

          response.headers().iter().for_each(|(header, value)| {
            headers.insert(header.to_string(), json!(value.to_str().unwrap()));
          });

          let data = response.text().await.unwrap();

          let body: Value = serde_json::from_str(&data).unwrap_or(serde_json::Value::Null);

          let assigned = AssignedRequest {
            status,
            body,
            headers,
          };

          let value = serde_json::to_value(assigned).unwrap();

          context.insert(key.to_owned(), value);

          Some(data)
        } else {
          None
        };

        if let Some(msg) = log_message_response {
          log_response(msg, &data)
        }
      }
    }
  }
}

#[async_trait]
impl Runnable for Request {
  async fn execute(&self, context: &mut Context, reports: &mut Reports, pool: &Pool, config: &Config) {
    if self.with_items.is_empty() {
      self.execute_one_request(context, pool, config, reports, None).await;
    } else {
      let mut with_items = self.with_items.clone();
      if self.shuffle {
        let mut rng = thread_rng();
        with_items.shuffle(&mut rng);
      }

      for with_item in with_items.iter().take(self.pick.inner()) {
        self.execute_one_request(context, pool, config, reports, Some(with_item)).await;
      }
    }
  }
}

fn csv_values(parent_path: &str, item: &Yaml) -> Vec<Yaml> {
  let (with_items_path, quote_char) = if let Some(with_items_path) = item["with_items_from_csv"].as_str() {
    (with_items_path, b'\"')
  } else if let Some(_with_items_hash) = item["with_items_from_csv"].as_hash() {
    let with_items_path = item["with_items_from_csv"]["file_name"].as_str().expect("Expected a file_name");
    let quote_char = item["with_items_from_csv"]["quote_char"].as_str().unwrap_or("\"").bytes().next().unwrap();

    (with_items_path, quote_char)
  } else {
    unreachable!();
  };

  let with_items_filepath = Path::new(parent_path).with_file_name(with_items_path);
  let final_path = with_items_filepath.to_str().unwrap();

  reader::read_csv_file_as_yml(final_path, quote_char)
}

fn file_values(parent_path: &str, item: &Yaml) -> Vec<Yaml> {
  let with_items_path = if let Some(with_items_path) = item["with_items_from_file"].as_str() {
    with_items_path
  } else {
    unreachable!();
  };

  let with_items_filepath = Path::new(parent_path).with_file_name(with_items_path);
  let final_path = with_items_filepath.to_str().unwrap();

  reader::read_file_as_yml_array(final_path)
}

fn iter_values(item: &Yaml) -> Vec<Yaml> {
  if let Some(with_iter_items) = item["with_items_range"].as_hash() {
    let init = Yaml::Integer(1);
    let lstart = Yaml::String("start".into());
    let lstep = Yaml::String("step".into());
    let lstop = Yaml::String("stop".into());

    let vstart: &Yaml = with_iter_items.get(&lstart).expect("Start property is mandatory");
    let vstep: &Yaml = with_iter_items.get(&lstep).unwrap_or(&init);
    let vstop: &Yaml = with_iter_items.get(&lstop).expect("Stop property is mandatory");

    let start: i64 = vstart.as_i64().expect("Start needs to be a number");
    let step: i64 = vstep.as_i64().expect("Step needs to be a number");
    let stop: i64 = vstop.as_i64().expect("Stop needs to be a number");

    let stop = stop + 1; // making stop inclusive

    if stop > start && start > 0 {
      (start..stop).step_by(step as usize).map(|int| Yaml::Integer(int)).collect()
    } else {
      Vec::new()
    }
  } else {
    Vec::new()
  }
}

pub fn values(item: &Yaml) -> Vec<Yaml> {
  item["with_items"].as_vec().unwrap().clone()
}

fn log_request(request: &reqwest::Request) {
  let mut message = String::new();
  write!(message, "{}", ">>>".bold().green()).unwrap();
  write!(message, " {} {},", "URL:".bold(), request.url()).unwrap();
  write!(message, " {} {},", "METHOD:".bold(), request.method()).unwrap();
  write!(message, " {} {:?}", "HEADERS:".bold(), request.headers()).unwrap();
  println!("{message}");
}

fn log_message_response(response: &Option<reqwest::Response>, duration_ms: f64) -> String {
  let mut message = String::new();
  match response {
    Some(response) => {
      write!(message, " {} {},", "URL:".bold(), response.url()).unwrap();
      write!(message, " {} {},", "STATUS:".bold(), response.status()).unwrap();
      write!(message, " {} {:?}", "HEADERS:".bold(), response.headers()).unwrap();
      write!(message, " {} {:.4} ms,", "DURATION:".bold(), duration_ms).unwrap();
    }
    None => {
      message = String::from("No response from server!");
    }
  }
  message
}

fn log_response(log_message_response: String, body: &Option<String>) {
  let mut message = String::new();
  write!(message, "{}{}", "<<<".bold().green(), log_message_response).unwrap();
  if let Some(body) = body.as_ref() {
    write!(message, " {} {:?}", "BODY:".bold(), body).unwrap()
  }
  println!("{message}");
}

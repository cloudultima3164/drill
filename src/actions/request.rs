use std::collections::HashMap;
use std::path::PathBuf;
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

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::benchmark::{Context, Pool, Reports};
use crate::config::Config;
use crate::interpolator;
use crate::parse::{Pick, WithItems};

use crate::actions::{Report, Runnable};

static USER_AGENT: &str = "drill";

#[derive(Clone)]
#[allow(dead_code)]
pub struct Request {
  name: String,
  base: Option<String>,
  url: String,
  _time: f64,
  method: String,
  headers: HashMap<String, String>,
  body: Option<String>,
  with_items: Option<Vec<serde_yaml::Value>>,
  shuffle: Option<bool>,
  pick: Option<Pick>,
  assign: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct AssignedRequest {
  status: u16,
  body: Value,
  headers: Map<String, Value>,
}

impl Request {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    name: String,
    base: Option<String>,
    url: String,
    _time: f64,
    method: String,
    headers: HashMap<String, String>,
    body: Option<String>,
    with_items: Option<WithItems>,
    assign: Option<String>,
  ) -> Self {
    let shuffle = with_items.as_ref().map(|wi| wi.shuffle);
    let pick = with_items.as_ref().map(|wi| wi.pick);
    let with_items = with_items.map(|wi| wi.items);

    Self {
      name,
      base,
      url,
      _time,
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

  async fn send_request(
    &self,
    context: &mut Context,
    pool: &Pool,
    config: &Config,
    with_item: Option<&serde_yaml::Value>,
  ) -> (Option<Response>, f64) {
    // Adding extra params as needed
    if let Some(val) = with_item {
      let map = val.as_mapping().unwrap();
      for (key, val) in map {
        context.insert(
          key.clone().as_str().unwrap().to_owned(),
          serde_json::Value::String(val.clone().as_str().unwrap().to_owned()),
        );
      }
    }

    let interpolator = interpolator::Interpolator::new(context);

    // Resolve relative urls
    let interpolated_base_url = if let Some(base_url) = self.base.clone() {
      match context.get("urls") {
        Some(value) => {
          if let Some(url_map) = value.as_object() {
            let mut joined_url = PathBuf::from_str(
              url_map
                .get(&base_url)
                .unwrap_or_else(|| {
                  panic!("No such key in \"urls\" object: {}", base_url)
                })
                .as_str()
                .unwrap(),
            )
            .unwrap();
            joined_url.push(self.url.clone());
            interpolator.resolve(joined_url.to_str().unwrap())
          } else {
            panic!(
              "{} Wrong type for 'urls' variable.",
              "ERROR:".yellow().bold()
            );
          }
        }
        _ => {
          panic!(
            "{} Request '{}' references a non-existent base url named '{}'",
            "ERROR:".yellow().bold(),
            self.name.green(),
            base_url.magenta().bold()
          );
        }
      }
    } else {
      interpolator.resolve(&self.url)
    };

    let url = Url::parse(&interpolated_base_url).expect("Invalid url");
    let domain = format!(
      "{}://{}:{}",
      url.scheme(),
      url.host_str().unwrap(),
      url.port().unwrap_or(0)
    ); // Unique domain key for keep-alive

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
      let client = pool2.entry(domain).or_insert_with(|| {
        ClientBuilder::default()
          .danger_accept_invalid_certs(config.no_check_certificate)
          .build()
          .unwrap()
      });

      let request = if let Some(body) = self.body.as_ref() {
        interpolated_body = interpolator.resolve(body);

        client
          .request(method, interpolated_base_url.as_str())
          .body(interpolated_body)
      } else {
        client.request(method, interpolated_base_url.as_str())
      };

      (client.clone(), request)
    };

    // Headers
    let mut headers = HeaderMap::new();
    headers
      .insert(header::USER_AGENT, HeaderValue::from_str(USER_AGENT).unwrap());

    if let Some(cookies) = context.get("cookies") {
      let cookies: Map<String, Value> =
        serde_json::from_value(cookies.clone()).unwrap();
      let cookie = cookies
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(";");

      headers.insert(header::COOKIE, HeaderValue::from_str(&cookie).unwrap());
    }

    // Resolve headers
    for (key, val) in self.headers.iter() {
      let interpolated_header = interpolator.resolve(val);
      headers.insert(
        HeaderName::from_bytes(key.as_bytes()).unwrap(),
        HeaderValue::from_str(&interpolated_header).unwrap(),
      );
    }

    let request_builder =
      request.headers(headers).timeout(Duration::from_secs(config.timeout));
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
          println!(
            "Error connecting '{}': {:?}",
            interpolated_base_url.as_str(),
            e
          );
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

          println!(
            "{:width$} {} {} {}",
            self.name.green(),
            interpolated_base_url.blue().bold(),
            status_text,
            Request::format_time(duration_ms, config.nanosec).cyan(),
            width = 25
          );
        }

        (Some(response), duration_ms)
      }
    }
  }

  async fn execute_one_request(
    &self,
    context: &mut Context,
    pool: &Pool,
    config: &Config,
    reports: &mut Reports,
    with_item: Option<&serde_yaml::Value>,
  ) {
    let (res, duration_ms) =
      self.send_request(context, pool, config, with_item).await;

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
          let cookies = context
            .entry("cookies")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .unwrap();
          cookies.insert(
            cookie.name().to_string(),
            json!(cookie.value().to_string()),
          );
        }

        let data = if let Some(key) = &self.assign {
          let mut headers = Map::new();

          response.headers().iter().for_each(|(header, value)| {
            headers.insert(header.to_string(), json!(value.to_str().unwrap()));
          });

          let data = response.text().await.unwrap();

          let body: Value =
            serde_json::from_str(&data).unwrap_or(serde_json::Value::Null);

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
  async fn execute(
    &self,
    context: &mut Context,
    reports: &mut Reports,
    pool: &Pool,
    config: &Config,
  ) {
    if let Some(with_items) =
      self.with_items.clone().filter(|vec| !vec.is_empty())
    {
      let mut with_items = with_items.clone();
      if self.shuffle.unwrap() {
        let mut rng = thread_rng();
        with_items.shuffle(&mut rng);
      }
      let take = if self.pick.unwrap().inner() == 0 {
        with_items.len()
      } else {
        self.pick.unwrap().inner()
      };
      for with_item in with_items.iter().take(take) {
        self
          .execute_one_request(context, pool, config, reports, Some(with_item))
          .await;
      }
    } else {
      self.execute_one_request(context, pool, config, reports, None).await;
    }
  }
}

fn log_request(request: &reqwest::Request) {
  let mut message = String::new();
  write!(message, "{}", ">>>".bold().green()).unwrap();
  write!(message, " {} {},", "URL:".bold(), request.url()).unwrap();
  write!(message, " {} {},", "METHOD:".bold(), request.method()).unwrap();
  write!(message, " {} {:?}", "HEADERS:".bold(), request.headers()).unwrap();
  println!("{message}");
}

fn log_message_response(
  response: &Option<reqwest::Response>,
  duration_ms: f64,
) -> String {
  let mut message = String::new();
  match response {
    Some(response) => {
      write!(message, " {} {},", "URL:".bold(), response.url()).unwrap();
      write!(message, " {} {},", "STATUS:".bold(), response.status()).unwrap();
      write!(message, " {} {:?}", "HEADERS:".bold(), response.headers())
        .unwrap();
      write!(message, " {} {:.4} ms,", "DURATION:".bold(), duration_ms)
        .unwrap();
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

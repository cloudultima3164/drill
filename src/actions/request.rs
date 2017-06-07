use std::collections::HashMap;
use std::io::Read;

extern crate yaml_rust;
use self::yaml_rust::Yaml;

extern crate colored;
use self::colored::*;

extern crate serde_json;
use self::serde_json::Value;

extern crate hyper;
extern crate hyper_native_tls;
use self::hyper::client::{Client, Response};
use self::hyper::net::HttpsConnector;
use self::hyper_native_tls::NativeTlsClient;
use self::hyper::header::UserAgent;

extern crate time;

use interpolator;

use actions::Runnable;

static USER_AGENT: &'static str = "woodpecker";

#[derive(Clone)]
pub struct Request {
  name: String,
  url: String,
  time: f64,
  pub with_item: Option<Yaml>,
  pub assign: Option<String>,
}

impl Request {
  pub fn is_that_you(item: &Yaml) -> bool{
    item["request"].as_hash().is_some()
  }

  pub fn new(item: &Yaml, with_item: Option<Yaml>) -> Request {
    let reference: Option<&str> = item["assign"].as_str();

    Request {
      name: item["name"].as_str().unwrap().to_string(),
      url: item["request"]["url"].as_str().unwrap().to_string(),
      time: 0.0,
      with_item: with_item,
      assign: reference.map(str::to_string)
    }
  }

  fn send_request(&self, url: &str) -> (Response, f64) {
    let ssl = NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    let client = Client::with_connector(connector);

    let begin = time::precise_time_s();

    let response = client.get(url)
        .header(UserAgent(USER_AGENT.to_string()))
        .send();

    if let Err(e) = response {
      panic!("Error connecting '{}': {:?}", url, e);
    }

    (response.unwrap(), time::precise_time_s() - begin)
  }
}

impl Runnable for Request {
  fn execute(&self, base_url: &String, context: &mut HashMap<String, Yaml>, responses: &mut HashMap<String, Value>) {
    if self.with_item.is_some() {
      context.insert("item".to_string(), self.with_item.clone().unwrap());
    }

    let final_url;

    // Resolve the url
    {
      let interpolator = interpolator::Interpolator::new(&base_url, &context, &responses);
      final_url = interpolator.resolve(&self.url);
    }

    let (mut response, duration) = self.send_request(&final_url);

    println!("{:width$} {} {} {}{}", self.name.green(), final_url.blue().bold(), response.status.to_string().yellow(), (duration * 1000.0).round().to_string().cyan(), "ms".cyan(), width=25);

    if let Some(ref key) = self.assign {
      let mut data = String::new();

      response.read_to_string(&mut data).unwrap();

      let value: Value = serde_json::from_str(&data).unwrap();

      responses.insert(key.to_owned(), value);
    }
  }

}
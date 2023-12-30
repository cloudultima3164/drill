use async_trait::async_trait;
use colored::*;

use crate::actions::Runnable;
use crate::benchmark::{Context, Pool, Reports};
use crate::config::Config;
use crate::interpolator;

#[derive(Clone)]
pub struct Assert {
  name: String,
  key: String,
  value: serde_json::Value,
}

impl Assert {
  pub fn new(name: String, key: String, value: serde_json::Value) -> Self {
    Self {
      name,
      key,
      value,
    }
  }
}

#[async_trait]
impl Runnable for Assert {
  async fn execute(
    &self,
    context: &mut Context,
    _reports: &mut Reports,
    _pool: &Pool,
    config: &Config,
  ) {
    let interpolator = interpolator::Interpolator::new(context);
    let eval = format!("{{{{ {} }}}}", &self.key);

    let lhs = &self.value;
    let rhs = interpolator.resolve(&eval);

    if !config.quiet {
      println!(
        "{:width$} {}={}",
        self.name.green(),
        self.key.cyan().bold(),
        serde_json::to_string(&self.value).unwrap().magenta(),
        width = 25
      );
    }

    if !eq(lhs, rhs.clone(), &interpolator) {
      panic!("Assertion mismatched: {} != {}", lhs, rhs);
    }

    if !config.quiet {
      println!("{:width$}", "Assertion successful".red(), width = 25);
    }
  }
}

fn eq(
  lhs: &serde_json::Value,
  rhs: String,
  interpolator: &interpolator::Interpolator,
) -> bool {
  match lhs {
    serde_json::Value::Null => panic!("Can't compare null values!"),
    serde_json::Value::Bool(b) => b.eq(&rhs.parse::<bool>().unwrap()),
    serde_json::Value::Number(n) => {
      n.as_f64().unwrap().eq(&rhs.parse::<f64>().unwrap())
    }
    serde_json::Value::String(s) => interpolator.resolve(s).eq(&rhs),
    serde_json::Value::Array(arr) => {
      let deser_rhs = serde_json::from_str::<Vec<String>>(&rhs).unwrap();
      arr
        .iter()
        .zip(deser_rhs)
        .map(|(lhs, rhs)| eq(lhs, rhs, interpolator))
        .all(|b| b)
    }
    serde_json::Value::Object(ob) => {
      let deser_rhs = serde_json::from_str::<
        serde_json::Map<String, serde_json::Value>,
      >(&rhs)
      .unwrap();
      ob.iter()
        .zip(deser_rhs)
        .map(|(lhs, rhs)| {
          [
            lhs.0.eq(&rhs.0),
            eq(lhs.1, serde_json::to_string(&rhs.1).unwrap(), interpolator),
          ]
          .iter()
          .all(|b| *b)
        })
        .all(|b| b)
    }
  }
}

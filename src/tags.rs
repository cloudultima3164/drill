use crate::reader;
use colored::*;
use std::collections::HashSet;

#[derive(Debug)]
pub struct Tags {
  pub tags: HashSet<String>,
  pub skip_tags: HashSet<String>,
}

impl Tags {
  pub fn new(tags_option: Vec<String>, skip_tags_option: Vec<String>) -> Self {
    let tags: HashSet<String> = tags_option.into_iter().collect();
    let skip_tags: HashSet<String> = skip_tags_option.into_iter().collect();

    if !tags.is_disjoint(&skip_tags) {
      panic!("`tags` and `skip-tags` must not contain the same values!");
    }

    Tags {
      tags,
      skip_tags,
    }
  }

  pub fn should_skip_item(&self, item: &serde_yaml::Value) -> bool {
    match item.as_mapping().unwrap().get("tags").unwrap().as_sequence() {
      Some(item_tags_raw) => {
        if item_tags_raw.is_empty() {
          return false;
        }

        let item_tags: HashSet<String> = item_tags_raw
          .iter()
          .map(|t| t.clone().as_str().unwrap().to_owned())
          .collect();

        if !self.skip_tags.is_disjoint(&item_tags) {
          return true;
        }

        if item_tags.contains("never") && !self.tags.contains("never") {
          return true;
        }
        if !self.tags.is_disjoint(&item_tags) {
          return false;
        }

        if item_tags.contains("always") {
          return false;
        }
        if item_tags.contains("never") {
          return true;
        }
        true
      }
      None => false,
    }
  }
}

pub fn list_benchmark_file_tasks(benchmark_file: &str, tags: &Tags) {
  let docs = reader::read_file_as_yml(benchmark_file);
  let items = reader::read_yaml_doc_accessor(&docs[0], "plan");

  println!();

  let mut include_tags: Vec<_> = tags.tags.iter().collect();
  include_tags.sort();
  println!(
    "{:width$} {:width2$?}",
    "Tags".green(),
    &tags,
    width = 15,
    width2 = 25
  );

  let mut skip_tags: Vec<_> = tags.skip_tags.iter().collect();
  skip_tags.sort();
  println!(
    "{:width$} {:width2$?}",
    "Skip-Tags".green(),
    &tags,
    width = 15,
    width2 = 25
  );

  let items: Vec<_> = items
    .as_sequence()
    .unwrap()
    .iter()
    .filter(|item| !tags.should_skip_item(item))
    .collect();

  if items.is_empty() {
    println!("{}", "No items".red());
    std::process::exit(1)
  }

  println!("{}", serde_yaml::to_string(&items).unwrap())
}

pub fn list_benchmark_file_tags(benchmark_file: &str) {
  let docs = reader::read_file_as_yml(benchmark_file);
  let items =
    reader::read_yaml_doc_accessor(&docs[0], "plan").as_sequence().unwrap();

  println!();

  if items.is_empty() {
    println!("{}", "No items".red());
    std::process::exit(1)
  }
  let mut tags: HashSet<&str> = HashSet::new();
  for item in items {
    if let Some(item_tags_raw) = item.get("tags") {
      let item_tags_raw = item_tags_raw.as_sequence().unwrap();
      tags.extend(item_tags_raw.iter().map(|t| t.as_str().unwrap()));
    }
  }

  let mut tags: Vec<_> = tags.into_iter().collect();
  tags.sort_unstable();
  println!("{:width$} {:?}", "Tags".green(), &tags, width = 15);
}

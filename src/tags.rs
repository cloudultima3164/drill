use crate::reader;
use colored::*;
use std::collections::HashSet;
use yaml_rust::{Yaml, YamlEmitter};

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

  pub fn should_skip_item(&self, item: &Yaml) -> bool {
    match item["tags"].as_vec() {
      Some(item_tags_raw) => {
        if item_tags_raw.is_empty() {
          return false;
        }

        let item_tags: HashSet<String> = item_tags_raw.iter().map(|t| t.clone().into_string().unwrap()).collect();

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
  let items = reader::read_yaml_doc_accessor(&docs[0], Some("plan"));

  println!();

  let mut include_tags: Vec<_> = tags.tags.iter().collect();
  include_tags.sort();
  println!("{:width$} {:width2$?}", "Tags".green(), &tags, width = 15, width2 = 25);

  let mut skip_tags: Vec<_> = tags.skip_tags.iter().collect();
  skip_tags.sort();
  println!("{:width$} {:width2$?}", "Skip-Tags".green(), &tags, width = 15, width2 = 25);

  let items: Vec<_> = items.iter().filter(|item| !tags.should_skip_item(item)).collect();

  if items.is_empty() {
    println!("{}", "No items".red());
    std::process::exit(1)
  }

  for item in items {
    let mut out_str = String::new();
    let mut emitter = YamlEmitter::new(&mut out_str);
    emitter.dump(item).unwrap();
    println!("{out_str}");
  }
}

pub fn list_benchmark_file_tags(benchmark_file: &str) {
  let docs = reader::read_file_as_yml(benchmark_file);
  let items = reader::read_yaml_doc_accessor(&docs[0], Some("plan"));

  println!();

  if items.is_empty() {
    println!("{}", "No items".red());
    std::process::exit(1)
  }
  let mut tags: HashSet<&str> = HashSet::new();
  for item in items {
    if let Some(item_tags_raw) = item["tags"].as_vec() {
      tags.extend(item_tags_raw.iter().map(|t| t.as_str().unwrap()));
    }
  }

  let mut tags: Vec<_> = tags.into_iter().collect();
  tags.sort_unstable();
  println!("{:width$} {:?}", "Tags".green(), &tags, width = 15);
}

#[cfg(test)]
mod tests {
  use super::*;

  fn str_to_yaml(text: &str) -> Yaml {
    let mut docs = yaml_rust::YamlLoader::load_from_str(text).unwrap();
    docs.remove(0)
  }

  fn prepare_default_item() -> Yaml {
    str_to_yaml("---\nname: foo\nrequest:\n  url: /\ntags:\n  - tag1\n  - tag2")
  }

  #[test]
  #[should_panic]
  fn same_tags_and_skip_tags() {
    let _ = Tags::new(vec!["tag1".into()], vec!["tag1".into()]);
  }

  #[test]
  fn empty_tags_both() {
    let item = str_to_yaml("---\nname: foo\nrequest:\n  url: /");
    let tags = Tags::new(vec![], vec![]);
    assert!(!tags.should_skip_item(&item));
  }

  #[test]
  fn empty_tags() {
    let tags = Tags::new(vec![], vec![]);
    assert!(!tags.should_skip_item(&prepare_default_item()));
  }

  #[test]
  fn tags_contains() {
    let tags = Tags::new(vec!["tag1".into()], vec![]);
    assert!(!tags.should_skip_item(&prepare_default_item()));
  }

  #[test]
  fn tags_contains_second() {
    let tags = Tags::new(vec!["tag2".into()], vec![]);
    assert!(!tags.should_skip_item(&prepare_default_item()));
  }

  #[test]
  fn tags_contains_both() {
    let tags = Tags::new(vec!["tag1".into(), "tag2".into()], vec![]);
    assert!(!tags.should_skip_item(&prepare_default_item()));
  }

  #[test]
  fn tags_not_contains() {
    let tags = Tags::new(vec!["tag99".into()], vec![]);
    assert!(tags.should_skip_item(&prepare_default_item()));
  }

  #[test]
  fn skip_tags_not_contains() {
    let tags = Tags::new(vec![], vec!["tag99".into()]);
    assert!(!tags.should_skip_item(&prepare_default_item()));
  }

  #[test]
  fn skip_tags_contains() {
    let tags = Tags::new(vec![], vec!["tag1".into()]);
    assert!(tags.should_skip_item(&prepare_default_item()));
  }

  #[test]
  fn skip_tags_contains_second() {
    let tags = Tags::new(vec![], vec!["tag2".into()]);
    assert!(tags.should_skip_item(&prepare_default_item()));
  }

  #[test]
  fn tags_contains_but_also_skip_tags_contains() {
    let tags = Tags::new(vec!["tag1".into()], vec!["tag2".into()]);
    assert!(tags.should_skip_item(&prepare_default_item()));
  }

  #[test]
  fn never_skipped_by_default() {
    let item = str_to_yaml("---\nname: foo\nrequest:\n  url: /\ntags:\n  - never\n  - tag2");
    let tags = Tags::new(vec![], vec![]);
    assert!(tags.should_skip_item(&item));
  }

  #[test]
  fn never_tag_skipped_even_when_other_tag_included() {
    let item = str_to_yaml("---\nname: foo\nrequest:\n  url: /\ntags:\n  - never\n  - tag2");
    let tags = Tags::new(vec!["tag2".into()], vec![]);
    assert!(tags.should_skip_item(&item));
  }

  #[test]
  fn include_never_tag() {
    let item = str_to_yaml("---\nname: foo\nrequest:\n  url: /\ntags:\n  - never\n  - tag2");
    let tags = Tags::new(vec!["never".into()], vec![]);
    assert!(!tags.should_skip_item(&item));
  }

  #[test]
  fn always_tag_included_by_default() {
    let item = str_to_yaml("---\nname: foo\nrequest:\n  url: /\ntags:\n  - always\n  - tag2");
    let tags = Tags::new(vec!["tag99".into()], vec![]);
    assert!(!tags.should_skip_item(&item));
  }

  #[test]
  fn skip_always_tag() {
    let item = str_to_yaml("---\nname: foo\nrequest:\n  url: /\ntags:\n  - always\n  - tag2");
    let tags = Tags::new(vec![], vec!["always".into()]);
    assert!(tags.should_skip_item(&item));
  }
}

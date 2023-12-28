use std::path::Path;
use yaml_rust::Yaml;

use crate::interpolator::INTERPOLATION_REGEX;

use crate::actions::{extract, extract_optional};
use crate::benchmark::Benchmark;
use crate::parseable::include;
use crate::tags::Tags;

use crate::reader;

use super::parse::IncludeOp;

pub fn is_that_you(item: &Yaml) -> bool {
  item["include"].as_str().is_some()
}

pub fn parse(parent_path: &str, item: &Yaml, benchmark: &mut Benchmark, tags: &Tags) {
  let include_path = item["include"].as_str().unwrap();

  if INTERPOLATION_REGEX.is_match(include_path) {
    panic!("Interpolations not supported in 'include' property!");
  }

  let include_filepath = Path::new(parent_path).with_file_name(include_path);
  let final_path = include_filepath.to_str().unwrap();

  parse_from_filepath(final_path, benchmark, None, tags);
}

const IGNORE_KEYS: [&str; 3] = ["name", "assign", "tags"];
pub fn parse_from_filepath(parent_path: &str, benchmark: &mut Benchmark, accessor: Option<&str>, tags: &Tags) {
  let docs = reader::read_file_as_yml(parent_path);
  let items = reader::read_yaml_doc_accessor(&docs[0], accessor);

  for item in items {
    if include::is_that_you(item) {
      include::parse(parent_path, item, benchmark, tags);

      continue;
    }

    if tags.should_skip_item(item) {
      continue;
    }

    let (target_key, target_value) = item.as_hash().unwrap().into_iter().filter(|(key, _)| !IGNORE_KEYS.contains(&key.as_str().unwrap())).next().unwrap();
    let op = IncludeOp::from(target_key.as_str().unwrap());
    let name = extract(item, "name");
    let assign = extract_optional(item, "assign");
    op.parse(target_value, benchmark, name, assign, Some(parent_path))
  }
}

#[cfg(test)]
mod tests {
  use crate::benchmark::Benchmark;
  use crate::parseable::include::{is_that_you, parse};
  use crate::tags::Tags;

  #[test]
  fn parse_include() {
    let text = "---\nname: Include comment\ninclude: comments.yml";
    let docs = yaml_rust::YamlLoader::load_from_str(text).unwrap();
    let doc = &docs[0];
    let mut benchmark: Benchmark = Benchmark::new();

    parse("example/benchmark.yml", doc, &mut benchmark, &Tags::new(None, None));

    assert!(is_that_you(doc));
    assert_eq!(benchmark.len(), 2);
  }

  #[test]
  #[should_panic]
  fn invalid_parse() {
    let text = "---\nname: Include comment\ninclude: {{ memory }}.yml";
    let docs = yaml_rust::YamlLoader::load_from_str(text).unwrap();
    let doc = &docs[0];
    let mut benchmark: Benchmark = Benchmark::new();

    parse("example/benchmark.yml", doc, &mut benchmark, &Tags::new(None, None));
  }
}

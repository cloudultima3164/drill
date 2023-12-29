mod op;
mod parsers;

use std::{convert::TryFrom, marker::PhantomData};

use yaml_rust::Yaml;

use crate::{
  actions::{extract, extract_optional},
  benchmark::Benchmark,
  reader,
  tags::Tags,
};

use self::{
  op::Op,
  parsers::{
    AssertArgs, AssertParser, AssignArgs, AssignParser,
    DbQueryArgs, DbQueryParser, DelayArgs, DelayParser,
    ExecArgs, ExecParser, IncludeArgs, IncludeParser,
    RequestArgs, RequestParser,
  },
};

pub fn walk(
  parent_path: &str,
  benchmark: &mut Benchmark,
  accessor: Option<&str>,
  tags: &Tags,
) {
  let docs = reader::read_file_as_yml(parent_path);
  let items =
    reader::read_yaml_doc_accessor(&docs[0], accessor);

  for item in items {
    if tags.should_skip_item(item) {
      continue;
    }

    let name = extract(item, "name");
    let assign = extract_optional(item, "assign");

    let parser_args =
      ParserArgs::new(name, assign, parent_path);

    let mut op_iter = item
      .as_hash()
      .unwrap()
      .into_iter()
      .filter_map(|(key, val)| {
        Op::try_from(key.as_str().unwrap())
          .ok()
          .map(|parsed_key| (parsed_key, val))
      })
      .rev();

    let (op, target_value) = if op_iter.clone().count() > 1
    {
      op_iter.find_map(|(op, val)| {
        op.ne(&Op::Assign).then_some((op, val))
      })
    } else {
      op_iter.next()
    }
    .expect("No actionable keys found in item");

    parse(op, target_value, benchmark, parser_args, tags)
  }
}

fn parse<'a>(
  op: Op,
  item: &'a Yaml,
  benchmark: &'a mut Benchmark,
  args: ParserArgs<'a>,
  tags: &'a Tags,
) {
  match op {
    Op::Include => Parser::new(
      IncludeParser::with_state(tags),
      IncludeArgs::from(args),
    )
    .parse(item, benchmark),
    Op::Assert => {
      Parser::new(AssertParser, AssertArgs::from(args))
        .parse(item, benchmark)
    }
    Op::Assign => {
      Parser::new(AssignParser, AssignArgs::from(args))
        .parse(item, benchmark)
    }
    Op::Delay => {
      Parser::new(DelayParser, DelayArgs::from(args))
        .parse(item, benchmark)
    }
    Op::Exec => {
      Parser::new(ExecParser, ExecArgs::from(args))
        .parse(item, benchmark)
    }
    Op::Request => Parser::new(
      RequestParser(PhantomData),
      RequestArgs::from(args),
    )
    .parse(item, benchmark),
    Op::DbQuery => {
      Parser::new(DbQueryParser, DbQueryArgs::from(args))
        .parse(item, benchmark)
    }
  }
}

pub trait Parse: Send + Sync {
  type Args;

  fn parse(
    &self,
    item: &Yaml,
    benchmark: &mut Benchmark,
    args: Self::Args,
  );
}

struct Parser<'a, P: Parse> {
  inner: P,
  args: <P as Parse>::Args,
  _marker: PhantomData<ParserArgs<'a>>,
}

impl<'a, P: Parse> Parser<'a, P> {
  fn new(inner: P, args: <P as Parse>::Args) -> Self {
    Self {
      inner,
      args,
      _marker: PhantomData,
    }
  }

  fn parse(self, item: &Yaml, benchmark: &mut Benchmark) {
    self.inner.parse(item, benchmark, self.args)
  }
}

pub struct ParserArgs<'a> {
  name: String,
  assign: Option<String>,
  parent_path: &'a str,
}

impl<'a> ParserArgs<'a> {
  fn new(
    name: String,
    assign: Option<String>,
    parent_path: &'a str,
  ) -> Self {
    Self {
      name,
      assign,
      parent_path,
    }
  }
}

#[cfg(test)]
mod test {
  use crate::benchmark::Benchmark;
  use crate::parse::parsers::IncludeArgs;
  use crate::parse::{IncludeParser, Parser};
  use crate::tags::Tags;

  #[test]
  fn parse_include() {
    let text =
      "---\nname: Include comment\ninclude: comments.yml";
    let docs =
      yaml_rust::YamlLoader::load_from_str(text).unwrap();
    let doc = &docs[0];
    let mut benchmark: Benchmark = Benchmark::new();
    let tags = Tags::new(vec![], vec![]);

    Parser::new(
      IncludeParser::with_state(&tags),
      IncludeArgs::new("example/benchmark.yml"),
    )
    .parse(doc, &mut benchmark);
    assert_eq!(benchmark.len(), 2);
  }

  // #[test]
  // #[should_panic]
  // fn invalid_parse() {
  //   let text = "---\nname: Include comment\ninclude: {{ memory }}.yml";
  //   let docs = yaml_rust::YamlLoader::load_from_str(text).unwrap();
  //   let doc = &docs[0];
  //   let mut benchmark: Benchmark = Benchmark::new();

  //   walk(doc, &mut benchmark, &Tags::new(vec![], vec![]));
  // }
}

// #[cfg(test)]
// mod tests {
//   use super::*;

//   mod pick {
//     use super::*;

//     #[test]
//     fn should_return_the_configured_value() {
//       let text = "---\nname: foobar\nrequest:\n  url: /api/{{ item }}\npick: 2\nwith_items:\n  - 1\n  - 2\n  - 3";
//       let item = &yaml_rust::YamlLoader::load_from_str(text).unwrap()[0];
//       let pick = validate_pick(item, item["with_items"].as_vec().unwrap());

//       assert_eq!(pick, 2);
//     }

//     #[test]
//     fn should_return_the_with_items_length_if_unset() {
//       let text = "---\nname: foobar\nrequest:\n  url: /api/{{ item }}\nwith_items:\n  - 1\n  - 2\n  - 3";
//       let item = &yaml_rust::YamlLoader::load_from_str(text).unwrap()[0];
//       let pick = validate_pick(item, item["with_items"].as_vec().unwrap());

//       assert_eq!(pick, 3);
//     }

//     #[test]
//     #[should_panic(expected = "pick option should not be negative, but was -1")]
//     fn should_panic_for_negative_values() {
//       let text = "---\nname: foobar\nrequest:\n  url: /api/{{ item }}\npick: -1\nwith_items:\n  - 1\n  - 2\n  - 3";
//       let item = &yaml_rust::YamlLoader::load_from_str(text).unwrap()[0];
//       validate_pick(item, item["with_items"].as_vec().unwrap());
//     }

//     #[test]
//     #[should_panic(expected = "pick option should not be greater than the provided items, but was 4")]
//     fn should_panic_for_values_greater_than_the_items_list() {
//       let text = "---\nname: foobar\nrequest:\n  url: /api/{{ item }}\npick: 4\nwith_items:\n  - 1\n  - 2\n  - 3";
//       let item = &yaml_rust::YamlLoader::load_from_str(text).unwrap()[0];
//       validate_pick(item, item["with_items"].as_vec().unwrap());
//     }
//   }
// }

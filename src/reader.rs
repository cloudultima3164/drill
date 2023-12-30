use std::ffi::OsStr;
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::Path;

pub fn get_file<S: AsRef<OsStr> + ?Sized>(filepath: &S) -> File {
  // Create a path to the desired file
  let path = Path::new(filepath);
  let display = path.display();

  // Open the path in read-only mode, returns `io::Result<File>`
  match File::open(path) {
    Err(why) => {
      panic!("couldn't open {}: {}", display, why)
    }
    Ok(file) => file,
  }
}

#[allow(dead_code)]
pub fn read_file<S: AsRef<OsStr> + ?Sized>(filepath: &S) -> String {
  let mut file = get_file(filepath);

  // Read the file contents into a string, returns `io::Result<usize>`
  let mut content = String::new();
  if let Err(why) = file.read_to_string(&mut content) {
    panic!("couldn't read {}: {}", filepath.as_ref().to_string_lossy(), why);
  }

  content
}

pub fn read_file_as_yml<S: AsRef<OsStr> + ?Sized>(
  filepath: &S,
) -> serde_yaml::Value {
  serde_yaml::from_reader(get_file(filepath)).unwrap()
}

pub fn read_yaml_doc_accessor<'a>(
  doc: &'a serde_yaml::Value,
  accessor: &str,
) -> &'a serde_yaml::Value {
  match doc.get(accessor) {
    Some(items) => items,
    None => {
      println!("Node missing on config: {accessor}");
      println!("Exiting.");
      std::process::exit(1)
    }
  }
}

#[allow(dead_code)]
pub fn read_file_as_yml_array<S: AsRef<OsStr> + ?Sized>(
  filepath: &S,
) -> Vec<serde_yaml::Value> {
  let file = get_file(filepath);
  let reader = BufReader::new(file);
  serde_yaml::from_reader(reader).unwrap()
}

pub fn read_csv_file_as_yml<S: AsRef<OsStr> + ?Sized>(
  filepath: &S,
  // quote: u8,
) -> Vec<serde_yaml::Value> {
  let file = get_file(filepath);

  let mut rdr = csv::ReaderBuilder::new()
    .has_headers(true)
    // .quote(quote)
    .from_reader(file);

  let mut items = Vec::new();

  let headers = match rdr.headers() {
    Err(why) => panic!("error parsing header: {:?}", why),
    Ok(h) => h.clone(),
  };

  for result in rdr.records() {
    match result {
      Ok(record) => {
        let yaml_record = headers
          .iter()
          .enumerate()
          .map(|(i, header)| {
            (
              serde_yaml::Value::String(header.to_string()),
              serde_yaml::Value::String(record.get(i).unwrap().to_string()),
            )
          })
          .collect();

        items.push(serde_yaml::Value::Mapping(yaml_record));
      }
      Err(e) => println!("error parsing header: {e:?}"),
    }
  }

  items
}

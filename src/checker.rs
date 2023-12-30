use colored::*;

use crate::actions::Report;
use crate::reader::get_file;

pub fn compare(
  list_reports: &[Vec<Report>],
  filepath: &str,
  threshold: &str,
) -> Result<(), i32> {
  let threshold_value = match threshold.parse::<f64>() {
    Ok(v) => v,
    _ => panic!("arrrgh"),
  };

  let file = get_file(filepath);

  let docs: Vec<serde_yaml::Value> = serde_yaml::from_reader(file).unwrap();
  let doc = &docs[0];
  let items = doc.as_sequence().unwrap();
  let mut slow_counter = 0;

  println!();

  for report in list_reports {
    for (i, report_item) in report.iter().enumerate() {
      let recorded_duration = items[i]["duration"].as_f64().unwrap();
      let delta_ms = report_item.duration - recorded_duration;

      if delta_ms > threshold_value {
        println!(
          "{:width$} is {}{} slower than before",
          report_item.name.green(),
          delta_ms.round().to_string().red(),
          "ms".red(),
          width = 25
        );

        slow_counter += 1;
      }
    }
  }

  if slow_counter == 0 {
    Ok(())
  } else {
    Err(slow_counter)
  }
}

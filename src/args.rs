use clap::{Args, Parser};

#[derive(Parser)]
#[command(name = "drill", version = "0.9.0", about = "HTTP load testing application written in Rust inspired by Ansible syntax", rename_all = "kebab-case")]
pub struct Cli {
  /// Sets the benchmark file
  #[arg(required = true)]
  pub benchmark: String,
  #[command(flatten)]
  pub metrics: Metrics,
  /// !UNIMPLEMENTED! Do not panic if an interpolation is not present.
  #[arg(long)]
  pub relaxed_interpolations: bool,
  /// Disables SSL certification check. (Not recommended)
  #[arg(long)]
  pub no_check_certificate: bool,
  #[command(flatten)]
  pub tag_options: TagOptions,
  /// List benchmark tasks (executes --tags/--skip-tags filter)
  #[arg(long)]
  pub list_tasks: bool,
  /// Disables output
  #[arg(long)]
  pub quiet: bool,
  /// Set timeout in seconds for all requests
  #[arg(long)]
  pub timeout: Option<String>,
  /// Shows statistics in nanoseconds
  #[arg(long)]
  pub nanosec: bool,
  /// Toggle verbose output
  #[arg(long)]
  pub verbose: bool,
}

impl Cli {
  pub fn to_flattened(self) -> FlattenedCli {
    FlattenedCli {
      benchmark_file: self.benchmark,
      relaxed_interpolations: self.relaxed_interpolations,
      no_check_certificate: self.no_check_certificate,
      list_tasks: self.list_tasks,
      quiet: self.quiet,
      timeout: self.timeout,
      nanosec: self.nanosec,
      verbose: self.verbose,
      threshold_option: self.metrics.compare.threshold,
      compare_path_option: self.metrics.compare.compare,
      stats_option: self.metrics.report.stats,
      report_path_option: self.metrics.report.report,
      list_tags: self.tag_options.list_tags,
      tags: self.tag_options.tag_lists.include_tags,
      skip_tags_option: self.tag_options.tag_lists.skip_tags,
    }
  }
}

#[derive(Args)]
#[group(required = false, multiple = false)]
pub struct Metrics {
  #[command(flatten)]
  pub report: ReportArgs,
  #[command(flatten)]
  pub compare: CompareFile,
}

#[derive(Args, Clone)]
#[group(required = false)]
pub struct ReportArgs {
  /// Shows request statistic
  #[arg(short, long)]
  pub stats: bool,
  /// Sets a report file
  #[arg(short, long)]
  pub report: Option<String>,
}

#[derive(Args, Clone)]
#[group(required = false)]
pub struct CompareFile {
  /// Sets a compare file
  #[arg(short, long)]
  pub compare: Option<String>,
  /// Sets a threshold value in ms amongst the compared file
  #[arg(short, long)]
  pub threshold: Option<String>,
}

#[derive(Args)]
#[group(required = false, multiple = false)]
pub struct TagOptions {
  #[command(flatten)]
  pub tag_lists: TagLists,
  /// List all benchmark tags
  #[arg(long)]
  pub list_tags: bool,
}

#[derive(Args)]
#[group(required = false)]
pub struct TagLists {
  /// Tags to include
  #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
  pub include_tags: Vec<String>,
  /// Tags to exclude
  #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
  pub skip_tags: Vec<String>,
}

pub struct FlattenedCli {
  pub benchmark_file: String,
  pub relaxed_interpolations: bool,
  pub no_check_certificate: bool,
  pub list_tasks: bool,
  pub quiet: bool,
  pub timeout: Option<String>,
  pub nanosec: bool,
  pub verbose: bool,
  pub report_path_option: Option<String>,
  pub compare_path_option: Option<String>,
  pub stats_option: bool,
  pub threshold_option: Option<String>,
  pub list_tags: bool,
  pub tags: Vec<String>,
  pub skip_tags_option: Vec<String>,
}

#[cfg(test)]
mod test {
  use super::Cli;
  use clap::CommandFactory;

  #[test]
  fn test_assertions() {
    Cli::command().debug_assert();
  }
}

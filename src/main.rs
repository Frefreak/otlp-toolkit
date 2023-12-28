#![feature(str_split_remainder)]
#[macro_use] extern crate quick_error;
use clap::Parser;
use std::error;

mod proto;
mod cmd_decode;
mod cmd_report_trace;
mod cmd_report_metric;
mod cmd_report_log;
mod cmd_search;
mod otk_error;
mod common;

#[derive(Parser, Debug)]
/// OpenTelemetry Toolkits
struct Opts {
    #[clap(subcommand)]
    command: SubCommand,
}

#[derive(Parser, Debug)]
enum SubCommand {
    #[clap(version="1.0", aliases=&["d", "de", "dec"])]
    Decode(cmd_decode::Decode),
    #[clap(version="1.0", aliases=&["t", "trace", "r", "re", "rep", "rt", "ret", "rept"])]
    ReportTrace(cmd_report_trace::Report),
    #[clap(version="1.0", aliases=&["rm", "rem", "repm", "metric"])]
    ReportMetric(cmd_report_metric::Report),
    #[clap(version="1.0", aliases=&["l", "rl", "repl", "log"])]
    ReportLog(cmd_report_log::Report),
    #[clap(version="1.0", aliases=&["s", "st"])]
    Search(cmd_search::Search)
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let opts = Opts::parse();
    match opts.command {
        SubCommand::Decode(decode) => {
            cmd_decode::do_decode(decode)?
        },
        SubCommand::ReportTrace(report) => {
            cmd_report_trace::do_report(report)?
        },
        SubCommand::ReportMetric(report) => {
            cmd_report_metric::do_report(report)?
        },
        SubCommand::ReportLog(report) => {
            cmd_report_log::do_report(report)?
        },
        SubCommand::Search(search) => {
            cmd_search::do_search(search)?
        },
    }
    Ok(())
}

use clap::Parser;
use prost::Message;
use std::error;
use std::io::{BufReader, BufRead};
use std::fs::File;
use crate::proto;
use hex::ToHex;

/// search from trace (input is base64 encoded binary)
#[derive(Parser, Debug)]
pub struct Search {
    /// file to read (- for stdin)
    input: String,

    /// search trace id (in 16 byte lowercase)
    #[clap(long)]
    trace_id: Option<String>,

    /// verbose
    #[clap(short, long)]
    verbose: bool,

    /// pretty print
    #[clap(short, long)]
    pretty: bool,
}

pub fn do_search(search: Search) -> Result<(), Box<dyn error::Error>> {
    if search.input == "-" {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            process(line.unwrap(), &search)?;
        }
    } else {
        let file = File::open(&search.input)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            process(line.unwrap(), &search)?;
        }
    }
    Ok(())
}

fn process(payload: String, search: &Search) -> Result<(), Box<dyn error::Error>> {
    let bs = base64::decode_config(payload, base64::STANDARD)?;
    let body = proto::collector::trace::v1::ExportTraceServiceRequest::decode(&bs as &[u8])?;
    if search.trace_id.is_some() {
        let id = search.trace_id.as_ref().unwrap();
        let found = body.resource_spans.iter().flat_map(|rs| {
            rs.scope_spans.iter().flat_map(|ils| {
                ils.spans.iter().map(|span| {
                    let trace_id = span.trace_id.encode_hex::<String>();
                    if search.verbose {
                        println!("{}", trace_id);
                    }
                    trace_id == *id
                })
            })
        }).any(|x| x);
        if found {
            if search.pretty {
                println!("{:#?}", body);
            } else {
                println!("{:?}", body);
            }
        }
    }
    Ok(())
}

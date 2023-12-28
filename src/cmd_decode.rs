use clap::Parser;
use rand::{distributions::Alphanumeric, Rng};
use std::error;
use prost::Message;
use crate::proto;
use std::io::{BufReader, BufRead, Read};
use strum::IntoEnumIterator;
use strum_macros::{EnumIter, EnumString, Display};
use std::fs::File;

#[derive(Debug, Clone, Display, EnumString, EnumIter)]
enum DecodeType {
    Direct,
    Span,
    Metric,
    LogRecord,
    ScopeSpans,
    ScopeMetrics,
    ScopeLogs,
    Resource,
    ResourceSpans,
    ResourceMetrics,
    ResourceLogs,
    ExportTraceServiceRequest,
    ExportMetricsServiceRequest,
    ExportLogsServiceRequest,
}

/// decode proto struct from input
#[derive(Parser, Debug)]
pub struct Decode {
    /// name of struct
    #[clap(short, long, default_value="ExportTraceServiceRequest")]
    name: DecodeType,
    /// file to read (- for stdin)
    input: String,
    /// input is base64-ed (streaming support for stdin)
    #[clap(short, long)]
    base64: bool,
    /// list available format
    #[clap(short, long)]
    list: bool,
    /// pretty print output
    #[clap(short, long)]
    pretty: bool,
}

pub fn do_decode(decode: Decode) -> Result<(), Box<dyn error::Error>> {
    // println!("{:?}", decode);
    if decode.list {
        for p in DecodeType::iter() {
            println!("{:?}", p);
        }
        return Ok(());
    }
    eprintln!("decoding as proto {}", decode.name);
    if decode.base64 {
        // stream enabled
        if decode.input == "-" {
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                decode_struct_b64(&decode.name, line.unwrap(), decode.pretty)?;
            }
        } else {
            let file = File::open(decode.input)?;
            let reader = BufReader::new(file);
            for line in reader.lines() {
                decode_struct_b64(&decode.name, line.unwrap(), decode.pretty)?;
            }
        }
    } else {
        // optimization: support incremental consuming
        if decode.input == "-" {
            let stdin = std::io::stdin();
            let mut stdin_lock = stdin.lock();
            let bytes = stdin_lock.fill_buf()?;
            decode_struct(&decode.name, bytes, decode.pretty)?;
        } else {
            let file = File::open(decode.input)?;
            let mut reader = BufReader::new(file);
            let mut buf = vec![];
            reader.read_to_end(&mut buf)?;
            decode_struct(&decode.name, &buf, decode.pretty)?;
        }
    }
    Ok(())
}

fn decode_struct_b64(name: &DecodeType, payload: String, pretty: bool) -> Result<(), Box<dyn error::Error>> {
    let bs = base64::decode_config(payload, base64::STANDARD)?;
    match decode_struct(name, &bs, pretty) {
        Ok(_) => {},
        Err(err) => {
            eprintln!("error during decoding: {}", err);
            let rs: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(7)
                .map(char::from)
                .collect();
            let filename = format!("otk.{rs}.bin");
            std::fs::write(&filename, bs)?;
            eprintln!("data dumped as {}", filename);
        },
    }
    Ok(())
}

fn decode_struct(name: &DecodeType, payload: &[u8], pretty: bool) -> Result<(), Box<dyn error::Error>> {
    // println!("{:?}", payload);
    match *name {
        DecodeType::Direct => {
            print_stuffs(payload, pretty);
        },
        DecodeType::Span => {
            print_stuffs(proto::trace::v1::Span::decode(payload)?, pretty);
        },
        DecodeType::Metric => {
            print_stuffs(proto::metrics::v1::Metric::decode(payload)?, pretty);
        },
        DecodeType::LogRecord => {
            print_stuffs(proto::logs::v1::LogRecord::decode(payload)?, pretty);
        },
        DecodeType::ScopeSpans => {
            print_stuffs(proto::trace::v1::ScopeSpans::decode(payload)?, pretty);
        },
        DecodeType::ScopeMetrics => {
            print_stuffs(proto::metrics::v1::ScopeMetrics::decode(payload)?, pretty);
        },
        DecodeType::ScopeLogs => {
            print_stuffs(proto::logs::v1::ScopeLogs::decode(payload)?, pretty);
        },
        DecodeType::Resource => {
            print_stuffs(proto::resource::v1::Resource::decode(payload)?, pretty);
        },
        DecodeType::ResourceSpans => {
            print_stuffs(proto::trace::v1::ResourceSpans::decode(payload)?, pretty);
        },
        DecodeType::ResourceMetrics => {
            print_stuffs(proto::metrics::v1::ResourceMetrics::decode(payload)?, pretty);
        },
        DecodeType::ResourceLogs => {
            print_stuffs(proto::logs::v1::ResourceLogs::decode(payload)?, pretty);
        },
        DecodeType::ExportTraceServiceRequest => {
            print_stuffs(proto::collector::trace::v1::ExportTraceServiceRequest::decode(payload)?, pretty);
        },
        DecodeType::ExportMetricsServiceRequest => {
            print_stuffs(proto::collector::metrics::v1::ExportMetricsServiceRequest::decode(payload)?, pretty);
        },
        DecodeType::ExportLogsServiceRequest => {
            print_stuffs(proto::collector::logs::v1::ExportLogsServiceRequest::decode(payload)?, pretty);
        },
    };
    Ok(())
}

fn print_stuffs<T: std::fmt::Debug>(obj: T, pretty: bool) {
    if pretty {
        println!("{:#?}", obj);
    } else {
        println!("{:?}", obj);
    }
}

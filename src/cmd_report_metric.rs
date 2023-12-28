use crate::common::{KeyValue, INSTRUMENTATION_LIB_NAME};
use crate::otk_error::OTKError;
use clap::Parser;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Histogram, UpDownCounter};
use opentelemetry::KeyValue as OTLPKeyValue;
use opentelemetry_otlp::{ExportConfig, WithExportConfig};
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::Resource;
use std::error;
use std::str::FromStr;
use std::time::Duration;
use strum_macros::{Display, EnumString};
use tokio::runtime::Runtime;

#[derive(Debug, Clone, Display, EnumString)]
enum Protocol {
    #[strum(serialize = "grpc", serialize = "g")]
    Grpc,
    #[strum(serialize = "http", serialize = "h")]
    Http,
    #[strum(serialize = "http_json", serialize = "hj")]
    HttpJson,
}

static DEFAULT_GRPC_PORT: u16 = 4317;
static DEFAULT_HTTP_PORT: u16 = 55681;
static DEFAULT_HTTP_JSON_PORT: u16 = 55681;

/// report to otlp receiver
#[derive(Parser, Debug)]
pub struct Report {
    /// protocol to use (grpc, http or http_json), currently
    /// only grpc is supported
    #[clap(long, default_value = "grpc")]
    protocol: Protocol,

    /// server host
    #[clap(long, default_value = "localhost", env = "OTK_REPORT_HOST")]
    host: String,

    /// server port (default value depends on protocol)
    #[clap(long, env = "OTK_REPORT_PORT")]
    port: Option<u16>,

    /// tag used in resource
    #[clap(short, long, num_args = 0..)]
    rtags: Vec<KeyValue>,

    /// instrumentation library name
    #[clap(long, default_value = INSTRUMENTATION_LIB_NAME)]
    library_name: String,

    /// metrics data type
    #[clap(short, long, default_value = "f64")]
    dtype: String,

    /// metrics type
    #[clap(short, long, default_value = "counter")]
    mtype: String,

    /// metrics name
    #[clap(short, long, default_value = "otk_test_metric")]
    name: String,

    /// metrics value. since this allow negative values, this needs to come at the end
    #[clap(short, long, default_value = "1", allow_hyphen_values = true, num_args = 0..)]
    value: Vec<String>,

    // TODO: removed temporarily (seems to be removed in higher version)
    // specify the selector, currently support [exact, inexpensive, histogram]
    // #[clap(short, long, default_value = "exact")]
    // selector: String,
    /// how many times to record
    #[clap(short, long, default_value = "1")]
    times: u32,

    /// how many seconds to wait
    #[clap(short, long, default_value = "0.15")]
    wait_secs: f64,

    /// histograms buckets
    #[clap(long, default_values = &["10", "20", "30", "40", "50", "60", "70", "80", "90"], num_args = 0..)]
    histograms: Vec<f64>,

    /// labels
    #[clap(short, long, num_args = 0..)]
    labels: Vec<KeyValue>,

    /// verbose
    #[clap(long)]
    verbose: bool,
}

pub fn do_report(report: Report) -> Result<(), Box<dyn error::Error>> {
    if report.verbose {
        println!("{:?}", report);
    }
    Runtime::new().unwrap().block_on(do_report_metric(report))
}

async fn do_report_metric(report: Report) -> Result<(), Box<dyn error::Error>> {
    let pipeline = opentelemetry_otlp::new_pipeline().metrics(Tokio);
    let port = report.port.unwrap_or_else(|| match report.protocol {
        Protocol::Grpc => DEFAULT_GRPC_PORT,
        Protocol::Http => DEFAULT_HTTP_PORT,
        Protocol::HttpJson => DEFAULT_HTTP_JSON_PORT,
    });
    let protocol = match report.protocol {
        Protocol::Grpc => opentelemetry_otlp::Protocol::Grpc,
        Protocol::Http => {
            return Err(Box::new(OTKError::UnimplementedError(
                "http not supported for now".into(),
            )))
        }
        Protocol::HttpJson => {
            return Err(Box::new(OTKError::UnimplementedError(
                "http json not supported for now".into(),
            )))
        }
    };
    let scheme = "http";
    let endpoint_base = format!("{}://{}:{}", scheme, report.host, port);
    let export_config = ExportConfig {
        endpoint: endpoint_base,
        protocol,
        timeout: Duration::from_secs(10),
    };
    let resource = Resource::new(report.rtags.into_iter().map(|x| x.into()));
    let labels = report
        .labels
        .into_iter()
        .map(|x| x.into())
        .collect::<Vec<_>>();
    if report.verbose {
        println!("resource: {:?}", resource);
        println!("labels: {:?}", labels);
    }
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_export_config(export_config);
    let _started = pipeline
        .with_exporter(exporter)
        .with_period(Duration::from_millis(100))
        .with_resource(resource)
        .build()?;
    let meter = global::meter(report.library_name);
    if report.verbose {
        println!("{} {}", report.dtype.as_str(), report.mtype.as_str());
    }
    let values = report
        .value
        .iter()
        .map(|x| x.as_str())
        .collect::<Vec<_>>()
        .repeat(report.times as usize);
    match (report.dtype.as_str(), report.mtype.as_str()) {
        ("u64", "counter") => {
            mk_counter_measurement(meter.u64_counter(report.name).init(), values, labels)?
        }
        ("f64", "counter") => {
            mk_counter_measurement(meter.f64_counter(report.name).init(), values, labels)?
        }
        ("i64", "up_down_counter") => {
            mk_updown_counter_measurement(meter.i64_up_down_counter(report.name).init(), values, labels)?
        }
        ("f64", "up_down_counter") => {
            mk_updown_counter_measurement(meter.f64_up_down_counter(report.name).init(), values, labels)?
        }
        ("i64", "histogram") => {
            mk_histogram_measurement(meter.i64_histogram(report.name).init(), values, labels)?
        }
        ("u64", "histogram") => {
            mk_histogram_measurement(meter.u64_histogram(report.name).init(), values, labels)?
        }
        ("f64", "histogram") => {
            mk_histogram_measurement(meter.f64_histogram(report.name).init(), values, labels)?
        }
        _ => {
            return Err(Box::new(OTKError::InvalidArgumentError(
                "invalid combination".into(),
            )))
        }
    };
    std::thread::sleep(Duration::from_millis((report.wait_secs * 1000.) as u64));

    Ok(())
}

fn mk_counter_measurement<T: FromStr>(
    counter: Counter<T>,
    values: Vec<&str>,
    labels: Vec<OTLPKeyValue>,
) -> Result<(), Box<OTKError>> {
    for val in values {
        match val.parse() {
            Ok(val) => counter.add(val, &labels),
            Err(_) => {
                return Err(Box::new(OTKError::InvalidArgumentError(
                    "parse metric value failed".into(),
                )))
            }
        }
    }
    Ok(())
}

fn mk_updown_counter_measurement<T: FromStr>(
    updown: UpDownCounter<T>,
    values: Vec<&str>,
    labels: Vec<OTLPKeyValue>,
) -> Result<(), Box<OTKError>> {
    for val in values {
        match val.parse() {
            Ok(val) => updown.add(val, &labels),
            Err(_) => {
                return Err(Box::new(OTKError::InvalidArgumentError(
                    "parse metric value failed".into(),
                )))
            }
        }
    }
    Ok(())
}

fn mk_histogram_measurement<T: FromStr>(
    recorder: Histogram<T>,
    values: Vec<&str>,
    labels: Vec<OTLPKeyValue>,
) -> Result<(), Box<OTKError>> {
    for val in values {
        match val.parse() {
            Ok(val) => recorder.record(val, &labels),
            _ => {
                return Err(Box::new(OTKError::InvalidArgumentError(
                    "parse metric value failed".into(),
                )))
            }
        }
    }
    Ok(())
}

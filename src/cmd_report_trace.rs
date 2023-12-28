use crate::common::KeyValue;
use crate::otk_error::OTKError;
use clap::Parser;
use opentelemetry::trace::{Span as _, Status, Tracer};
use opentelemetry::KeyValue as OTLP_KeyValue;
use opentelemetry::{global, Key};
use opentelemetry_otlp::{NoExporterConfig, OtlpTracePipeline, WithExportConfig};
use opentelemetry_sdk::trace::RandomIdGenerator;
use opentelemetry_sdk::{trace, Resource};
use std::error;
use std::fs::read_to_string;
use std::str::FromStr;
use strum_macros::{Display, EnumString};
use tokio::runtime::Runtime;
use tonic::metadata::{AsciiMetadataKey, MetadataMap};
use tonic::transport::{Certificate, ClientTlsConfig};

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
static DEFAULT_HTTP_PORT: u16 = 4318;
static DEFAULT_HTTP_JSON_PORT: u16 = 4318;

/// report to otlp receiver
#[derive(Parser, Debug)]
pub struct Report {
    /// protocol to use (grpc, http or http_json), currently
    /// only grpc is supported
    #[clap(long, default_value = "grpc")]
    protocol: Protocol,

    /// whether to use tls
    #[clap(long)]
    tls: bool,

    /// CA cert path if tls is enabled
    #[clap(long, requires = "tls")]
    ca_cert: Option<String>,

    /// server host name to verify
    #[clap(long, requires = "tls")]
    domain: Option<String>,

    /// server host
    #[clap(long, default_value = "localhost", env = "OTK_REPORT_HOST")]
    host: String,

    /// server port (default value depends on protocol)
    #[clap(long, env = "OTK_REPORT_PORT")]
    port: Option<u16>,

    /// tag used in resource
    #[clap(short, long, num_args = 0..)]
    rtags: Vec<KeyValue>,

    /// metadata map value
    #[clap(short, long, num_args = 0..)]
    metadata: Vec<KeyValue>,

    /// span name
    #[clap(short, long, default_value = "otk_test_span")]
    name: String,

    /// span attributes
    #[clap(short, long, num_args = 0..)]
    attrs: Vec<KeyValue>,

    /// long length tag (for testing size limit), tag name is "ll",
    /// and for k=v will repeat string k, v times
    #[clap(long)]
    long_length_tag: Option<KeyValue>,

    /// status message
    #[clap(long)]
    status_msg: Option<String>,

    /// duration in milliseconds
    #[clap(long, default_value = "0")]
    duration: u64,

    /// send a batch of spans
    #[clap(long, default_value = "1")]
    batch: u64,

    /// verbose
    #[clap(short, long)]
    verbose: bool,

    /// send timeout in seconds (this is a general timeout and might be restricted by other
    /// timeout, like batch processor timeout)
    #[clap(short, long, default_value = "10")]
    timeout: u64,
}

pub fn do_report(report: Report) -> Result<(), Box<dyn error::Error>> {
    if report.verbose {
        println!("{:?}", report);
    }
    Runtime::new().unwrap().block_on(do_report_trace(report))
}

async fn do_report_trace(report: Report) -> Result<(), Box<dyn error::Error>> {
    let pipeline = opentelemetry_otlp::new_pipeline().tracing();
    let port = report.port.unwrap_or_else(|| match report.protocol {
        Protocol::Grpc => DEFAULT_GRPC_PORT,
        Protocol::Http => DEFAULT_HTTP_PORT,
        Protocol::HttpJson => DEFAULT_HTTP_JSON_PORT,
    });
    let scheme = if report.tls { "https" } else { "http" };
    let endpoint_base = format!("{}://{}:{}", scheme, report.host, port);
    let resource = Resource::new(report.rtags.iter().map(|x| x.clone().into()));
    let trace_config = trace::config()
        .with_sampler(trace::Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(resource);
    let pipeline = pipeline.with_trace_config(trace_config);

    match report.protocol {
        Protocol::Grpc => do_report_trace_grpc(pipeline, report, endpoint_base).await,
        Protocol::Http => do_report_trace_http(pipeline, report, endpoint_base).await,
        _ => return Err(Box::new(OTKError::UnimplementedError("httpjson".into()))),
    }
}

async fn do_report_trace_grpc(
    pipeline: OtlpTracePipeline<NoExporterConfig>,
    report: Report,
    endpoint_base: String,
) -> Result<(), Box<dyn error::Error>> {
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(endpoint_base)
        .with_timeout(std::time::Duration::from_secs(report.timeout));
    let exporter = if report.tls {
        let mut tls_config = ClientTlsConfig::new();
        if report.ca_cert.is_some() {
            let pem = read_to_string(report.ca_cert.unwrap()).expect("open cacert");
            tls_config = tls_config.ca_certificate(Certificate::from_pem(pem));
        };
        if report.domain.is_some() {
            tls_config = tls_config.domain_name(report.domain.unwrap());
        }
        exporter.with_tls_config(tls_config)
    } else {
        exporter
    };
    let mut meta_map = MetadataMap::new();
    for kv in &report.metadata {
        meta_map.append(
            AsciiMetadataKey::from_str(kv.k.as_str())?,
            kv.v.as_str().parse()?,
        );
    }
    let exporter = exporter.with_metadata(meta_map);
    let pipeline = pipeline.with_exporter(exporter);

    let tracer = pipeline.install_batch(opentelemetry_sdk::runtime::Tokio)?;

    let span_builder = tracer.span_builder(report.name);
    for _ in 0..report.batch {
        let mut span = span_builder.clone().start(&tracer);
        for attr in &report.attrs {
            span.set_attribute(attr.clone().into())
        }
        if let Some(ll) = &report.long_length_tag {
            let val = ll.k.repeat(ll.v.parse::<u32>()? as usize);
            span.set_attribute(Key::new("ll").string(val));
        }
        std::thread::sleep(std::time::Duration::from_millis(report.duration));
        if report.status_msg.is_none() {
            span.set_status(Status::Ok);
        } else {
            span.set_status(Status::error(report.status_msg.clone().unwrap()));
        }
        span.end();
        if report.verbose {
            println!("{:x}", span.span_context().trace_id())
        }
    }
    global::shutdown_tracer_provider();
    Ok(())
}

async fn do_report_trace_http(
    pipeline: OtlpTracePipeline<NoExporterConfig>,
    report: Report,
    endpoint_base: String,
) -> Result<(), Box<dyn error::Error>> {
    if report.tls {
        return Err(Box::new(OTKError::UnimplementedError(
            "http does not support tls for now".into(),
        )));
    }
    if !report.metadata.is_empty() {
        return Err(Box::new(OTKError::InvalidArgumentError(
            "http can not set metadata for now".into(),
        )));
    }

    let exporter = opentelemetry_otlp::new_exporter()
        .http()
        .with_endpoint(endpoint_base)
        .with_timeout(std::time::Duration::from_secs(report.timeout));

    let tracer = pipeline
        .with_exporter(exporter)
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    let span_builder = tracer.span_builder(report.name);
    for _ in 0..report.batch {
        let mut span = span_builder.clone().start(&tracer);
        for attr in &report.attrs {
            span.set_attribute(OTLP_KeyValue::new(attr.k.clone(), attr.v.clone()))
        }
        if let Some(ll) = &report.long_length_tag {
            let val = ll.k.repeat(ll.v.parse::<u32>()? as usize);
            span.set_attribute(Key::new("ll").string(val));
        }
        std::thread::sleep(std::time::Duration::from_millis(report.duration));
        if report.status_msg.is_none() {
            span.set_status(Status::Ok);
        } else {
            span.set_status(Status::error(report.status_msg.clone().unwrap()));
        }
        span.end();
        if report.verbose {
            println!("{:x}", span.span_context().trace_id())
        }
    }
    global::shutdown_tracer_provider();
    Ok(())
}

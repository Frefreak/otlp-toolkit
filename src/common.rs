use opentelemetry::KeyValue as OTLP_KeyValue;
use std::str::FromStr;
use crate::otk_error::OTKError;

pub const INSTRUMENTATION_LIB_NAME: &str = "otk.kto";

#[derive(Debug, Clone)]
pub struct KeyValue {
    pub k: String,
    pub v: String,
}

impl FromStr for KeyValue {
    type Err = OTKError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut splits = s.split('=');
        let fst = splits
            .next()
            .ok_or_else(|| OTKError::ParseError(String::from("")))?;
        let snd = splits.remainder();
        if snd.is_none() {
            return Err(OTKError::ParseError(String::from(
                "invalid format (expect key=value)",
            )));
        }
        Ok(KeyValue {
            k: String::from(fst),
            v: String::from(snd.unwrap()),
        })
    }
}

impl From<KeyValue> for OTLP_KeyValue {
    fn from(kv: KeyValue) -> Self {
        OTLP_KeyValue::new(kv.k, kv.v)
    }
}

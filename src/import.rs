use anyhow::{anyhow, Context, Result};
use base64::engine::{general_purpose, Engine as _};
use inflate::inflate_bytes_zlib;
use serde_json::Value;

pub fn decompress_string(blueprint_string: &str) -> Result<Value> {
    let skip_first_byte = &blueprint_string.as_bytes()[1..blueprint_string.len()];
    let base64_decoded = general_purpose::STANDARD.decode(skip_first_byte)?;
    let decoded = inflate_bytes_zlib(&base64_decoded).map_err(|s| anyhow!(s))?;
    Ok(serde_json::from_slice(&decoded)?)
}

pub fn get_json_entities(json: Value) -> Result<Vec<Value>> {
    json.get("blueprint")
        .context("No blueprint key in json")?
        .get("entities")
        .context("No entities key in blueprint")?
        .as_array()
        .context("Entities are not an array")
        .map(|v| v.to_owned())
}

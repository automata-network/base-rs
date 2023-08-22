use std::prelude::v1::*;

use std::fs;

pub fn parse_file<T>(path: &str) -> Result<T, String>
where
    for<'a> T: serde::de::Deserialize<'a>,
{
    let data =
        fs::read_to_string(path).map_err(|err| format!("open file {} fail: {:?}", path, err))?;
    serde_json::from_str(&data).map_err(|err| format!("parse file {} fail: {:?}", path, err))
}

pub fn read_file(path: &str) -> Result<Vec<u8>, String> {
    let data =
        fs::read_to_string(path).map_err(|err| format!("open file {} fail: {:?}", path, err))?;
    Ok(data.into())
}

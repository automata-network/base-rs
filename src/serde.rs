use std::prelude::v1::*;

use eth_types::SU256;
use serde::{Deserialize, Deserializer};

use super::format::read_ether;

pub fn deserialize_ether<'de, D>(deserializer: D) -> Result<SU256, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(read_ether(s, 18).into())
}

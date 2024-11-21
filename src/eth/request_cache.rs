use std::{future::Future, io, path::PathBuf};

use alloy::primitives::keccak256;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::value::RawValue;

#[derive(Clone, Debug)]
pub struct RequestCache {
    base_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
pub struct JsonCache {
    pub key: Box<serde_json::value::RawValue>,
    pub value: Box<serde_json::value::RawValue>,
}

impl RequestCache {
    pub fn new(base_path: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&base_path);
        Self { base_path }
    }

    fn get_key(&self, key: &[u8]) -> PathBuf {
        let key = keccak256(&key);
        let fp = self
            .base_path
            .join(self.base_path.join(format!("{}.cache", key)));
        fp
    }

    pub fn add_cache(&self, key: &[u8], data: &[u8]) -> io::Result<()> {
        std::fs::write(self.get_key(key), data)?;
        Ok(())
    }

    pub fn get_cache(&self, key: &[u8]) -> Option<Vec<u8>> {
        match std::fs::read(self.get_key(key)) {
            Ok(data) => Some(data),
            Err(_) => None,
        }
    }

    pub fn json_key<K>(&self, key: K) -> Box<RawValue>
    where
        K: Serialize + std::fmt::Debug,
    {
        RawValue::from_string(serde_json::to_string(&key).unwrap()).unwrap()
    }

    pub fn batch_json<V, I, K>(&self, params: I) -> Result<Vec<Option<V>>, serde_json::Error>
    where
        V: DeserializeOwned,
        K: Serialize + std::fmt::Debug,
        I: Iterator<Item = K>,
    {
        let mut out = Vec::new();
        for param in params {
            let key = self.json_key(param);
            out.push(match self.get_cache(key.get().as_bytes()) {
                Some(n) => {
                    let val: JsonCache = serde_json::from_slice(&n)?;
                    serde_json::from_str(val.value.get())?
                }
                None => None,
            });
        }
        Ok(out)
    }

    pub fn save_json<V>(&self, key: &RawValue, data: &V) -> io::Result<()>
    where
        V: Serialize + DeserializeOwned,
    {
        let data = RawValue::from_string(serde_json::to_string_pretty(&data).unwrap()).unwrap();
        let cache = JsonCache {
            key: key.to_owned(),
            value: data,
        };
        let val = serde_json::to_vec_pretty(&cache).unwrap();

        self.add_cache(key.get().as_bytes(), &val)?;
        Ok(())
    }

    pub async fn json<F, V, E>(&self, key: &RawValue, f: F) -> Result<V, E>
    where
        V: Serialize + DeserializeOwned,
        F: Future<Output = Result<V, E>>,
    {
        if let Some(value) = self.get_cache(key.get().as_bytes()) {
            log::info!(target: "cache", "get from cache: {:?} -> {:?}", key, self.get_key(key.get().as_bytes()));

            let val: JsonCache = serde_json::from_slice(&value).unwrap();
            return Ok(serde_json::from_str(val.value.get()).unwrap());
        }

        log::info!(target: "cache", "retrive from remote: {:?} -> {:?}", key, self.get_key(key.get().as_bytes()));
        let value = f.await?;
        self.save_json(key, &value).unwrap();
        Ok(value)
    }
}

use std::{borrow::Cow, path::PathBuf, sync::Arc, time::Duration};

use alloy::{
    eips::BlockId,
    primitives::{Address, Bytes, B256, U256},
    providers::{
        network::{Ethereum, EthereumWallet, TransactionBuilder},
        PendingTransactionBuilder, Provider, ProviderBuilder,
    },
    rpc::{
        client::{BatchRequest, RpcClientInner},
        json_rpc::{RpcParam, RpcReturn},
        types::{BlockTransactionsKind, Transaction, TransactionRequest},
    },
    signers::local::{LocalSignerError, PrivateKeySigner},
    sol_types::{SolCall, SolInterface},
    transports::{
        http::{Client, Http},
        RpcError, TransportErrorKind,
    },
};
use serde::{de::DeserializeOwned, Serialize};

use crate::thread::{wait_timeout, TimeoutError};

use super::RequestCache;

crate::stack_error! {
    #[derive(Debug)]
    name: EthError,
    stack_name: EthErrorStack,
    error: {},
    wrap: {
        Signer(LocalSignerError),
        Url(url::ParseError),
        Json(serde_json::Error),
        Rpc(RpcError<TransportErrorKind>),
        Type(alloy::sol_types::Error),
        Timeout(TimeoutError),
    },
    stack: {
        OnTransact(contract: Address, sig: &'static str),
        OnCall(contract: Address, sig: &'static str),
        OnDecodeReturn(contract: Address, sig: &'static str, data: Bytes),
        Request(method: Cow<'static, str>),
        BatchRequestSerFail(),
        BatchRequestDerRespFail(),
        BatchRequestWait(),
        WaitResponse(),
        BatchSend(),
    }
}

impl EthError {
    pub fn revert(&self) -> Option<Bytes> {
        match self.origin() {
            Self::Rpc(RpcError::ErrorResp(payload)) => payload.as_revert_data(),
            _ => None,
        }
    }

    pub fn revert_data<T: SolInterface>(self) -> Result<(T, EthError), EthError> {
        match self.origin() {
            Self::Rpc(RpcError::ErrorResp(payload)) => match payload.as_revert_data() {
                Some(data) => Ok((T::abi_decode(&data, true)?, self)),
                None => Err(self),
            },
            _ => Err(self),
        }
    }
}

#[derive(Clone)]
pub struct Eth {
    cache: Option<RequestCache>,
    client: Arc<Box<dyn Provider<Http<Client>>>>,
    call_timeout: Option<Duration>,
}

impl Eth {
    pub fn dial(endpoint: &str, private_key: Option<&str>) -> Result<Eth, EthError> {
        let url = endpoint.try_into()?;

        let provider: Box<dyn Provider<Http<Client>>> = match private_key {
            Some(pk) => {
                let signer = pk.parse::<PrivateKeySigner>()?;
                let wallet = EthereumWallet::new(signer);
                let provider = ProviderBuilder::new()
                    .with_recommended_fillers()
                    .wallet(wallet)
                    .on_http(url);
                Box::new(provider)
            }
            None => {
                let provider = ProviderBuilder::new().on_http(url);
                Box::new(provider)
            }
        };

        Ok(Eth {
            client: Arc::new(provider),
            call_timeout: None,
            cache: None,
        })
    }

    pub fn with_cache(&mut self, base_path: PathBuf) -> &mut Self {
        self.cache = Some(RequestCache::new(base_path));
        self
    }

    pub fn with_call_timeout(&mut self, call_timeout: Option<Duration>) -> &mut Self {
        self.call_timeout = call_timeout;
        self
    }

    pub async fn transact<T: SolCall>(
        &self,
        contract: Address,
        call: &T,
    ) -> Result<PendingTransactionBuilder<Http<Client>, Ethereum>, EthError> {
        let tx = TransactionRequest::default().with_call(call).to(contract);
        let result = self
            .client
            .send_transaction(tx)
            .await
            .map_err(EthError::OnTransact(&contract, &T::SIGNATURE))?;
        Ok(result)
    }

    pub async fn call<T: SolCall>(
        &self,
        contract: Address,
        call: &T,
    ) -> Result<T::Return, EthError> {
        let tx = TransactionRequest::default().with_call(call).to(contract);
        let result = crate::thread::wait_timeout(self.call_timeout, self.client.call(&tx))
            .await
            .map_err(EthError::OnCall(&contract, &T::SIGNATURE))?
            .map_err(EthError::OnCall(&contract, &T::SIGNATURE))?;
        let result = T::abi_decode_returns(&result, true).map_err(EthError::OnDecodeReturn(
            &contract,
            &T::SIGNATURE,
            &result,
        ))?;
        Ok(result)
    }

    pub async fn select_reference_block(&self) -> Result<(U256, B256), EthError> {
        // corner case:
        //  1. block numbers may not sequential
        //  2. the types.Header.Hash() may not compatible with the chain
        let k = BlockTransactionsKind::Hashes;
        let p = self.provider();
        let head = p.get_block(BlockId::latest(), k).await?.unwrap();
        let hash = head.header.parent_hash;
        let reference_block = p.get_block(hash.into(), k).await?.unwrap();
        let number = reference_block.header.number.unwrap();
        Ok((U256::from_limbs_slice(&[number]), hash))
    }

    pub fn provider(&self) -> Arc<Box<dyn Provider<Http<Client>>>> {
        self.client.clone()
    }

    pub fn client(&self) -> &RpcClientInner<Http<Client>> {
        self.client.client()
    }

    pub async fn get_transaction(&self, hash: B256) -> Option<Transaction> {
        let tx = self.client.get_transaction_by_hash(hash).await.unwrap();
        tx
    }

    pub async fn request<Params, Resp>(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: Params,
    ) -> Result<Resp, EthError>
    where
        Params: Serialize + Clone + std::fmt::Debug + Send + Sync + Unpin,
        Resp: Serialize + DeserializeOwned + std::fmt::Debug + Send + Sync + Unpin + 'static,
    {
        wait_timeout(self.call_timeout, self.inner_request(method, params))
            .await
            .map_err(EthError::WaitResponse())?
    }

    async fn inner_request<Params, Resp>(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: Params,
    ) -> Result<Resp, EthError>
    where
        Params: Serialize + Clone + std::fmt::Debug + Send + Sync + Unpin,
        Resp: Serialize + DeserializeOwned + std::fmt::Debug + Send + Sync + Unpin + 'static,
    {
        let method = method.into();
        match &self.cache {
            Some(cache) => {
                let key = cache.json_key((&method, &params));
                cache
                    .json(&key, self.client().request(method.clone(), params))
                    .await
                    .map_err(EthError::Request(&method))
            }
            None => self
                .client()
                .request(method.clone(), params)
                .await
                .map_err(EthError::Request(&method)),
        }
    }

    pub async fn batch_request_chunks<
        Params: RpcParam + std::fmt::Debug,
        Resp: RpcReturn + Serialize,
    >(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: &[Params],
        chunk_size: usize,
    ) -> Result<Vec<Resp>, EthError> {
        wait_timeout(
            self.call_timeout,
            self.inner_batch_request_chunks(method, params, chunk_size),
        )
        .await
        .map_err(EthError::WaitResponse())?
    }

    async fn inner_batch_request_chunks<
        Params: RpcParam + std::fmt::Debug,
        Resp: RpcReturn + Serialize,
    >(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: &[Params],
        chunk_size: usize,
    ) -> Result<Vec<Resp>, EthError> {
        let params_chunks = params.chunks(chunk_size);
        let mut out = Vec::with_capacity(params.len());
        let method = method.into();
        for p in params_chunks {
            let resp: Vec<Resp> = self.batch_request(method.clone(), p).await?;
            out.extend(resp);
        }
        Ok(out)
    }

    pub async fn batch_request<Params: RpcParam + std::fmt::Debug, Resp: RpcReturn + Serialize>(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: &[Params],
    ) -> Result<Vec<Resp>, EthError> {
        wait_timeout(self.call_timeout, self.inner_batch_request(method, params))
            .await
            .map_err(EthError::WaitResponse())?
    }

    async fn inner_batch_request<
        Params: RpcParam + std::fmt::Debug,
        Resp: RpcReturn + Serialize,
    >(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: &[Params],
    ) -> Result<Vec<Resp>, EthError> {
        let method: Cow<'static, str> = method.into();
        let mut batch = BatchRequest::new(self.client());
        let mut waiters = Vec::new();
        let mut cached_result: Vec<Option<Resp>> = match &self.cache {
            Some(cache) => cache
                .batch_json(params.iter().map(|p| (method.clone(), p)))
                .map_err(EthError::BatchRequestDerRespFail())?,
            None => params.iter().map(|_| None).collect(),
        };
        for (idx, param) in params.into_iter().enumerate() {
            if cached_result[idx].is_some() {
                continue;
            }
            waiters.push((
                param,
                idx,
                batch
                    .add_call::<_, Resp>(method.clone(), param)
                    .map_err(EthError::BatchRequestSerFail())?,
            ));
        }

        if waiters.len() > 0 {
            batch.send().await.map_err(EthError::BatchSend())?;
            wait_timeout(self.call_timeout, async {
                for (p, idx, waiter) in waiters {
                    let result = waiter.await.map_err(EthError::BatchRequestDerRespFail())?;
                    if let Some(cache) = &self.cache {
                        let key = cache.json_key((method.clone(), p));
                        cache.save_json(&key, &result).unwrap();
                    }
                    cached_result[idx] = Some(result);
                }
                Ok::<(), EthError>(())
            })
            .await
            .map_err(EthError::BatchRequestWait())?
            .map_err(EthError::BatchRequestWait())?;
        }

        Ok(cached_result.into_iter().map(|n| n.unwrap()).collect())
    }
}

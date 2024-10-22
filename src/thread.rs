use std::{
    future::{Future, IntoFuture},
    sync::Arc,
    time::Duration,
};

use tokio::{runtime::Builder, sync::Semaphore};

use crate::trace::Alive;

pub async fn parallel<O, T, C, A, F, E>(
    _alive: &Alive,
    ctx: C,
    tasks: Vec<T>,
    worker: usize,
    f: F,
) -> Result<Vec<O>, E>
where
    E: Send + 'static,
    O: Send + 'static,
    C: Clone + Send + 'static,
    T: Send + 'static,
    A: Future<Output = Result<O, E>> + Send + 'static,
    F: Fn(T, C) -> A + Clone + Send + 'static,
{
    let rt = Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(worker)
        .build()
        .unwrap();
    let semaphore = Arc::new(Semaphore::new(worker));
    let mut results = Vec::new();
    let task_len = tasks.len();
    for task in tasks {
        let handler = f.clone();
        let ctx = ctx.clone();
        let semaphore = semaphore.clone();
        let handle = rt.spawn(async move {
            let _guard = semaphore.acquire().await.unwrap();
            handler(task, ctx).await
        });
        results.push(handle);
    }
    let mut out = Vec::with_capacity(task_len);
    for result in results {
        match result.await.unwrap() {
            Ok(n) => out.push(n),
            Err(err) => {
                rt.shutdown_background();
                return Err(err);
            }
        }
    }
    rt.shutdown_background();
    return Ok(out);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutError;

pub async fn wait_timeout<F>(
    duration: Option<Duration>,
    future: F,
) -> Result<F::Output, TimeoutError>
where
    F: IntoFuture,
{
    match duration {
        Some(du) => match tokio::time::timeout(du, future).await {
            Ok(n) => Ok(n),
            Err(_) => Err(TimeoutError),
        },
        None => Ok(future.await),
    }
}

pub async fn wait_result<F, T, E>(
    duration: Option<Duration>,
    future: F,
) -> Result<T, E>
where
    F: IntoFuture<Output = Result<T, E>>,
    E: From<TimeoutError>,
{
    match wait_timeout(duration, future).await {
        Ok(result) => result,
        Err(err) => Err(err.into())
    }
}
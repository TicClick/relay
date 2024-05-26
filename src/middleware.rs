use std::sync::{Arc, Mutex};

use viz::{async_trait, Handler, IntoResponse, Request, Response, Result, StatusCode, Transform};

#[derive(Debug, Clone)]
pub struct Config {
    max_concurrent_requests: i32,
}

impl Config {
    pub fn new(max_concurrent_requests: i32) -> Self {
        Self {
            max_concurrent_requests,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 10,
        }
    }
}

impl<H> Transform<H> for Config
where
    H: Clone,
{
    type Output = RateLimiter<H>;

    fn transform(&self, h: H) -> Self::Output {
        RateLimiter {
            h,
            config: self.clone(),
            concurrent_requests: Arc::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimiter<H> {
    h: H,
    config: Config,
    concurrent_requests: Arc<Mutex<i32>>,
}

#[async_trait]
impl<H, O> Handler<Request> for RateLimiter<H>
where
    H: Handler<Request, Output = Result<O>>,
    O: IntoResponse,
{
    type Output = Result<Response>;

    async fn call(&self, req: Request) -> Self::Output {
        {
            let mut current_value = self.concurrent_requests.lock().unwrap();
            if *current_value > self.config.max_concurrent_requests {
                return Ok(StatusCode::SERVICE_UNAVAILABLE.into_response());
            }
            *current_value += 1;
        }

        let resp = self.h.call(req).await.map(IntoResponse::into_response);
        {
            *self.concurrent_requests.lock().unwrap() -= 1;
        }
        resp
    }
}

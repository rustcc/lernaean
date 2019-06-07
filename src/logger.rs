use futures::future::BoxFuture;
use tide::{
    middleware::{Middleware, Next},
    Context, Response,
};

pub struct Logger;
impl<Data: Send + Sync + 'static> Middleware<Data> for Logger {
    fn handle<'a>(&'a self, cx: Context<Data>, next: Next<'a, Data>) -> BoxFuture<'a, Response> {
        futures::FutureExt::boxed(async move {
            let path = cx.uri().path().to_owned();
            let method = cx.method().as_str().to_owned();

            let res = next.run(cx).await;
            let status = res.status();
            info!("{} {} {}", method, path, status.as_str());
            res
        })
    }
}

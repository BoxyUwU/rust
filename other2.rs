#![allow(unused)]
#![feature(async_fn_traits)]

use std::future::Future;

/// Provides zero to an async function.
fn provide_zero<'a, F>(f: &'a mut F) -> impl Future<Output = ()> + 'a
where
    F: AsyncFnMut(i32),
    for<'b> F::CallRefFuture<'b>: Send,
{
    async { f(0).await }
}

/// Builds a vector containing zero in a really silly way.
async fn foo() -> Vec<i32> {
    let mut v = Vec::new();
    provide_zero(&mut async |n| v.push(n)).await;
    v
}

fn main() {}
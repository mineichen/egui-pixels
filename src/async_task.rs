use std::task::Context;

use futures::{future::BoxFuture, FutureExt};

pub enum AsyncRefTask<T> {
    Pending(BoxFuture<'static, T>),
    Ready(T),
}

impl<T> AsyncRefTask<T> {
    pub fn new(b: BoxFuture<'static, T>) -> Self {
        Self::Pending(b)
    }

    pub fn new_ready(r: T) -> Self {
        Self::Ready(r)
    }

    pub fn data(&mut self) -> Option<&mut T> {
        match self {
            AsyncRefTask::Pending(x) => {
                let waker = futures::task::noop_waker();
                let mut cx = Context::from_waker(&waker);
                match x.poll_unpin(&mut cx) {
                    std::task::Poll::Ready(r) => {
                        *self = Self::Ready(r);
                        let Self::Ready(x) = self else { unreachable!() };
                        Some(x)
                    }
                    std::task::Poll::Pending => None,
                }
            }
            AsyncRefTask::Ready(x) => Some(x),
        }
    }
}
pub struct AsyncTask<T>(BoxFuture<'static, T>);

impl<T> AsyncTask<T> {
    pub fn new(b: BoxFuture<'static, T>) -> Self {
        Self(b)
    }

    pub fn data(&mut self) -> Option<T> {
        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        match self.0.poll_unpin(&mut cx) {
            std::task::Poll::Ready(r) => {
                #[cfg(debug_assertions)]
                {
                    self.0 = std::future::poll_fn(|_| {
                        panic!("The result of AsyncTask mustn't be used after it returned")
                    })
                    .boxed();
                }
                Some(r)
            }
            std::task::Poll::Pending => None,
        }
    }
}
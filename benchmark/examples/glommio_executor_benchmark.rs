use futures::future::{FutureExt, LocalBoxFuture};
use glommio::prelude::*;

fn dumb_fibonacci(n: u32) -> LocalBoxFuture<'static, u32> {
    async move {
        match n {
            1 => 1,
            2 => 2,
            n => {
                let a = glommio::spawn_local(dumb_fibonacci(n - 1));
                let b = glommio::spawn_local(dumb_fibonacci(n - 2));
                a.await + b.await
            }
        }
    }
    .boxed_local()
}

fn main() {
    LocalExecutorBuilder::default()
        .spawn(|| async move {
            let mut sum_of_fibbs = 0;

            for i in 1..30 {
                sum_of_fibbs += dumb_fibonacci(i).await;
            }

            println!("the sum of fibbonacci numbers is {sum_of_fibbs}");

            assert_eq!(sum_of_fibbs, 2178307);
        })
        .expect("failed to spawn local executor")
        .join()
        .unwrap();
}

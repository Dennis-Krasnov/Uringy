use futures::future::{FutureExt, LocalBoxFuture};

fn dumb_fibonacci(n: u32) -> LocalBoxFuture<'static, u32> {
    async move {
        match n {
            1 => 1,
            2 => 2,
            n => {
                let a = tokio::task::spawn_local(dumb_fibonacci(n - 1));
                let b = tokio::task::spawn_local(dumb_fibonacci(n - 2));
                a.await.unwrap() + b.await.unwrap()
            }
        }
    }
    .boxed_local()
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            let mut sum_of_fibbs = 0;

            for i in 1..30 {
                sum_of_fibbs += dumb_fibonacci(i).await;
            }

            println!("the sum of fibbonacci numbers is {sum_of_fibbs}");

            assert_eq!(sum_of_fibbs, 2178307);
        })
        .await;
}

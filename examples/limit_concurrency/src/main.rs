use std::time::{Duration, Instant};

use async_io::Timer;
use async_lock::Semaphore;

use uringy::spawn;
use std::ops::{Mul, Div};

const N: u32 = 1_000_000;
const MAX_CONCURRENT: usize = 100;
const SLEEP_TIME: u64 = 1;

// TODO: #[uringy::main]
fn main() {
    uringy::block_on(async {
        // Image we have data coming in that requires processing (eg. HTTP requests)
        let (s, r) = async_channel::unbounded();

        // Doesn't matter how this data arrives
        spawn(async move {
            for n in 0..N {
                s.send(n).await.unwrap();
            }
        });

        let start_time = Instant::now();

        // Because of resource constraints, we only want to process up to 10 at a time
        let semaphore = Semaphore::new(MAX_CONCURRENT);

        while let Ok(n) = r.recv().await {
            // Wait until there's an available permit, only then spawn the task
            let permit = semaphore.acquire().await;

            // Move ownership of the permit into the task
            spawn(async move {
                process_data(n).await;
                drop(permit);
            });
        }

        let theoretical = Duration::from_millis(SLEEP_TIME).mul(N).div(MAX_CONCURRENT as u32);
        println!("Theoretically, this should take {} seconds", theoretical.as_secs());
        println!("In practice, this took {} seconds", start_time.elapsed().as_secs());
    });
}

/// ...
/// Blissfully unaware of any concurrency going on.
async fn process_data(n: u32) {
    Timer::after(Duration::from_millis(SLEEP_TIME)).await;
    println!("processed #{}", n);
}

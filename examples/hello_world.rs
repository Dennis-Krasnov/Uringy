use uringy::runtime;

fn main() {
    // run async block to completion
    runtime::block_on(async {
        // create another task, will run concurrently with the current async block
        let handle = runtime::spawn(async {
            println!("world");
        });

        println!("hello");
        handle.await.unwrap();
    });
}

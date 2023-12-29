Uringy
======

[![website]](https://uringy-documentation.fly.dev/)
[![github]](https://github.com/Dennis-Krasnov/Uringy)
[![crates-io]](https://crates.io/crates/uringy)
[![docs-rs]](https://docs.rs/uringy)
[![license]](https://github.com/Dennis-Krasnov/Uringy/blob/master/LICENSE)

[website]: https://img.shields.io/static/v1?label=website&message=uringy-documentation.fly.dev&style=for-the-badge&labelColor=555555&color=blue&logo=github
[github]: https://img.shields.io/static/v1?label=github&message=Dennis-Krasnov/Uringy&style=for-the-badge&labelColor=555555&color=17c208&logo=github
[crates-io]: https://img.shields.io/crates/v/uringy.svg?style=for-the-badge&logo=image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA1MTIgNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJNMjM5LjEgNi4zbC0yMDggNzhjLTE4LjcgNy0zMS4xIDI1LTMxLjEgNDV2MjI1LjFjMCAxOC4yIDEwLjMgMzQuOCAyNi41IDQyLjlsMjA4IDEwNGMxMy41IDYuOCAyOS40IDYuOCA0Mi45IDBsMjA4LTEwNGMxNi4zLTguMSAyNi41LTI0LjggMjYuNS00Mi45VjEyOS4zYzAtMjAtMTIuNC0zNy45LTMxLjEtNDQuOWwtMjA4LTc4QzI2MiAyLjIgMjUwIDIuMiAyMzkuMSA2LjN6TTI1NiA2OC40bDE5MiA3MnYxLjFsLTE5MiA3OC0xOTItNzh2LTEuMWwxOTItNzJ6bTMyIDM1NlYyNzUuNWwxNjAtNjV2MTMzLjlsLTE2MCA4MHoiPjwvcGF0aD48L3N2Zz4=
[docs-rs]: https://img.shields.io/static/v1?label=docs.rs&message=uringy&style=for-the-badge&labelColor=555555&color=red&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K
[license]: https://img.shields.io/static/v1?label=license&message=BSD0&style=for-the-badge&labelColor=555555&color=b509a4&logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA2NDAgNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJNOTYgNDY0djMyYzAgOC44NCA3LjE2IDE2IDE2IDE2aDIyNGM4Ljg0IDAgMTYtNy4xNiAxNi0xNlYxNTMuMjVjNC41Ni0yIDguOTItNC4zNSAxMi45OS03LjEybDE0Mi4wNSA0Ny42M2M4LjM4IDIuODEgMTcuNDUtMS43MSAyMC4yNi0xMC4wOGwxMC4xNy0zMC4zNGMyLjgxLTguMzgtMS43MS0xNy40NS0xMC4wOC0yMC4yNmwtMTI4LjQtNDMuMDVjLjQyLTMuMzIgMS4wMS02LjYgMS4wMS0xMC4wMyAwLTQ0LjE4LTM1LjgyLTgwLTgwLTgwLTI5LjY5IDAtNTUuMyAxNi4zNi02OS4xMSA0MC4zN0wxMzIuOTYuODNjLTguMzgtMi44MS0xNy40NSAxLjcxLTIwLjI2IDEwLjA4bC0xMC4xNyAzMC4zNGMtMi44MSA4LjM4IDEuNzEgMTcuNDUgMTAuMDggMjAuMjZsMTMyIDQ0LjI2YzcuMjggMjEuMjUgMjIuOTYgMzguNTQgNDMuMzggNDcuNDdWNDQ4SDExMmMtOC44NCAwLTE2IDcuMTYtMTYgMTZ6TTAgMzA0YzAgNDQuMTggNTcuMzEgODAgMTI4IDgwczEyOC0zNS44MiAxMjgtODBoLS4wMmMwLTE1LjY3IDIuMDgtNy4yNS04NS4wNS0xODEuNTEtMTcuNjgtMzUuMzYtNjguMjItMzUuMjktODUuODcgMEMtMS4zMiAyOTUuMjcuMDIgMjg3LjgyLjAyIDMwNEgwem01Ni0xNmw3Mi0xNDQgNzIgMTQ0SDU2em0zMjguMDIgMTQ0SDM4NGMwIDQ0LjE4IDU3LjMxIDgwIDEyOCA4MHMxMjgtMzUuODIgMTI4LTgwaC0uMDJjMC0xNS42NyAyLjA4LTcuMjUtODUuMDUtMTgxLjUxLTE3LjY4LTM1LjM2LTY4LjIyLTM1LjI5LTg1Ljg3IDAtODYuMzggMTcyLjc4LTg1LjA0IDE2NS4zMy04NS4wNCAxODEuNTF6TTQ0MCA0MTZsNzItMTQ0IDcyIDE0NEg0NDB6Ij48L3BhdGg+PC9zdmc+

Writing concurrent code in Rust doesn't need to be painful.
Uringy is a runtime that combines structured concurrency, a single-threaded design, and Linux's io_uring.
Intended for server applications, from simple single-threaded to highly scalable thread-per-core designs.

## Goals
#### Simple API
- Familiar blocking syntax which closely mirrors Rust's standard library
- Avoid `async`/`await`'s limitations and footguns
- Easy to learn with stellar documentation and examples
- Spawn with non-`Send` and non-`'static` types
- Leak-free hierarchy of fibers with first-class cancellation support

#### Performant
- Issue non-blocking, batched, zero-copy syscalls with io_uring
- Efficient context switching with cooperative multitasking
- Atomic-free scheduler, parallelized manually if required

#### Quick to compile
- Compile only what you need using [cargo features](#Compile Time Flags)
- Minimal dependencies
- Minimal use of macros

## Quick Start
[Install Rust](https://www.rust-lang.org/tools/install) and [create a new cargo project](https://doc.rust-lang.org/book/ch01-03-hello-cargo.html).

Add uringy as a dependency: `cargo add uringy`

Then replace `src/main.rs` with:
```rust
// No need for async main
#[uringy::start]
fn main() {
    let handle = uringy::fiber::spawn(|| tcp_echo_server(9000)); // No need for async block

    uringy::signals().filter(Signal::is_terminal).next().unwrap();
    uringy::println!("gracefully shutting down");
    handle.cancel(); // Cancellation propagates throughout the entire fiber hierarchy

    // Automatically waits for all fibers to complete
}

// No need for async functions
fn tcp_echo_server(port: u16) {
    let listener = uringy::net::TcpListener::bind(("0.0.0.0", port)).unwrap();
    uringy::println!("listening for TCP connections on port {port}"); // No need for .await
    let mut connections = listener.incoming();
    while let Ok((stream, _)) = connections.next() {
        uringy::fiber::spawn(move || handle_connection(stream));
    }
}

fn handle_connection(tcp: TcpStream) {
    let (mut r, mut w) = stream.split();
    let _ = std::io::copy(&mut r, &mut w); // TcpStream implements std::io's Read and Write
}
```

And run your project using: `cargo run --release`

If you're using macOS, use a [Linux virtual machine](https://orbstack.dev) or a docker container.
If you're using Windows, use [WSL](https://learn.microsoft.com/en-us/windows/wsl/install).

For more, check out the [examples](examples) directory.

## Compile Time Flags
There are currently no cargo flags.

## Comparison with Other Runtimes
|                                                                                             | std thread                                 | uringy fiber                                 | tokio task                                                                                                                                  |
|---------------------------------------------------------------------------------------------|--------------------------------------------|----------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------|
| OS support                                                                                  | all                                        | Linux                                        | most                                                                                                                                        |
| IO interface                                                                                | blocking                                   | [io_uring](https://unixism.net/loti)         | epoll + thread pool                                                                                                                         |
| [function color](https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function) | sync                                       | sync                                         | sync and async                                                                                                                              |
| start                                                                                       | N/A                                        | 27 μs                                        | 27.5 μs (3.5 μs using [current thread scheduler](https://docs.rs/tokio/latest/tokio/runtime/struct.Runtime.html#current-thread-scheduler))  |
| spawn                                                                                       | 9828 ns                                    | 59 ns                                        | 907 ns (58ns using [current thread scheduler](https://docs.rs/tokio/latest/tokio/runtime/struct.Runtime.html#current-thread-scheduler))     |
| spawn `Send` bound                                                                          | yes                                        | no                                           | yes, unless using [LocalSet](https://docs.rs/tokio/latest/tokio/task/struct.LocalSet.html)                                                  |
| spawn `'static` bound                                                                       | yes, unless using scope                    | yes, unless using scope                      | yes                                                                                                                                         |
| [stack size](https://without.boats/blog/futures-and-segmented-stacks)                       | virtual 8MB (configurable), 4KB increments | virtual 128KB (configurable), 4KB increments | perfectly sized                                                                                                                             |
| stack limitations                                                                           | may overflow                               | may overflow                                 | can't use recursion                                                                                                                         |
| context switch                                                                              | 1405 ns                                    | 60 ns                                        | 1328 ns (308 ns using [current thread scheduler](https://docs.rs/tokio/latest/tokio/runtime/struct.Runtime.html#current-thread-scheduler))  |
| multi-tasking                                                                               | preemptive                                 | cooperative                                  | mostly cooperative                                                                                                                          |
| structured concurrency                                                                      | no guarantees                              | parent fiber outlives its children           | no guarantees                                                                                                                               |
| runs until                                                                                  | main thread completes                      | all fibers complete                          | block_on completes                                                                                                                          |
| parallelism                                                                                 | automatic                                  | manual                                       | automatic, unless using [current thread scheduler](https://docs.rs/tokio/latest/tokio/runtime/struct.Runtime.html#current-thread-scheduler) |
| userspace scheduler                                                                         | N/A                                        | minimal                                      | work stealing                                                                                                                               |
| cancellation                                                                                | using esoteric unix signals                | first class, voluntary                       | leaks memory, [causes bugs](https://docs.rs/tokio/latest/tokio/macro.select.html#cancellation-safety)                                       |

## Supported Rust Versions
The MSRV is 1.75.0 (released in December 2023).
Check your Rust version by running `rustc --version` in a terminal.

## Supported Linux Kernel Versions
The minimum kernel version is 6.1 (released in December 2022).
Check your kernel version by running `uname -r` in a terminal.

## License
Uringy is licensed under the [MIT license](LICENSE).
It's a permissive license, which basically means you can do whatever you want.

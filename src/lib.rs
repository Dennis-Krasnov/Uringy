#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]
#![warn(missing_debug_implementations)]
#![warn(rust_2018_idioms)]

//! # Uringy
//!
//! [![github]](https://github.com/Dennis-Krasnov/Uringy)
//! [![crates-io]](https://crates.io/crates/uringy)
//! [![docs-rs]](https://docs.rs/uringy)
//! [![license]](https://github.com/Dennis-Krasnov/Uringy/blob/master/LICENSE)
//!
//! [github]: https://img.shields.io/static/v1?label=github&message=Dennis-Krasnov/Uringy&style=for-the-badge&labelColor=555555&color=blue&logo=github
//! [crates-io]: https://img.shields.io/crates/v/uringy.svg?style=for-the-badge&logo=image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA1MTIgNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJNMjM5LjEgNi4zbC0yMDggNzhjLTE4LjcgNy0zMS4xIDI1LTMxLjEgNDV2MjI1LjFjMCAxOC4yIDEwLjMgMzQuOCAyNi41IDQyLjlsMjA4IDEwNGMxMy41IDYuOCAyOS40IDYuOCA0Mi45IDBsMjA4LTEwNGMxNi4zLTguMSAyNi41LTI0LjggMjYuNS00Mi45VjEyOS4zYzAtMjAtMTIuNC0zNy45LTMxLjEtNDQuOWwtMjA4LTc4QzI2MiAyLjIgMjUwIDIuMiAyMzkuMSA2LjN6TTI1NiA2OC40bDE5MiA3MnYxLjFsLTE5MiA3OC0xOTItNzh2LTEuMWwxOTItNzJ6bTMyIDM1NlYyNzUuNWwxNjAtNjV2MTMzLjlsLTE2MCA4MHoiPjwvcGF0aD48L3N2Zz4=
//! [docs-rs]: https://img.shields.io/static/v1?label=docs.rs&message=uringy&style=for-the-badge&labelColor=555555&color=red&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K
//! [license]: https://img.shields.io/static/v1?label=license&message=MIT&style=for-the-badge&labelColor=555555&color=yellowgreen&logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA2NDAgNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJNOTYgNDY0djMyYzAgOC44NCA3LjE2IDE2IDE2IDE2aDIyNGM4Ljg0IDAgMTYtNy4xNiAxNi0xNlYxNTMuMjVjNC41Ni0yIDguOTItNC4zNSAxMi45OS03LjEybDE0Mi4wNSA0Ny42M2M4LjM4IDIuODEgMTcuNDUtMS43MSAyMC4yNi0xMC4wOGwxMC4xNy0zMC4zNGMyLjgxLTguMzgtMS43MS0xNy40NS0xMC4wOC0yMC4yNmwtMTI4LjQtNDMuMDVjLjQyLTMuMzIgMS4wMS02LjYgMS4wMS0xMC4wMyAwLTQ0LjE4LTM1LjgyLTgwLTgwLTgwLTI5LjY5IDAtNTUuMyAxNi4zNi02OS4xMSA0MC4zN0wxMzIuOTYuODNjLTguMzgtMi44MS0xNy40NSAxLjcxLTIwLjI2IDEwLjA4bC0xMC4xNyAzMC4zNGMtMi44MSA4LjM4IDEuNzEgMTcuNDUgMTAuMDggMjAuMjZsMTMyIDQ0LjI2YzcuMjggMjEuMjUgMjIuOTYgMzguNTQgNDMuMzggNDcuNDdWNDQ4SDExMmMtOC44NCAwLTE2IDcuMTYtMTYgMTZ6TTAgMzA0YzAgNDQuMTggNTcuMzEgODAgMTI4IDgwczEyOC0zNS44MiAxMjgtODBoLS4wMmMwLTE1LjY3IDIuMDgtNy4yNS04NS4wNS0xODEuNTEtMTcuNjgtMzUuMzYtNjguMjItMzUuMjktODUuODcgMEMtMS4zMiAyOTUuMjcuMDIgMjg3LjgyLjAyIDMwNEgwem01Ni0xNmw3Mi0xNDQgNzIgMTQ0SDU2em0zMjguMDIgMTQ0SDM4NGMwIDQ0LjE4IDU3LjMxIDgwIDEyOCA4MHMxMjgtMzUuODIgMTI4LTgwaC0uMDJjMC0xNS42NyAyLjA4LTcuMjUtODUuMDUtMTgxLjUxLTE3LjY4LTM1LjM2LTY4LjIyLTM1LjI5LTg1Ljg3IDAtODYuMzggMTcyLjc4LTg1LjA0IDE2NS4zMy04NS4wNCAxODEuNTF6TTQ0MCA0MTZsNzItMTQ0IDcyIDE0NEg0NDB6Ij48L3BhdGg+PC9zdmc+
//!
//! A simple single-threaded async runtime for Rust based on io_uring.
//!
//! # Feature Flags
//! Uringy uses a set of [feature flags] to reduce the amount of compiled code.
//! It's ideal to enable only the features for the functionality that you use.
//! A set of sensible default features is enabled by default.
//! Below is the list of the available feature flags:
//!
//! - `fs`: Enables filesystem IO. (*enabled by default*)
//! - `quic`: ...
//! - `process`: ... and signal ... (or split up???)
//! - `time`: Enables timeouts and delays. (*enabled by default*)
//!
//! [feature flags]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section

#[cfg(feature = "fs")]
pub mod fs;
#[cfg(feature = "net")]
pub mod net;
#[cfg(feature = "process")]
pub mod process;
pub mod runtime;
pub mod sync;
#[cfg(feature = "time")]
pub mod time;

mod utils;

use crate::manager::ManagerState;
use crate::Inner;
use bipbuffer::BipBuffer;
use std::collections::VecDeque;
use std::rc::Rc;
use std::slice;
use uringy::sync::notify::Notify;

#[derive(Debug)]
pub(crate) struct WriterState {
    pub(crate) bipbuffer: BipBuffer<u8>,
    pub(crate) no_longer_full: Notify,
    pub(crate) no_longer_empty: Notify,
    // For skipping partially sent message on reconnect
    pub(crate) message_boundaries: VecDeque<usize>,
    pub(crate) total_bytes_sent: usize,
}

impl WriterState {
    pub(crate) fn new(capacity: usize) -> Self {
        let mut message_boundaries = VecDeque::with_capacity(capacity / 64);
        message_boundaries.push_back(0);

        WriterState {
            bipbuffer: BipBuffer::new(capacity),
            no_longer_empty: Notify::new(),
            no_longer_full: Notify::new(),
            message_boundaries,
            total_bytes_sent: 0,
        }
    }
}

pub(crate) async fn actor(connection: Rc<Inner>) {
    let mut local_tcp = None;

    loop {
        // ...
        {
            let mut state = connection.writer_state.borrow_mut();
            if state.bipbuffer.committed_len() == 0 {
                // Give up mutable borrow during await
                let waiter = state.no_longer_empty.waiter();
                drop(state);
                waiter.await;
            }
        }

        // latest tcp and .. from manager.
        let tcp = loop {
            let mut state = connection.manager_state.borrow_mut();

            match *state {
                ManagerState::Connected { ref mut writer, .. } => {
                    if let Some(tcp) = writer.take() {
                        local_tcp = Some(tcp);

                        // Skip partially sent message
                        {
                            let mut state = connection.writer_state.borrow_mut();
                            let next_boundary = state.message_boundaries[0];
                            let bytes_to_skip = next_boundary - state.total_bytes_sent;
                            assert!(state.bipbuffer.committed_len() >= bytes_to_skip);
                            state.bipbuffer.decommit(bytes_to_skip);
                            state.total_bytes_sent = next_boundary;
                        }
                    }

                    break local_tcp.as_mut().unwrap();
                }
                ManagerState::Disconnected {
                    ref mut connection_established,
                } => {
                    // Give up mutable borrow during await
                    let waiter = connection_established.waiter();
                    drop(state);
                    waiter.await;
                }
            }
        };

        // ...
        let (raw_buffer, buffer_length) = {
            let mut state = connection.writer_state.borrow_mut();
            let slice = state.bipbuffer.read().unwrap(); // FIXME none! // already waited until it's no longer empty...
            (slice.as_ptr(), slice.len())
        };

        // Safety: disjoint, synchronized by writer state's bipbuffer
        let slice = unsafe { slice::from_raw_parts(raw_buffer, buffer_length) };

        // Treat all tcp errors like disconnected
        let bytes_sent = unsafe { tcp.write(slice) }.await.unwrap_or(0);

        // ...
        {
            let mut state = connection.writer_state.borrow_mut();
            state.bipbuffer.decommit(bytes_sent);

            // Remove end boundaries of messages that have been fully sent. Keep at least one.
            state.total_bytes_sent += bytes_sent;
            // TODO: optimize with binary search + drain
            while let Some(&message_boundary) = state.message_boundaries.get(0) {
                if message_boundary < state.total_bytes_sent {
                    state.message_boundaries.pop_front().unwrap();
                } else {
                    break;
                }
            }

            // TCP socket failed in any way TODO: or timed out after 10s
            if bytes_sent == 0 {
                println!("writer detected that server disconnected");
                connection.manager_state.borrow_mut().disconnect();
                continue;
            }

            // cooperative concurrency with blocked writers...
            state.no_longer_full.notify_all();
        }
    }
}

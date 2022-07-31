use crate::Inner;
use nats_proto::client_operation::ClientOperation;
use nats_proto::error::NatsProtoError;
use nats_proto::server_operation::ServerOperation;
use std::cmp::min;
use std::net::SocketAddr;
use std::ops::Mul;
use std::rc::Rc;
use std::time::Duration;
use uringy::net::tcp::TcpStream;
use uringy::sync::notify::Notify;
use uringy::time;

#[derive(Debug)]
pub(crate) enum ManagerState {
    Connected {
        reader: Option<TcpStream>,
        writer: Option<TcpStream>,
        connection_broken: Notify,
    },
    Disconnected {
        connection_established: Notify,
    },
}

impl ManagerState {
    pub(crate) fn new() -> Self {
        ManagerState::Disconnected {
            connection_established: Notify::new(),
        }
    }

    pub(crate) fn disconnect(&mut self) {
        match self {
            ManagerState::Connected {
                connection_broken, ..
            } => connection_broken.notify_all(),
            ManagerState::Disconnected { .. } => {}
        }

        *self = ManagerState::Disconnected {
            connection_established: Notify::new(),
        };
    }
}

// responsible for reconnection and handing out tcp streams to other actors
pub(crate) async fn actor(connection: Rc<Inner>, initial_url: String) {
    let address: SocketAddr = initial_url.parse().unwrap();

    loop {
        let subscriptions = vec![];

        // TODO: pass in connection.reconnection_strategy (taken from options) for max_attempts
        let (tcp, _) = acquire_connection(address, &subscriptions, usize::MAX).await;

        let connection_broken = {
            let manager_state = &mut *connection.manager_state.borrow_mut();

            // ...
            if let ManagerState::Disconnected {
                connection_established,
            } = manager_state
            {
                connection_established.notify_all();
            }

            let mut connection_broken = Notify::new();
            let connection_broken_waiter = connection_broken.waiter();

            *manager_state = ManagerState::Connected {
                reader: Some(tcp.try_clone().unwrap()),
                writer: Some(tcp),
                connection_broken,
            };

            connection_broken_waiter
        };

        connection_broken.await;
    }
}

// responsible for reconnection strategy
async fn acquire_connection(
    address: SocketAddr, // most up-to-date list of addresses, dynamic info can't arrive during reconnect.
    subscriptions: &[u64],
    max_attempts: usize,
) -> (TcpStream, ServerInfo) {
    for attempt in 0..max_attempts {
        if let Ok(result) = attempt_connection(address, subscriptions).await {
            return result;
        }

        let base_delay = Duration::from_millis(1);
        let exponential_backoff = 1 << min(attempt, 10); // 1, 2, 4, ..., 1024
        let thundering_herd = 0.75 + fastrand::f32() / 2.0; // 0.75 -> 1.25
        time::sleep(base_delay.mul(exponential_backoff).mul_f32(thundering_herd)).await;
    }

    panic!("failed...")
}

// responsible for configuring a TCP connection
async fn attempt_connection(
    address: SocketAddr,
    subscriptions: &[u64],
) -> Result<(TcpStream, ServerInfo), ConnectionError> {
    let mut tcp = TcpStream::connect(address).await?;
    let server_info = handshake(&mut tcp).await?;
    resubscribe(&mut tcp, subscriptions).await?;
    Ok((tcp, server_info))
}

enum ConnectionError {
    /// ...
    HandshakeError(HandshakeError),

    /// ...
    IOError(std::io::Error),
}

impl From<HandshakeError> for ConnectionError {
    fn from(err: HandshakeError) -> ConnectionError {
        ConnectionError::HandshakeError(err)
    }
}

impl From<std::io::Error> for ConnectionError {
    fn from(err: std::io::Error) -> ConnectionError {
        ConnectionError::IOError(err)
    }
}

async fn handshake(tcp: &mut TcpStream) -> Result<ServerInfo, HandshakeError> {
    let server_info = server_hello(tcp).await?;
    client_hello(tcp, &server_info).await?;
    Ok(server_info)
}

async fn server_hello(tcp: &mut TcpStream) -> Result<ServerInfo, HandshakeError> {
    // ...
    let mut buffer = vec![0; 1024];

    // ...
    let bytes_read = unsafe { tcp.read(&mut buffer) }.await?;
    let (wire_size, server_operation) = ServerOperation::decode(&buffer[..bytes_read])?;
    assert_eq!(bytes_read, wire_size); // TODO: handle partial reads

    if let ServerOperation::Info { json } = server_operation {
        let json = json::parse(json)?;
        ServerInfo::try_from(json)
    } else {
        Err(HandshakeError::InvalidProtocol)
    }
}

async fn client_hello(
    tcp: &mut TcpStream,
    _server_info: &ServerInfo,
) -> Result<(), HandshakeError> {
    // ...
    let mut buffer = vec![0; 1024];

    let client_operation = ClientOperation::Connect {
        json: &json::object! {
            verbose: false,
            pedantic: true,
            tls_required: false,
            name: "uringy-nats",
            lang: "rust",
            version: env!("CARGO_PKG_VERSION"),
            protocol: 0, // dynamic reconfiguration of cluster topology
            echo: false, // ...
            headers: false, // support for hpub/hmsg operations
        }
        .dump(),
    };

    let wire_size = client_operation.encode(&mut buffer)?;
    let bytes_wrote = unsafe { tcp.write(&buffer[..wire_size]) }.await?;
    assert_eq!(bytes_wrote, wire_size); // TODO: handle partial writes

    Ok(())
}

async fn resubscribe(_tcp: &mut TcpStream, _subscriptions: &[u64]) -> std::io::Result<()> {
    // ...
    // let _bipbuffer: BipBuffer<u8> = BipBuffer::new(1024);

    // ...

    Ok(())
}

#[derive(Debug)]
struct ServerInfo {
    _server_id: String,
    _server_name: String,
    _version: String,
}

impl TryFrom<json::JsonValue> for ServerInfo {
    type Error = HandshakeError;

    fn try_from(json: json::JsonValue) -> Result<Self, Self::Error> {
        use HandshakeError::InvalidProtocol;

        let server_id = json["server_id"].as_str().ok_or(InvalidProtocol)?;
        let server_name = &json["server_name"].as_str().ok_or(InvalidProtocol)?;
        let version = &json["version"].as_str().ok_or(InvalidProtocol)?;
        // let proto = &json["proto"].as_u8().unwrap();
        // let git_commit = &json["git_commit"].as_str().unwrap();
        // let go = &json["go"].as_str().unwrap();
        // let host = &json["host"].as_str().unwrap(); // TODO: to ip addr
        // let port = &json["port"].as_u16().unwrap();
        // let headers = &json["headers"].as_bool().unwrap();
        // let max_payload = &json["max_payload"].as_usize().unwrap();
        // let jetstream = &json["jetstream"].as_bool().unwrap();
        // let client_id = &json["client_id"].as_usize().unwrap();
        // let client_ip = &json["client_ip"].as_str().unwrap(); // TODO: to ip addr

        Ok(ServerInfo {
            _server_id: server_id.to_string(),
            _server_name: server_name.to_string(),
            _version: version.to_string(),
        })
    }
}

#[derive(Debug)]
enum HandshakeError {
    /// ...
    InvalidProtocol,

    /// ...
    BufferTooSmall,

    /// ...
    IOError(std::io::Error),
}

impl std::error::Error for HandshakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            HandshakeError::InvalidProtocol => None, // TODO: source
            HandshakeError::BufferTooSmall => None,
            HandshakeError::IOError(_) => None, // TODO: source
        }
    }
}

impl std::fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            HandshakeError::InvalidProtocol => write!(f, "Invalid protocol"),
            HandshakeError::BufferTooSmall => write!(f, "Buffer too small"),
            HandshakeError::IOError(_err) => write!(f, "IO error..."),
            // TODO: HandshakeError::IOError(ref err) => err.fmt(f),
        }
    }
}

impl From<std::io::Error> for HandshakeError {
    fn from(err: std::io::Error) -> HandshakeError {
        HandshakeError::IOError(err)
    }
}

impl From<NatsProtoError> for HandshakeError {
    fn from(err: NatsProtoError) -> HandshakeError {
        match err {
            NatsProtoError::BufferTooSmall => HandshakeError::BufferTooSmall,
            NatsProtoError::InvalidProtocol => HandshakeError::InvalidProtocol,
        }
    }
}

impl From<json::Error> for HandshakeError {
    fn from(_: json::Error) -> Self {
        HandshakeError::InvalidProtocol
    }
}

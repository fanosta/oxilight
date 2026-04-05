use std::env;
use std::os::unix::net::UnixDatagram;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotifyError {
    #[error("notify() requires a message")]
    MissingMessage,

    #[error("unsupported socket type")]
    UnsupportedSocketType,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Time(#[from] std::time::SystemTimeError),
}

fn notify(message: &[u8]) -> Result<(), NotifyError> {
    if message.is_empty() {
        return Err(NotifyError::MissingMessage);
    }

    let mut socket_path = match env::var("NOTIFY_SOCKET") {
        Ok(p) => p,
        Err(_) => return Ok(()), // no socket -> no-op
    };

    if !socket_path.starts_with('/') && !socket_path.starts_with('@') {
        return Err(NotifyError::UnsupportedSocketType);
    }

    // abstract socket (@ -> \0)
    if socket_path.starts_with('@') {
        socket_path.replace_range(0..1, "\0");
    }

    let sock = UnixDatagram::unbound()?;
    sock.connect(&socket_path)?;
    sock.send(message)?;

    Ok(())
}

pub fn notify_ready() -> Result<(), NotifyError> {
    notify(b"READY=1")
}

// pub fn notify_reloading() -> Result<(), NotifyError> {
//     let micros = SystemTime::now()
//         .duration_since(UNIX_EPOCH)?
//         .as_micros();
//
//     let msg = format!("RELOADING=1\nMONOTONIC_USEC={micros}");
//     notify(msg.as_bytes())
// }
//
// pub fn notify_stopping() -> Result<(), NotifyError> {
//     notify(b"STOPPING=1")
// }

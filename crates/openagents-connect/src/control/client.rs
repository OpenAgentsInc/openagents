//! Blocking typed operations over the existing same-user control socket.
use super::{MAX_MESSAGE_BYTES, Op, Reply, Request, Response, VERSION};
use coder_access::{Code, Error, Operation, Outcome};
use std::path::PathBuf;
use std::time::Duration;

/// The host's control socket client: one connection per
/// operation, carrying `openagents.control.v1` length-prefixed JSON.
#[derive(Debug)]
pub struct OperationClient {
    path: PathBuf,
    next: u64,
}

impl OperationClient {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path, next: 1 }
    }

    /// One request on the socket and the host's reply.
    #[cfg(unix)]
    fn exchange(&mut self, op: Op) -> coder_access::Result<Reply> {
        use std::io::{Read, Write};
        let unreachable = || Error::new(Code::Unavailable, "the host does not answer its socket");
        let malformed = || Error::new(Code::Malformed, "the host's answer is malformed");
        let id = self.next;
        self.next += 1;
        let body = serde_json::to_vec(&Request::new(id, op)).map_err(|_| malformed())?;
        if body.len() > MAX_MESSAGE_BYTES {
            return Err(Error::new(Code::Bounds, "the request exceeds one message"));
        }
        let mut stream =
            std::os::unix::net::UnixStream::connect(&self.path).map_err(|_| unreachable())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(10))))
            .map_err(|_| unreachable())?;
        let length = u32::try_from(body.len()).map_err(|_| malformed())?;
        let mut frame = Vec::with_capacity(4 + body.len());
        frame.extend(length.to_be_bytes());
        frame.extend(body);
        stream
            .write_all(&frame)
            .and_then(|()| stream.flush())
            .map_err(|_| unreachable())?;
        let mut length = [0u8; 4];
        stream.read_exact(&mut length).map_err(|_| unreachable())?;
        let length = u32::from_be_bytes(length) as usize;
        if length > MAX_MESSAGE_BYTES {
            return Err(malformed());
        }
        let mut body = vec![0u8; length];
        stream.read_exact(&mut body).map_err(|_| unreachable())?;
        let response: Response = serde_json::from_slice(&body).map_err(|_| malformed())?;
        if response.v != VERSION || response.id != id {
            return Err(malformed());
        }
        Ok(response.result)
    }

    /// The control socket is a Unix socket here; elsewhere there is none
    /// to reach.
    #[cfg(not(unix))]
    fn exchange(&mut self, _op: Op) -> coder_access::Result<Reply> {
        Err(Error::new(
            Code::Unavailable,
            "the studio reaches a host only over its Unix control socket",
        ))
    }
}

impl OperationClient {
    pub fn call(&mut self, request: &str, operation: &Operation) -> coder_access::Result<Outcome> {
        operation.validate()?;
        let reply = self.exchange(Op::Task {
            request: request.into(),
            operation: operation.clone(),
        })?;
        match reply {
            Reply::Task { outcome } if outcome.answers(operation) => {
                outcome.validate()?;
                Ok(outcome)
            }
            Reply::Refused { code, message } => Err(Error::new(refusal(&code), message)),
            _ => Err(Error::new(
                Code::Malformed,
                "the host answered another operation",
            )),
        }
    }
}

/// The refusal code a reply names, or `unavailable` for one this client
/// does not know.
fn refusal(code: &str) -> Code {
    serde_json::from_value(serde_json::Value::String(code.into())).unwrap_or(Code::Unavailable)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;

    fn exchange(reply: Reply) -> (coder_access::Result<Outcome>, Request) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("control.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut size = [0; 4];
            socket.read_exact(&mut size).unwrap();
            let mut body = vec![0; u32::from_be_bytes(size) as usize];
            socket.read_exact(&mut body).unwrap();
            let request: Request = serde_json::from_slice(&body).unwrap();
            let response = serde_json::to_vec(&Response::new(request.id, reply)).unwrap();
            socket
                .write_all(&(response.len() as u32).to_be_bytes())
                .unwrap();
            socket.write_all(&response).unwrap();
            request
        });
        let result = OperationClient::new(path).call(
            &"a".repeat(64),
            &Operation::PauseSeat { seat: "ada".into() },
        );
        (result, worker.join().unwrap())
    }

    #[test]
    fn socket_preserves_operation_and_request_identity() {
        let (result, request) = exchange(Reply::Task {
            outcome: Outcome::Dispatched {
                receipt: coder_access::protocol::Receipt {
                    operation: "studio.seat.pause".into(),
                    reference: "ada".into(),
                },
            },
        });
        assert!(result.is_ok());
        assert!(
            matches!(request.op, Op::Task { request, operation: Operation::PauseSeat { seat } }
            if request == "a".repeat(64) && seat == "ada")
        );
    }

    #[test]
    fn socket_preserves_host_refusal_and_rejects_another_receipt() {
        let (result, _) = exchange(Reply::Refused {
            code: "revoked".into(),
            message: "grant revoked".into(),
        });
        assert_eq!(result.unwrap_err().code, Code::Revoked);
        let (result, _) = exchange(Reply::Task {
            outcome: Outcome::Dispatched {
                receipt: coder_access::protocol::Receipt {
                    operation: "studio.seat.stop".into(),
                    reference: "ada".into(),
                },
            },
        });
        assert_eq!(result.unwrap_err().code, Code::Malformed);
    }
}

//! Browser binary WebSocket carriage of the existing NIP-REACH channel.
//! The DOM owns network allocation before an event is delivered. This adapter
//! bounds retained messages and outbound bytes; it cannot bound that DOM allocation.
use crate::websocket_frame::{MAX_MESSAGE_BYTES, check_message};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{BinaryType, Event, MessageEvent, WebSocket};
const BUFFER: usize = MAX_MESSAGE_BYTES * 4;
struct Inbox {
    messages: VecDeque<Vec<u8>>,
    offset: usize,
    bytes: usize,
    ended: bool,
    failed: bool,
    read: Option<Waker>,
    write: Option<Waker>,
}
impl Inbox {
    fn wake(&mut self) {
        if let Some(w) = self.read.take() {
            w.wake();
        }
        if let Some(w) = self.write.take() {
            w.wake();
        }
    }
}
pub struct Socket {
    socket: WebSocket,
    inbox: Rc<RefCell<Inbox>>,
    outgoing: Vec<u8>,
    waiting: Rc<Cell<bool>>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _open: Closure<dyn FnMut(Event)>,
    _close: Closure<dyn FnMut(Event)>,
    _error: Closure<dyn FnMut(Event)>,
}
fn error() -> io::Error {
    io::Error::other("Browser direct channel unavailable or malformed")
}
impl Socket {
    /// Opens the exact supplied route. Host identity is proved by `channel::connect`.
    pub async fn open(url: &str) -> io::Result<Self> {
        let parsed = url::Url::parse(url).map_err(|_| error())?;
        if !matches!(parsed.scheme(), "ws" | "wss")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return Err(error());
        }
        let socket = WebSocket::new(url).map_err(|_| error())?;
        socket.set_binary_type(BinaryType::Arraybuffer);
        let inbox = Rc::new(RefCell::new(Inbox {
            messages: VecDeque::new(),
            offset: 0,
            bytes: 0,
            ended: false,
            failed: false,
            read: None,
            write: None,
        }));
        let state = inbox.clone();
        let ws = socket.clone();
        let message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let mut state = state.borrow_mut();
            if state.ended {
                return;
            }
            if let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() {
                let bytes = js_sys::Uint8Array::new(&buffer);
                let n = bytes.length() as usize;
                if n <= MAX_MESSAGE_BYTES && state.bytes + n <= BUFFER {
                    let data = bytes.to_vec();
                    if check_message(&data).is_ok() {
                        state.bytes += n;
                        state.messages.push_back(data);
                        state.wake();
                        return;
                    }
                }
            }
            state.failed = true;
            state.ended = true;
            state.messages.clear();
            state.bytes = 0;
            let _ = ws.close();
            state.wake();
        }) as Box<dyn FnMut(MessageEvent)>);
        let state = inbox.clone();
        let open = Closure::wrap(
            Box::new(move |_: Event| state.borrow_mut().wake()) as Box<dyn FnMut(Event)>
        );
        let state = inbox.clone();
        let close = Closure::wrap(Box::new(move |_: Event| {
            let mut s = state.borrow_mut();
            s.ended = true;
            s.wake();
        }) as Box<dyn FnMut(Event)>);
        let state = inbox.clone();
        let failure = Closure::wrap(Box::new(move |_: Event| {
            let mut s = state.borrow_mut();
            s.ended = true;
            s.failed = true;
            s.wake();
        }) as Box<dyn FnMut(Event)>);
        socket.set_onmessage(Some(message.as_ref().unchecked_ref()));
        socket.set_onopen(Some(open.as_ref().unchecked_ref()));
        socket.set_onclose(Some(close.as_ref().unchecked_ref()));
        socket.set_onerror(Some(failure.as_ref().unchecked_ref()));
        let result = Self {
            socket,
            inbox,
            outgoing: Vec::new(),
            waiting: Rc::new(Cell::new(false)),
            _message: message,
            _open: open,
            _close: close,
            _error: failure,
        };
        futures_util::future::poll_fn(|cx| {
            let mut state = result.inbox.borrow_mut();
            if state.ended {
                Poll::Ready(Err(error()))
            } else if result.socket.ready_state() == WebSocket::OPEN {
                Poll::Ready(Ok(()))
            } else {
                state.write = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await?;
        Ok(result)
    }
    fn send(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.inbox.borrow().ended || self.socket.ready_state() != WebSocket::OPEN {
            return Poll::Ready(Err(error()));
        }
        while self.outgoing.len() >= 4 {
            let len = u32::from_be_bytes(self.outgoing[..4].try_into().unwrap()) as usize;
            if !(9..=MAX_MESSAGE_BYTES - 4).contains(&len) {
                return Poll::Ready(Err(error()));
            }
            if self.outgoing.len() < len + 4 {
                break;
            }
            if self.socket.buffered_amount() as usize > BUFFER - MAX_MESSAGE_BYTES {
                self.inbox.borrow_mut().write = Some(cx.waker().clone());
                if !self.waiting.replace(true) {
                    let state = self.inbox.clone();
                    let waiting = self.waiting.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(10).await;
                        waiting.set(false);
                        state.borrow_mut().wake();
                    });
                }
                return Poll::Pending;
            }
            self.waiting.set(false);
            self.socket
                .send_with_u8_array(&self.outgoing[..len + 4])
                .map_err(|_| error())?;
            self.outgoing.drain(..len + 4);
        }
        Poll::Ready(Ok(()))
    }
}
impl AsyncRead for Socket {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut s = self.inbox.borrow_mut();
        if s.failed {
            return Poll::Ready(Err(error()));
        }
        if let Some(data) = s.messages.front() {
            let n = buf.remaining().min(data.len() - s.offset);
            buf.put_slice(&data[s.offset..s.offset + n]);
            s.offset += n;
            s.bytes -= n;
            if s.offset == s.messages.front().unwrap().len() {
                s.messages.pop_front();
                s.offset = 0;
            }
            return Poll::Ready(Ok(()));
        }
        if s.ended {
            Poll::Ready(Ok(()))
        } else {
            s.read = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
impl AsyncWrite for Socket {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        std::task::ready!(this.send(cx))?;
        let n = buf.len().min(MAX_MESSAGE_BYTES - this.outgoing.len());
        if n == 0 {
            return Poll::Ready(Err(error()));
        }
        this.outgoing.extend_from_slice(&buf[..n]);
        Poll::Ready(Ok(n))
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.get_mut().send(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        std::task::ready!(this.send(cx))?;
        if !this.outgoing.is_empty() {
            return Poll::Ready(Err(error()));
        }
        this.socket.close().map_err(|_| error())?;
        Poll::Ready(Ok(()))
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        self.socket.set_onmessage(None);
        self.socket.set_onopen(None);
        self.socket.set_onclose(None);
        self.socket.set_onerror(None);
        let _ = self.socket.close();
    }
}

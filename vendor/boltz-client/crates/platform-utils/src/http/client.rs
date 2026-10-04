//! HTTP client using reqwest for both native and WASM targets.

use std::collections::HashMap;
use std::time::Duration;

use futures::StreamExt;

use super::{HttpClient, HttpError, HttpResponse, MAX_RESPONSE_BYTES, REQUEST_TIMEOUT};
use crate::proxy::ProxyConfig;

/// The charset declared in `Content-Type`, defaulting to UTF-8.
///
/// Mirrors `reqwest::Response::text`, which [`read_capped_text`] replaces: an
/// absent or unrecognised charset decodes as UTF-8.
fn body_encoding(headers: &reqwest::header::HeaderMap) -> &'static encoding_rs::Encoding {
    headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<mime::Mime>().ok())
        .and_then(|mime| {
            encoding_rs::Encoding::for_label(mime.get_param("charset")?.as_str().as_bytes())
        })
        .unwrap_or(encoding_rs::UTF_8)
}

/// Buffers the response body, refusing it once it passes `limit`.
///
/// `Content-Length` is only a fast path. The running count over the streamed
/// bytes is the authority, because that header is absent on a chunked response
/// and on WASM reports the compressed size of a body the browser hands over
/// already decoded. Dropping the response cancels the transfer, so nothing is
/// read past the cap.
pub async fn read_capped_bytes(
    response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, HttpError> {
    let too_large = || HttpError::Body(format!("response exceeds the {limit} byte limit"));
    if response
        .content_length()
        .is_some_and(|len| len > limit as u64)
    {
        return Err(too_large());
    }

    let mut buf = Vec::new();
    let mut stream = std::pin::pin!(response.bytes_stream());
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if buf.len().saturating_add(chunk.len()) > limit {
            return Err(too_large());
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

/// Like [`read_capped_bytes`], decoding the body the way `Response::text` does.
pub async fn read_capped_text(
    response: reqwest::Response,
    limit: usize,
) -> Result<String, HttpError> {
    let encoding = body_encoding(response.headers());
    let buf = read_capped_bytes(response, limit).await?;
    Ok(encoding.decode(&buf).0.into_owned())
}

/// Sends `req` and reads its response through the [`MAX_RESPONSE_BYTES`] cap.
async fn send(req: reqwest::RequestBuilder) -> Result<HttpResponse, HttpError> {
    let response = req.send().await?;
    let status = response.status().as_u16();
    let body = read_capped_text(response, MAX_RESPONSE_BYTES).await?;
    tracing::debug!("Received response, status: {status}");
    tracing::trace!("raw response body: {body}");

    Ok(HttpResponse { status, body })
}

/// HTTP client implementation backed by reqwest.
pub struct ReqwestHttpClient {
    client: reqwest::Client,
}

impl ReqwestHttpClient {
    /// Create a new `ReqwestHttpClient` with an optional user agent.
    ///
    /// Native targets layer HTTP/2 and TCP keepalives on top of reqwest's
    /// defaults so a long-lived shared client survives intermediaries that
    /// reap idle HTTP/2 flows. On WASM the browser owns connection management
    /// and these knobs aren't exposed.
    ///
    /// The `user_agent` is applied on native only. In the browser a
    /// script-set `User-Agent` is honored by Firefox/Safari but dropped by
    /// Chrome; because it is not CORS-safelisted, setting it turns otherwise
    /// simple requests into preflighted ones, and some third-party endpoints
    /// (e.g. the USDT0 deployments API) reject the preflight. Browsers send
    /// their own `User-Agent` regardless, so omitting it loses nothing. This
    /// is a deliberate divergence from the upstream `spark-sdk` client, which
    /// only talks to servers it controls and so can keep the override.
    ///
    /// Fails when reqwest cannot assemble a client: an invalid `user_agent`
    /// (one that is not a valid header value), or a TLS backend or system
    /// resolver that will not initialise.
    pub fn new(user_agent: Option<String>) -> Result<Self, HttpError> {
        Self::with_proxy(user_agent, None)
    }

    /// Like [`Self::new`], but routes every request through `proxy`.
    ///
    /// Setting a proxy also switches reqwest off system-proxy autodetection,
    /// so no environment variable can redirect traffic around it. A proxy that
    /// can't be reached fails the request: there is no direct fallback.
    ///
    /// A proxy is rejected on WASM: `fetch` exposes no proxy control, so
    /// honouring it is impossible and ignoring it would connect direct behind
    /// the caller's back.
    // `user_agent` is unused on WASM by design (see above); the signature is
    // kept uniform across targets and to match the upstream client.
    #[cfg_attr(
        all(target_family = "wasm", target_os = "unknown"),
        expect(clippy::needless_pass_by_value, unused_variables)
    )]
    pub fn with_proxy(
        user_agent: Option<String>,
        proxy: Option<&ProxyConfig>,
    ) -> Result<Self, HttpError> {
        let builder = reqwest::Client::builder();

        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        let builder = {
            let mut builder = builder;
            if let Some(ua) = user_agent {
                builder = builder.user_agent(ua);
            }
            if let Some(proxy) = proxy {
                builder = builder.proxy(reqwest::Proxy::all(proxy.reqwest_url())?);
            }
            builder
                .tcp_keepalive(Some(Duration::from_mins(1)))
                .http2_keep_alive_interval(Duration::from_secs(30))
                .http2_keep_alive_timeout(Duration::from_secs(10))
                .http2_keep_alive_while_idle(true)
        };

        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        if proxy.is_some() {
            return Err(HttpError::Builder(
                "a SOCKS5 proxy cannot be honoured on WASM: fetch exposes no proxy control"
                    .to_string(),
            ));
        }

        Ok(Self {
            client: builder.build()?,
        })
    }
}

#[macros::async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get(
        &self,
        url: String,
        headers: Option<HashMap<String, String>>,
    ) -> Result<HttpResponse, HttpError> {
        tracing::debug!("Making GET request to: {url}");
        let mut req = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT));

        if let Some(headers) = headers {
            for (key, value) in &headers {
                req = req.header(key, value);
            }
        }

        send(req).await
    }

    async fn post(
        &self,
        url: String,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    ) -> Result<HttpResponse, HttpError> {
        tracing::debug!("Making POST request to: {url}");
        let mut req = self
            .client
            .post(&url)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT));

        if let Some(headers) = headers {
            for (key, value) in &headers {
                req = req.header(key, value);
            }
        }
        if let Some(body) = body {
            req = req.body(body);
        }

        send(req).await
    }

    async fn delete(
        &self,
        url: String,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    ) -> Result<HttpResponse, HttpError> {
        tracing::debug!("Making DELETE request to: {url}");
        let mut req = self
            .client
            .delete(&url)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT));

        if let Some(headers) = headers {
            for (key, value) in &headers {
                req = req.header(key, value);
            }
        }
        if let Some(body) = body {
            req = req.body(body);
        }

        send(req).await
    }
}
#[cfg(all(test, not(all(target_family = "wasm", target_os = "unknown"))))]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    use super::{HttpClient, HttpError, MAX_RESPONSE_BYTES, ReqwestHttpClient};

    /// Filler byte the oversized bodies are built from.
    const FILL: u8 = b'a';

    /// Size of each body write the one-shot server makes.
    const BLOCK: usize = 64 * 1024;

    /// How the one-shot server frames the body it writes.
    enum Body {
        /// Exact bytes, written as-is.
        Literal(Vec<u8>),
        /// `len` filler bytes, unframed.
        Filler(usize),
        /// `len` filler bytes in HTTP/1.1 `chunked` framing.
        ChunkedFiller(usize),
        /// Exact bytes, one `chunked` frame per entry, so a test can choose
        /// where the chunk boundaries fall.
        ChunkedLiteral(Vec<Vec<u8>>),
    }

    /// Serves exactly one request: writes `head` verbatim, then `body`, giving
    /// up early if the client hangs up.
    ///
    /// Returns the URL it is bound to and a handle yielding the number of body
    /// bytes it managed to write, which is how the tests observe that the
    /// client stopped reading rather than buffering everything on offer.
    async fn serve_once(head: String, body: Body) -> (String, JoinHandle<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("http://{}/", listener.local_addr().expect("local addr"));

        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");

            // Drain the request head so the client's send completes.
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                match socket.read(&mut byte).await {
                    Ok(0) | Err(_) => return 0,
                    Ok(_) => request.push(byte[0]),
                }
            }

            if socket.write_all(head.as_bytes()).await.is_err() {
                return 0;
            }

            let (total, chunked) = match body {
                Body::Literal(bytes) => {
                    return match socket.write_all(&bytes).await {
                        Ok(()) => bytes.len(),
                        Err(_) => 0,
                    };
                }
                Body::ChunkedLiteral(chunks) => {
                    let mut written = 0usize;
                    for chunk in chunks {
                        let mut frame = format!("{:x}\r\n", chunk.len()).into_bytes();
                        frame.extend_from_slice(&chunk);
                        frame.extend_from_slice(b"\r\n");
                        if socket.write_all(&frame).await.is_err() {
                            return written;
                        }
                        written = written.saturating_add(chunk.len());
                    }
                    let _ = socket.write_all(b"0\r\n\r\n").await;
                    return written;
                }
                Body::Filler(len) => (len, false),
                Body::ChunkedFiller(len) => (len, true),
            };

            let mut written = 0usize;
            while written < total {
                let len = BLOCK.min(total.saturating_sub(written));
                let payload = vec![FILL; len];
                let frame = if chunked {
                    let mut frame = format!("{len:x}\r\n").into_bytes();
                    frame.extend_from_slice(&payload);
                    frame.extend_from_slice(b"\r\n");
                    frame
                } else {
                    payload
                };
                if socket.write_all(&frame).await.is_err() {
                    break;
                }
                written = written.saturating_add(len);
            }
            if chunked && written == total {
                let _ = socket.write_all(b"0\r\n\r\n").await;
            }
            let _ = socket.flush().await;
            written
        });

        (url, handle)
    }

    fn client() -> ReqwestHttpClient {
        ReqwestHttpClient::new(None).expect("build client")
    }

    fn assert_too_large(err: &HttpError) {
        assert!(
            matches!(err, HttpError::Body(message) if message.contains("byte limit")),
            "expected the body-limit error, got {err:?}"
        );
    }

    #[macros::async_test_not_wasm]
    async fn rejects_oversized_content_length() {
        let declared = MAX_RESPONSE_BYTES.saturating_mul(10);
        let (url, server) = serve_once(
            format!("HTTP/1.1 200 OK\r\nContent-Length: {declared}\r\n\r\n"),
            Body::Filler(declared),
        )
        .await;

        assert_too_large(&client().get(url, None).await.expect_err("should refuse"));

        // Refused on the advertised length alone, so the body is never read:
        // the server only gets as far as the socket buffers let it.
        let written = server.await.expect("server task");
        assert!(
            written < MAX_RESPONSE_BYTES,
            "server wrote {written} bytes, so the client was still reading"
        );
    }

    #[macros::async_test_not_wasm]
    async fn rejects_oversized_chunked_body() {
        let offered = MAX_RESPONSE_BYTES.saturating_mul(10);
        let (url, server) = serve_once(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_string(),
            Body::ChunkedFiller(offered),
        )
        .await;

        assert_too_large(&client().get(url, None).await.expect_err("should refuse"));

        // No Content-Length to reject on, so the running total is what stops
        // it: the server is cut off near the cap, not at what it offered.
        let written = server.await.expect("server task");
        assert!(
            written < MAX_RESPONSE_BYTES.saturating_mul(2),
            "server wrote {written} bytes of the {offered} it offered"
        );
    }

    #[macros::async_test_not_wasm]
    async fn rejects_oversized_error_body() {
        let (url, server) = serve_once(
            "HTTP/1.1 500 Internal Server Error\r\nTransfer-Encoding: chunked\r\n\r\n".to_string(),
            Body::ChunkedFiller(MAX_RESPONSE_BYTES.saturating_mul(10)),
        )
        .await;

        assert_too_large(&client().get(url, None).await.expect_err("should refuse"));
        server.await.expect("server task");
    }

    #[macros::async_test_not_wasm]
    async fn rejects_chunked_body_one_byte_over_the_cap() {
        let (url, server) = serve_once(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_string(),
            Body::ChunkedFiller(MAX_RESPONSE_BYTES.saturating_add(1)),
        )
        .await;

        assert_too_large(&client().get(url, None).await.expect_err("should refuse"));
        server.await.expect("server task");
    }

    #[macros::async_test_not_wasm]
    async fn accepts_chunked_body_at_the_cap() {
        let (url, server) = serve_once(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_string(),
            Body::ChunkedFiller(MAX_RESPONSE_BYTES),
        )
        .await;

        let response = client().get(url, None).await.expect("should accept");
        assert_eq!(response.status, 200);
        assert_eq!(response.body.len(), MAX_RESPONSE_BYTES);
        server.await.expect("server task");
    }

    /// Builds a fixed-length 200 response head for `body`.
    fn head_for(content_type: &str, body: &[u8]) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
    }

    /// Serves `body` under `content_type` and returns the decoded text.
    async fn decoded(content_type: &str, body: Vec<u8>) -> String {
        let (url, server) = serve_once(head_for(content_type, &body), Body::Literal(body)).await;
        let response = client().get(url, None).await.expect("should accept");
        server.await.expect("server task");
        response.body
    }

    #[macros::async_test_not_wasm]
    async fn caps_post_and_delete_as_well_as_get() {
        // All three share one read path; this fails if any is reverted to
        // reading the body without the cap.
        let offered = MAX_RESPONSE_BYTES.saturating_mul(10);
        let head = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";

        let (url, server) = serve_once(head.to_string(), Body::ChunkedFiller(offered)).await;
        assert_too_large(
            &client()
                .post(url, None, Some("{}".to_string()))
                .await
                .expect_err("post should refuse"),
        );
        server.await.expect("server task");

        let (url, server) = serve_once(head.to_string(), Body::ChunkedFiller(offered)).await;
        assert_too_large(
            &client()
                .delete(url, None, None)
                .await
                .expect_err("delete should refuse"),
        );
        server.await.expect("server task");
    }

    #[macros::async_test_not_wasm]
    async fn accepts_fixed_length_body_at_the_cap() {
        // Guards the `>` in the Content-Length fast path: a body of exactly the
        // cap is allowed, not refused off by one.
        let (url, server) = serve_once(
            format!("HTTP/1.1 200 OK\r\nContent-Length: {MAX_RESPONSE_BYTES}\r\n\r\n"),
            Body::Filler(MAX_RESPONSE_BYTES),
        )
        .await;

        let response = client().get(url, None).await.expect("should accept");
        assert_eq!(response.body.len(), MAX_RESPONSE_BYTES);
        server.await.expect("server task");
    }

    #[macros::async_test_not_wasm]
    async fn decodes_a_character_split_across_chunk_boundaries() {
        // `é` is 0xC3 0xA9, deliberately straddling two chunks. Decoding has to
        // happen once the whole body is buffered: a per-chunk decode would
        // yield replacement characters here.
        let (url, server) = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nTransfer-Encoding: chunked\r\n\r\n"
                .to_string(),
            Body::ChunkedLiteral(vec![vec![b'C', b'a', b'f', 0xC3], vec![0xA9, b'!']]),
        )
        .await;

        let response = client().get(url, None).await.expect("should accept");
        assert_eq!(response.body, "Café!");
        server.await.expect("server task");
    }

    #[macros::async_test_not_wasm]
    async fn falls_back_to_utf8_when_the_charset_is_unusable() {
        let utf8 = "né".as_bytes().to_vec();
        // Unknown label, and a Content-Type that is not a MIME type at all.
        assert_eq!(
            decoded("text/plain; charset=no-such-charset", utf8.clone()).await,
            "né"
        );
        assert_eq!(decoded("not a mime type", utf8.clone()).await, "né");
        assert_eq!(decoded("application/json", utf8).await, "né");
    }

    #[macros::async_test_not_wasm]
    async fn honours_a_quoted_charset_param() {
        assert_eq!(
            decoded("text/plain; charset=\"iso-8859-1\"", vec![b'n', 0xE9]).await,
            "né"
        );
    }

    #[macros::async_test_not_wasm]
    async fn replaces_invalid_bytes_instead_of_failing() {
        // Parity with `Response::text`, which decodes lossily rather than
        // erroring, so a malformed body still surfaces its status to the caller.
        assert_eq!(decoded("text/plain", vec![b'n', 0xFF]).await, "n\u{FFFD}");
    }

    #[macros::async_test_not_wasm]
    async fn sniffs_a_utf16_bom() {
        // `Encoding::decode` picks the encoding from a leading BOM whatever
        // `Content-Type` claims, matching `Response::text`. 0xFF 0xFE is the
        // UTF-16LE BOM, so these bytes are text, not invalid UTF-8.
        let body = vec![0xFF, 0xFE, b'n', 0x00];
        assert_eq!(decoded("text/plain; charset=utf-8", body).await, "n");
    }

    #[macros::async_test_not_wasm]
    async fn strips_a_utf8_bom() {
        // `Encoding::decode` sniffs the BOM, as `Response::text` does. Leaving
        // it in place would break `serde_json` on the first byte.
        let body = vec![0xEF, 0xBB, 0xBF, b'{', b'}'];
        assert_eq!(decoded("application/json", body).await, "{}");
    }

    #[macros::async_test_not_wasm]
    async fn reads_an_empty_body() {
        let (url, server) = serve_once(
            "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_string(),
            Body::Literal(Vec::new()),
        )
        .await;

        let response = client().get(url, None).await.expect("should accept");
        assert_eq!(response.status, 204);
        assert!(response.body.is_empty());
        server.await.expect("server task");
    }

    #[macros::async_test_not_wasm]
    async fn decodes_declared_charset() {
        // 0xE9 is `é` in ISO-8859-1 and not valid UTF-8, so a plain lossy
        // decode would drop it. Guards parity with `Response::text`.
        let body = vec![b'C', b'a', b'f', 0xE9];
        let (url, server) = serve_once(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=iso-8859-1\r\n\
                 Content-Length: {}\r\n\r\n",
                body.len()
            ),
            Body::Literal(body),
        )
        .await;

        let response = client().get(url, None).await.expect("should accept");
        assert_eq!(response.body, "Café");
        server.await.expect("server task");
    }

    #[macros::async_test_not_wasm]
    async fn reads_an_ordinary_body_unchanged() {
        let body = br#"{"status":"OK"}"#.to_vec();
        let (url, server) = serve_once(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                body.len()
            ),
            Body::Literal(body),
        )
        .await;

        let response = client().get(url, None).await.expect("should accept");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, r#"{"status":"OK"}"#);
        server.await.expect("server task");
    }
}

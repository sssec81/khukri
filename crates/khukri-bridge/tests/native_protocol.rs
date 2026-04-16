use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use axum::{
    extract::State,
    http::HeaderMap,
    response::Response,
    routing::get,
    Router,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;

#[derive(Debug, serde::Deserialize)]
struct BridgeEvent {
    #[serde(rename = "type")]
    kind: String,
    status: String,
    bytes_done: u64,
    total_bytes: Option<u64>,
    output_path: Option<String>,
    message: Option<String>,
}

fn payload_10mb() -> Arc<Vec<u8>> {
    Arc::new(
        (0u32..10 * 1024 * 1024)
            .map(|i| i.wrapping_mul(31) as u8)
            .collect(),
    )
}

fn sha256(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

async fn spawn_server(router: Router) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.ok();
    });
    addr
}

async fn range_handler(
    headers: HeaderMap,
    State(data): State<Arc<Vec<u8>>>,
) -> Response<axum::body::Body> {
    let total = data.len() as u64;

    if let Some(rh) = headers.get("range") {
        let range = rh.to_str().unwrap_or("");
        let range = range.strip_prefix("bytes=").unwrap_or("");
        let mut parts = range.split('-');
        let start: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
        let end: u64 = parts
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(total - 1);
        let body = data[start as usize..=end as usize].to_vec();
        return Response::builder()
            .status(206)
            .header("Content-Range", format!("bytes {start}-{end}/{total}"))
            .header("Content-Length", (end - start + 1).to_string())
            .header("Accept-Ranges", "bytes")
            .body(axum::body::Body::from(body))
            .unwrap();
    }

    Response::builder()
        .status(200)
        .header("Content-Length", total.to_string())
        .header("Accept-Ranges", "bytes")
        .body(axum::body::Body::from(data.as_ref().clone()))
        .unwrap()
}

fn write_framed(stdin: &mut impl Write, value: serde_json::Value) {
    let body = serde_json::to_vec(&value).unwrap();
    stdin.write_all(&(body.len() as u32).to_le_bytes()).unwrap();
    stdin.write_all(&body).unwrap();
    stdin.flush().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_native_bridge_queues_and_downloads_10mb() {
    let data = payload_10mb();
    let expected_hash = sha256(&data);
    let addr = spawn_server(
        Router::new()
            .route("/ten-meg.bin", get(range_handler))
            .with_state(data.clone()),
    )
    .await;

    let root = std::env::temp_dir().join(format!(
        "khukri_bridge_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let downloads = root.join("Downloads");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&downloads).unwrap();
    std::fs::create_dir_all(&data_dir).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_khukri-bridge"))
        .env("HOME", &root)
        .env("USERPROFILE", &root)
        .env("KHUKRI_DATA_DIR", &data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    write_framed(
        &mut stdin,
        json!({
            "type": "queue_download",
            "url": format!("http://{addr}/ten-meg.bin"),
            "filename": "ten-meg.bin",
            "source": "test",
            "pageUrl": format!("http://{addr}/page"),
            "customHeaders": {
                "User-Agent": "khukri-bridge-test/1.0"
            }
        }),
    );

    let (event_tx, event_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut stdout = stdout;
        loop {
            let mut len_buf = [0u8; 4];
            match stdout.read_exact(&mut len_buf) {
                Ok(()) => {
                    let len = u32::from_le_bytes(len_buf) as usize;
                    let mut body = vec![0u8; len];
                    if stdout.read_exact(&mut body).is_err() {
                        let _ = event_tx.send(BridgeEvent {
                            kind: "reader_error".to_string(),
                            status: "failed".to_string(),
                            bytes_done: 0,
                            total_bytes: None,
                            output_path: None,
                            message: Some("stdout closed mid-frame".to_string()),
                        });
                        break;
                    }
                    let event: BridgeEvent = serde_json::from_slice(&body).unwrap();
                    if event_tx.send(event).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = event_tx.send(BridgeEvent {
                        kind: "reader_eof".to_string(),
                        status: "failed".to_string(),
                        bytes_done: 0,
                        total_bytes: None,
                        output_path: None,
                        message: Some("stdout closed before next frame".to_string()),
                    });
                    break;
                }
            }
        }
    });

    let (stderr_tx, stderr_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut stderr = stderr;
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        let _ = stderr_tx.send(buf);
    });

    let final_event = loop {
        let event = event_rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .unwrap_or_else(|_| {
                let status = child.try_wait().ok().flatten();
                let _ = child.kill();
                let _ = child.wait();
                let stderr = stderr_rx
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap_or_else(|_| "<no stderr captured>".to_string());
                panic!(
                    "timed out waiting for bridge completion\nchild status before kill: {:?}\nbridge stderr:\n{}",
                    status,
                    stderr
                );
            });
        if event.kind == "reader_eof" || event.kind == "reader_error" {
            let status = child.try_wait().ok().flatten();
            let stderr = stderr_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap_or_else(|_| "<no stderr captured>".to_string());
            panic!(
                "bridge stdout closed early: kind={} message={:?}\nchild status: {:?}\nbridge stderr:\n{}",
                event.kind,
                event.message,
                status,
                stderr
            );
        }
        if event.kind == "error" {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = stderr_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap_or_else(|_| "<no stderr captured>".to_string());
            panic!(
                "bridge emitted error event: status={} output_path={:?} message={:?}\nbridge stderr:\n{}",
                event.status,
                event.output_path,
                event.message,
                stderr
            );
        }
        if event.kind == "progress" && (event.status == "failed" || event.status == "paused") {
            let status = child.try_wait().ok().flatten();
            let stderr = stderr_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap_or_else(|_| "<no stderr captured>".to_string());
            panic!(
                "bridge reported terminal progress status={} bytes_done={} total_bytes={:?} output_path={:?} message={:?}\nchild status: {:?}\nbridge stderr:\n{}",
                event.status,
                event.bytes_done,
                event.total_bytes,
                event.output_path,
                event.message,
                status,
                stderr
            );
        }
        if event.kind == "progress" && event.status == "complete" {
            break event;
        }
    };

    let output_path = final_event.output_path.expect("bridge did not report output path");
    let output = std::fs::read(&output_path).expect("missing downloaded file");
    assert_eq!(output.len(), data.len());
    assert_eq!(sha256(&output), expected_hash);
    assert_eq!(final_event.bytes_done as usize, data.len());
    assert_eq!(final_event.total_bytes, Some(data.len() as u64));

    let _ = child.kill();
    let _ = child.wait();
}

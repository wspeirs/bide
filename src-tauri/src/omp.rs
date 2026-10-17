use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use base64::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::{broadcast, mpsc, oneshot},
    task::JoinHandle,
};

pub const MAX_REASSEMBLED_FRAME_BYTES: usize = 67_108_864; // 64 MiB
pub const STDERR_RING_BUFFER_CAPACITY: usize = 200;

pub type PendingRequests = Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>;
pub type TauriChannels = Arc<Mutex<Vec<tauri::ipc::Channel<Value>>>>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcChunk {
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub chunk_id: String,
    pub index: usize,
    pub count: usize,
    pub byte_length: usize,
    pub data: String,
}

#[derive(Debug, Default)]
struct PendingAssembly {
    count: usize,
    expected_total_bytes: usize,
    chunks: HashMap<usize, Vec<u8>>,
    current_bytes: usize,
}

#[derive(Debug, Default)]
pub struct ChunkAssembler {
    pending: HashMap<String, PendingAssembly>,
}

impl ChunkAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_chunk(&mut self, chunk: RpcChunk) -> Result<Option<Value>, String> {
        if chunk.byte_length > MAX_REASSEMBLED_FRAME_BYTES {
            self.pending.remove(&chunk.chunk_id);
            return Err(format!(
                "Reassembled frame byte length {} exceeds 64 MiB limit",
                chunk.byte_length
            ));
        }

        if chunk.count == 0 {
            return Err("Chunk count must be greater than zero".to_string());
        }

        if chunk.index >= chunk.count {
            return Err(format!(
                "Chunk index {} out of range (count {})",
                chunk.index, chunk.count
            ));
        }

        let decoded = BASE64_STANDARD
            .decode(chunk.data.as_bytes())
            .map_err(|e| format!("Base64 decode failed for chunk: {e}"))?;

        let entry = self
            .pending
            .entry(chunk.chunk_id.clone())
            .or_insert_with(|| PendingAssembly {
                count: chunk.count,
                expected_total_bytes: chunk.byte_length,
                chunks: HashMap::new(),
                current_bytes: 0,
            });

        if entry.count != chunk.count || entry.expected_total_bytes != chunk.byte_length {
            self.pending.remove(&chunk.chunk_id);
            return Err("Chunk metadata mismatch across chunks".to_string());
        }

        if !entry.chunks.contains_key(&chunk.index) {
            if entry.current_bytes + decoded.len() > MAX_REASSEMBLED_FRAME_BYTES {
                self.pending.remove(&chunk.chunk_id);
                return Err("Accumulated chunk bytes exceed 64 MiB limit".to_string());
            }
            entry.current_bytes += decoded.len();
            entry.chunks.insert(chunk.index, decoded);
        }

        if entry.chunks.len() == entry.count {
            let assembly = self.pending.remove(&chunk.chunk_id).unwrap();
            let mut full_bytes = Vec::with_capacity(assembly.current_bytes);
            for idx in 0..assembly.count {
                match assembly.chunks.get(&idx) {
                    Some(bytes) => full_bytes.extend_from_slice(bytes),
                    None => return Err(format!("Missing chunk at index {idx}")),
                }
            }

            let value: Value = serde_json::from_slice(&full_bytes)
                .map_err(|e| format!("Failed to parse reassembled JSON: {e}"))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }
}

pub fn process_line(
    line: &str,
    assembler: &mut ChunkAssembler,
) -> Result<Option<Value>, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let value: Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("Failed to parse JSON line: {e}"))?;

    if value.get("type").and_then(|v| v.as_str()) == Some("rpc_chunk") {
        let chunk: RpcChunk = serde_json::from_value(value)
            .map_err(|e| format!("Invalid rpc_chunk schema: {e}"))?;
        assembler.handle_chunk(chunk)
    } else {
        Ok(Some(value))
    }
}

pub fn route_frame(
    frame: Value,
    pending: &PendingRequests,
    tauri_channels: &TauriChannels,
    broadcast_tx: &broadcast::Sender<Value>,
    ready_tx: &Arc<Mutex<Option<oneshot::Sender<Value>>>>,
    is_settled: &Arc<AtomicBool>,
    is_streaming: &Arc<AtomicBool>,
) {
    if frame.get("type").and_then(|v| v.as_str()) == Some("ready") {
        if let Some(tx) = ready_tx.lock().unwrap().take() {
            let _ = tx.send(frame.clone());
        }
    }

    if frame.get("type").and_then(|v| v.as_str()) == Some("response") {
        let id_opt = frame.get("id").map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        });

        if let Some(id) = id_opt {
            let sender = pending.lock().unwrap().remove(&id);
            if let Some(tx) = sender {
                let success = frame
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let res = if success {
                    let data = frame.get("data").cloned().unwrap_or_else(|| frame.clone());
                    Ok(data)
                } else {
                    let err_msg = frame
                        .get("error")
                        .and_then(|e| e.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            frame
                                .get("error")
                                .map(|e| e.to_string())
                                .unwrap_or_else(|| "Unknown RPC error".to_string())
                        });
                    Err(err_msg)
                };
                let _ = tx.send(res);
            }
        }
        return;
    }

    if let Some(t) = frame.get("type").and_then(|v| v.as_str()) {
        match t {
            "session_settled" => {
                is_settled.store(true, Ordering::SeqCst);
                is_streaming.store(false, Ordering::SeqCst);
            }
            "agent_start" => {
                is_streaming.store(true, Ordering::SeqCst);
                is_settled.store(false, Ordering::SeqCst);
            }
            "agent_end" => {
                is_streaming.store(false, Ordering::SeqCst);
            }
            _ => {}
        }
    }

    let _ = broadcast_tx.send(frame.clone());
    let mut chs = tauri_channels.lock().unwrap();
    chs.retain(|ch| ch.send(frame.clone()).is_ok());
}

pub struct OmpSession {
    child: Arc<tokio::sync::Mutex<Option<Child>>>,
    stdin_tx: Arc<tokio::sync::Mutex<Option<mpsc::Sender<String>>>>,
    stdout_task: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>,
    stderr_task: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>,
    pending_requests: PendingRequests,
    tauri_channels: TauriChannels,
    broadcast_tx: broadcast::Sender<Value>,
    stderr_buf: Arc<Mutex<VecDeque<String>>>,
    session_file: Arc<Mutex<Option<String>>>,
    is_settled: Arc<AtomicBool>,
    is_streaming: Arc<AtomicBool>,
    model: Arc<Mutex<Option<Value>>>,
}

impl OmpSession {
    pub async fn spawn(cwd: &Path, resume_file: Option<&Path>) -> Result<Arc<Self>, String> {
        let mut cmd = Command::new("omp");
        cmd.arg("--mode").arg("rpc-ui");
        if let Some(resume) = resume_file {
            cmd.arg("--resume").arg(resume);
        }
        cmd.current_dir(cwd);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn omp: {e}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture stdin for omp child".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture stdout for omp child".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Failed to capture stderr for omp child".to_string())?;

        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(128);
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(line) = stdin_rx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.flush().await.is_err() {
                    break;
                }
            }
        });

        let stderr_buf = Arc::new(Mutex::new(VecDeque::with_capacity(
            STDERR_RING_BUFFER_CAPACITY,
        )));
        let stderr_buf_clone = Arc::clone(&stderr_buf);
        let stderr_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut buf = stderr_buf_clone.lock().unwrap();
                if buf.len() >= STDERR_RING_BUFFER_CAPACITY {
                    buf.pop_front();
                }
                buf.push_back(line);
            }
        });

        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let tauri_channels = Arc::new(Mutex::new(Vec::new()));
        let (broadcast_tx, _) = broadcast::channel(256);

        let (ready_tx, ready_rx) = oneshot::channel::<Value>();
        let ready_tx_cell = Arc::new(Mutex::new(Some(ready_tx)));

        let is_settled = Arc::new(AtomicBool::new(false));
        let is_streaming = Arc::new(AtomicBool::new(false));
        let session_file = Arc::new(Mutex::new(None));
        let model = Arc::new(Mutex::new(None));

        let pending_clone = Arc::clone(&pending_requests);
        let tauri_clone = Arc::clone(&tauri_channels);
        let broadcast_clone = broadcast_tx.clone();
        let ready_tx_clone = Arc::clone(&ready_tx_cell);
        let settled_clone = Arc::clone(&is_settled);
        let streaming_clone = Arc::clone(&is_streaming);

        let stdout_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            let mut assembler = ChunkAssembler::new();
            while let Ok(Some(line)) = reader.next_line().await {
                match process_line(&line, &mut assembler) {
                    Ok(Some(frame)) => {
                        route_frame(
                            frame,
                            &pending_clone,
                            &tauri_clone,
                            &broadcast_clone,
                            &ready_tx_clone,
                            &settled_clone,
                            &streaming_clone,
                        );
                    }
                    Ok(None) => {}
                    Err(e) => {
                        eprintln!("Error processing frame: {e}");
                    }
                }
            }

            let mut pending = pending_clone.lock().unwrap();
            for (_, tx) in pending.drain() {
                let _ = tx.send(Err("Session closed".to_string()));
            }
        });

        let session = Arc::new(Self {
            child: Arc::new(tokio::sync::Mutex::new(Some(child))),
            stdin_tx: Arc::new(tokio::sync::Mutex::new(Some(stdin_tx))),
            stdout_task: Arc::new(tokio::sync::Mutex::new(Some(stdout_task))),
            stderr_task: Arc::new(tokio::sync::Mutex::new(Some(stderr_task))),
            pending_requests,
            tauri_channels,
            broadcast_tx,
            stderr_buf,
            session_file,
            is_settled,
            is_streaming,
            model,
        });

        // 1) Waits for {"type": "ready", ...} frame.
        let _ready_frame = tokio::time::timeout(Duration::from_secs(15), ready_rx)
            .await
            .map_err(|_| "Timeout waiting for ready frame from omp".to_string())?
            .map_err(|_| "Omp child exited before sending ready frame".to_string())?;

        // 2) Sends negotiate_protocol { protocolVersion: 2 }
        let mut neg = serde_json::Map::new();
        neg.insert(
            "type".to_string(),
            Value::String("negotiate_protocol".to_string()),
        );
        neg.insert("protocolVersion".to_string(), Value::Number(2.into()));
        session
            .request(neg)
            .await
            .map_err(|e| format!("Handshake negotiate_protocol failed: {e}"))?;

        // 3) Sends set_event_filter { events: null, messageUpdates: "delta" }
        let mut filter = serde_json::Map::new();
        filter.insert(
            "type".to_string(),
            Value::String("set_event_filter".to_string()),
        );
        filter.insert("events".to_string(), Value::Null);
        filter.insert(
            "messageUpdates".to_string(),
            Value::String("delta".to_string()),
        );
        session
            .request(filter)
            .await
            .map_err(|e| format!("Handshake set_event_filter failed: {e}"))?;

        // 4) Sends set_ask_dialog { enabled: true }
        let mut ask = serde_json::Map::new();
        ask.insert(
            "type".to_string(),
            Value::String("set_ask_dialog".to_string()),
        );
        ask.insert("enabled".to_string(), Value::Bool(true));
        session
            .request(ask)
            .await
            .map_err(|e| format!("Handshake set_ask_dialog failed: {e}"))?;

        // 5) Sends get_state (extract and cache sessionFile, isSettled, isStreaming, model)
        session
            .get_state()
            .await
            .map_err(|e| format!("Handshake get_state failed: {e}"))?;

        // 6) Sends get_available_commands
        let mut cmds = serde_json::Map::new();
        cmds.insert(
            "type".to_string(),
            Value::String("get_available_commands".to_string()),
        );
        session
            .request(cmds)
            .await
            .map_err(|e| format!("Handshake get_available_commands failed: {e}"))?;

        Ok(session)
    }

    fn update_state_cache(&self, state: &Value) {
        let data = state.get("data").unwrap_or(state);
        if let Some(sf) = data.get("sessionFile").and_then(|v| v.as_str()) {
            *self.session_file.lock().unwrap() = Some(sf.to_string());
        }
        if let Some(settled) = data.get("isSettled").and_then(|v| v.as_bool()) {
            self.is_settled.store(settled, Ordering::SeqCst);
        }
        if let Some(streaming) = data.get("isStreaming").and_then(|v| v.as_bool()) {
            self.is_streaming.store(streaming, Ordering::SeqCst);
        }
        if let Some(model) = data.get("model") {
            *self.model.lock().unwrap() = Some(model.clone());
        }
    }

    pub async fn request(
        &self,
        mut command: serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let id = match command.get("id") {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => {
                let new_id = uuid::Uuid::new_v4().to_string();
                command.insert("id".to_string(), Value::String(new_id.clone()));
                new_id
            }
        };

        let (tx, rx) = oneshot::channel();
        self.pending_requests.lock().unwrap().insert(id.clone(), tx);

        let line = match serde_json::to_string(&command) {
            Ok(json_str) => json_str + "\n",
            Err(e) => {
                self.pending_requests.lock().unwrap().remove(&id);
                return Err(format!("Serialization error: {e}"));
            }
        };

        let send_res = {
            let tx_guard = self.stdin_tx.lock().await;
            if let Some(tx) = tx_guard.as_ref() {
                tx.send(line).await
            } else {
                self.pending_requests.lock().unwrap().remove(&id);
                return Err("Stdin is closed".to_string());
            }
        };

        if send_res.is_err() {
            self.pending_requests.lock().unwrap().remove(&id);
            return Err("Failed to write command to stdin".to_string());
        }

        match rx.await {
            Ok(res) => res,
            Err(_) => Err("Request dropped: session closed or no response received".to_string()),
        }
    }

    pub async fn prompt(
        &self,
        text: &str,
        streaming_behavior: Option<&str>,
    ) -> Result<Value, String> {
        let mut map = serde_json::Map::new();
        map.insert("type".to_string(), Value::String("prompt".to_string()));
        map.insert("message".to_string(), Value::String(text.to_string()));
        if let Some(sb) = streaming_behavior {
            map.insert(
                "streamingBehavior".to_string(),
                Value::String(sb.to_string()),
            );
        }
        self.request(map).await
    }

    pub async fn abort(&self) -> Result<Value, String> {
        let mut map = serde_json::Map::new();
        map.insert("type".to_string(), Value::String("abort".to_string()));
        self.request(map).await
    }

    pub async fn get_state(&self) -> Result<Value, String> {
        let mut map = serde_json::Map::new();
        map.insert("type".to_string(), Value::String("get_state".to_string()));
        let res = self.request(map).await?;
        self.update_state_cache(&res);
        Ok(res)
    }

    pub async fn new_session(&self) -> Result<Value, String> {
        let mut map = serde_json::Map::new();
        map.insert(
            "type".to_string(),
            Value::String("new_session".to_string()),
        );
        let res = self.request(map).await?;
        let _ = self.get_state().await;
        Ok(res)
    }

    pub async fn command(&self, slash: &str) -> Result<Value, String> {
        self.prompt(slash, None).await
    }

    pub async fn send_ui_response(&self, response: Value) -> Result<(), String> {
        let line = serde_json::to_string(&response)
            .map_err(|e| format!("Serialization error: {e}"))?
            + "\n";
        let tx_guard = self.stdin_tx.lock().await;
        if let Some(tx) = tx_guard.as_ref() {
            tx.send(line)
                .await
                .map_err(|_| "Failed to write to stdin: channel closed".to_string())?;
            Ok(())
        } else {
            Err("Session stdin closed".to_string())
        }
    }

    pub fn subscribe(&self, channel: tauri::ipc::Channel<Value>) {
        self.tauri_channels.lock().unwrap().push(channel);
    }

    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<Value> {
        self.broadcast_tx.subscribe()
    }

    pub fn session_file(&self) -> Option<String> {
        self.session_file.lock().unwrap().clone()
    }

    pub fn stderr_tail(&self) -> Vec<String> {
        self.stderr_buf.lock().unwrap().iter().cloned().collect()
    }

    pub fn is_settled(&self) -> bool {
        self.is_settled.load(Ordering::SeqCst)
    }

    pub fn is_streaming(&self) -> bool {
        self.is_streaming.load(Ordering::SeqCst)
    }

    pub fn model(&self) -> Option<Value> {
        self.model.lock().unwrap().clone()
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        // 1. Close stdin: drop writer channel sender
        {
            let mut tx_guard = self.stdin_tx.lock().await;
            tx_guard.take();
        }

        // 2. Drain stdout with 5s timeout
        let stdout_handle = {
            let mut guard = self.stdout_task.lock().await;
            guard.take()
        };
        if let Some(handle) = stdout_handle {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        // 3. Kill child process
        let mut child_guard = self.child.lock().await;
        if let Some(mut child) = child_guard.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }

        let stderr_handle = {
            let mut guard = self.stderr_task.lock().await;
            guard.take()
        };
        if let Some(handle) = stderr_handle {
            handle.abort();
        }

        Ok(())
    }

    pub fn new_mock() -> (
        Arc<Self>,
        mpsc::Receiver<String>,
        mpsc::Sender<String>,
    ) {
        let (stdin_tx, stdin_rx) = mpsc::channel(128);
        let (mock_stdout_tx, mut mock_stdout_rx) = mpsc::channel::<String>(128);

        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let tauri_channels = Arc::new(Mutex::new(Vec::new()));
        let (broadcast_tx, _) = broadcast::channel(256);
        let stderr_buf = Arc::new(Mutex::new(VecDeque::new()));
        let session_file = Arc::new(Mutex::new(None));
        let is_settled = Arc::new(AtomicBool::new(true));
        let is_streaming = Arc::new(AtomicBool::new(false));
        let model = Arc::new(Mutex::new(None));

        let pending_clone = Arc::clone(&pending_requests);
        let tauri_clone = Arc::clone(&tauri_channels);
        let broadcast_clone = broadcast_tx.clone();
        let ready_tx_cell = Arc::new(Mutex::new(None));
        let settled_clone = Arc::clone(&is_settled);
        let streaming_clone = Arc::clone(&is_streaming);

        let stdout_task = tokio::spawn(async move {
            let mut assembler = ChunkAssembler::new();
            while let Some(line) = mock_stdout_rx.recv().await {
                if let Ok(Some(frame)) = process_line(&line, &mut assembler) {
                    route_frame(
                        frame,
                        &pending_clone,
                        &tauri_clone,
                        &broadcast_clone,
                        &ready_tx_cell,
                        &settled_clone,
                        &streaming_clone,
                    );
                }
            }
            let mut pending = pending_clone.lock().unwrap();
            for (_, tx) in pending.drain() {
                let _ = tx.send(Err("Session closed".to_string()));
            }
        });

        let session = Arc::new(Self {
            child: Arc::new(tokio::sync::Mutex::new(None)),
            stdin_tx: Arc::new(tokio::sync::Mutex::new(Some(stdin_tx))),
            stdout_task: Arc::new(tokio::sync::Mutex::new(Some(stdout_task))),
            stderr_task: Arc::new(tokio::sync::Mutex::new(None)),
            pending_requests,
            tauri_channels,
            broadcast_tx,
            stderr_buf,
            session_file,
            is_settled,
            is_streaming,
            model,
        });

        (session, stdin_rx, mock_stdout_tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_chunk_assembler_in_order() {
        let mut assembler = ChunkAssembler::new();
        let payload = r#"{"name":"test_chunk","data":"super_long_payload_value"}"#;
        let payload_bytes = payload.as_bytes();

        let part1 = &payload_bytes[..15];
        let part2 = &payload_bytes[15..30];
        let part3 = &payload_bytes[30..];

        let c1 = RpcChunk {
            chunk_type: "rpc_chunk".to_string(),
            chunk_id: "chk-1".to_string(),
            index: 0,
            count: 3,
            byte_length: payload_bytes.len(),
            data: BASE64_STANDARD.encode(part1),
        };
        let c2 = RpcChunk {
            chunk_type: "rpc_chunk".to_string(),
            chunk_id: "chk-1".to_string(),
            index: 1,
            count: 3,
            byte_length: payload_bytes.len(),
            data: BASE64_STANDARD.encode(part2),
        };
        let c3 = RpcChunk {
            chunk_type: "rpc_chunk".to_string(),
            chunk_id: "chk-1".to_string(),
            index: 2,
            count: 3,
            byte_length: payload_bytes.len(),
            data: BASE64_STANDARD.encode(part3),
        };

        assert!(assembler.handle_chunk(c1).unwrap().is_none());
        assert!(assembler.handle_chunk(c2).unwrap().is_none());
        let assembled = assembler.handle_chunk(c3).unwrap().expect("assembled");

        assert_eq!(assembled["name"], "test_chunk");
        assert_eq!(assembled["data"], "super_long_payload_value");
    }

    #[test]
    fn test_chunk_assembler_out_of_order() {
        let mut assembler = ChunkAssembler::new();
        let payload = r#"{"id":"ooo","val":42}"#;
        let payload_bytes = payload.as_bytes();

        let mid = payload_bytes.len() / 2;
        let part1 = &payload_bytes[..mid];
        let part2 = &payload_bytes[mid..];

        let c1 = RpcChunk {
            chunk_type: "rpc_chunk".to_string(),
            chunk_id: "chk-ooo".to_string(),
            index: 0,
            count: 2,
            byte_length: payload_bytes.len(),
            data: BASE64_STANDARD.encode(part1),
        };
        let c2 = RpcChunk {
            chunk_type: "rpc_chunk".to_string(),
            chunk_id: "chk-ooo".to_string(),
            index: 1,
            count: 2,
            byte_length: payload_bytes.len(),
            data: BASE64_STANDARD.encode(part2),
        };

        // Deliver chunk 1 before chunk 0
        assert!(assembler.handle_chunk(c2).unwrap().is_none());
        let assembled = assembler.handle_chunk(c1).unwrap().expect("assembled");

        assert_eq!(assembled["id"], "ooo");
        assert_eq!(assembled["val"], 42);
    }

    #[test]
    fn test_chunk_assembler_64mb_cap() {
        let mut assembler = ChunkAssembler::new();
        let c_overflow = RpcChunk {
            chunk_type: "rpc_chunk".to_string(),
            chunk_id: "chk-huge".to_string(),
            index: 0,
            count: 1,
            byte_length: MAX_REASSEMBLED_FRAME_BYTES + 1,
            data: BASE64_STANDARD.encode(b"{}"),
        };

        let err = assembler.handle_chunk(c_overflow).unwrap_err();
        assert!(err.contains("exceeds 64 MiB limit"));
    }

    #[test]
    fn test_parse_fixtures() {
        let fixture_files = [
            "tests/fixtures/01_ready.json",
            "tests/fixtures/02_responses.jsonl",
            "tests/fixtures/03_message_stream.jsonl",
            "tests/fixtures/04_tool_execution.jsonl",
            "tests/fixtures/05_prompt_result_settled.jsonl",
            "tests/fixtures/06_command_output_advisor.jsonl",
            "tests/fixtures/07_extension_ui_request_review_select.jsonl",
            "tests/fixtures/08_extension_ui_noise_nonui.jsonl",
            "tests/fixtures/09_available_commands_update.jsonl",
            "tests/fixtures/10_resume_get_state_and_messages.jsonl",
            "tests/fixtures/11_review_headless_noui_prompt_result.jsonl",
        ];

        let mut assembler = ChunkAssembler::new();
        let pending = Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<Result<Value, String>>>::new()));
        let tauri_channels = Arc::new(Mutex::new(Vec::new()));
        let (broadcast_tx, mut broadcast_rx) = broadcast::channel(1024);
        let ready_tx_cell = Arc::new(Mutex::new(None));
        let is_settled = Arc::new(AtomicBool::new(false));
        let is_streaming = Arc::new(AtomicBool::new(false));

        for fixture_path in fixture_files {
            let content = fs::read_to_string(fixture_path)
                .or_else(|_| fs::read_to_string(format!("src-tauri/{fixture_path}")))
                .unwrap_or_else(|e| panic!("Could not read fixture {fixture_path}: {e}"));

            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }

                let parsed = process_line(line, &mut assembler).expect("Failed to process line");
                if let Some(frame) = parsed {
                    route_frame(
                        frame.clone(),
                        &pending,
                        &tauri_channels,
                        &broadcast_tx,
                        &ready_tx_cell,
                        &is_settled,
                        &is_streaming,
                    );

                    let frame_type = frame.get("type").and_then(|v| v.as_str());
                    if frame_type != Some("response") {
                        let received = broadcast_rx.try_recv().expect("Expected broadcast frame");
                        assert_eq!(received["type"], frame["type"]);
                    }
                }
            }
        }
    }

    #[test]
    fn test_parse_stdin_commands_fixture() {
        let fixture_path = "tests/fixtures/12_stdin_commands.jsonl";
        let content = fs::read_to_string(fixture_path)
            .or_else(|_| fs::read_to_string(format!("src-tauri/{fixture_path}")))
            .expect("read 12_stdin_commands.jsonl");

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let val: Value = serde_json::from_str(line).expect("valid stdin command json");
            assert!(val.is_object());
            let serialized = serde_json::to_string(&val).expect("serializable");
            assert!(!serialized.is_empty());
        }
    }

    #[tokio::test]
    async fn test_request_response_success() {
        let (session, mut stdin_rx, mock_stdout_tx) = OmpSession::new_mock();

        let req_handle = tokio::spawn(async move {
            let mut cmd = serde_json::Map::new();
            cmd.insert("type".to_string(), Value::String("get_state".to_string()));
            session.request(cmd).await
        });

        let stdin_line = stdin_rx.recv().await.expect("received stdin line");
        let stdin_val: Value = serde_json::from_str(&stdin_line).expect("valid json on stdin");
        let req_id = stdin_val["id"].as_str().expect("id string").to_string();

        let resp_json = serde_json::json!({
            "id": req_id,
            "type": "response",
            "command": "get_state",
            "success": true,
            "data": {
                "sessionFile": "/path/to/session.jsonl",
                "isSettled": true,
                "isStreaming": false,
                "model": {"id": "test-model"}
            }
        });

        mock_stdout_tx
            .send(serde_json::to_string(&resp_json).unwrap() + "\n")
            .await
            .unwrap();

        let res = req_handle.await.unwrap().expect("request succeeded");
        assert_eq!(res["sessionFile"], "/path/to/session.jsonl");
        assert_eq!(res["isSettled"], true);
    }

    #[tokio::test]
    async fn test_request_response_error() {
        let (session, mut stdin_rx, mock_stdout_tx) = OmpSession::new_mock();

        let req_handle = tokio::spawn(async move {
            let mut cmd = serde_json::Map::new();
            cmd.insert("type".to_string(), Value::String("prompt".to_string()));
            session.request(cmd).await
        });

        let stdin_line = stdin_rx.recv().await.expect("received stdin line");
        let stdin_val: Value = serde_json::from_str(&stdin_line).expect("valid json on stdin");
        let req_id = stdin_val["id"].as_str().expect("id string").to_string();

        let resp_json = serde_json::json!({
            "id": req_id,
            "type": "response",
            "command": "prompt",
            "success": false,
            "error": "Model not available"
        });

        mock_stdout_tx
            .send(serde_json::to_string(&resp_json).unwrap() + "\n")
            .await
            .unwrap();

        let err = req_handle.await.unwrap().unwrap_err();
        assert_eq!(err, "Model not available");
    }

    #[tokio::test]
    async fn test_convenience_methods() {
        let (session, mut stdin_rx, mock_stdout_tx) = OmpSession::new_mock();

        // Test send_ui_response
        let ui_resp = serde_json::json!({
            "type": "extension_ui_response",
            "id": "ui-1",
            "value": "selected_option"
        });
        session.send_ui_response(ui_resp.clone()).await.unwrap();

        let line = stdin_rx.recv().await.expect("stdin line for ui response");
        let val: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(val["type"], "extension_ui_response");
        assert_eq!(val["value"], "selected_option");

        // Test prompt
        let prompt_handle = tokio::spawn({
            let session = Arc::clone(&session);
            async move {
                session.prompt("hello assistant", Some("steer")).await
            }
        });

        let line = stdin_rx.recv().await.expect("stdin line for prompt");
        let val: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(val["type"], "prompt");
        assert_eq!(val["message"], "hello assistant");
        assert_eq!(val["streamingBehavior"], "steer");

        let resp = serde_json::json!({
            "id": val["id"],
            "type": "response",
            "command": "prompt",
            "success": true
        });
        mock_stdout_tx.send(serde_json::to_string(&resp).unwrap() + "\n").await.unwrap();
        let prompt_res = prompt_handle.await.unwrap().expect("prompt success");
        assert_eq!(prompt_res["success"], true);

        // Test abort
        let abort_handle = tokio::spawn({
            let session = Arc::clone(&session);
            async move {
                session.abort().await
            }
        });
        let line = stdin_rx.recv().await.expect("stdin line for abort");
        let val: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(val["type"], "abort");
        let resp = serde_json::json!({
            "id": val["id"],
            "type": "response",
            "command": "abort",
            "success": true
        });
        mock_stdout_tx.send(serde_json::to_string(&resp).unwrap() + "\n").await.unwrap();
        abort_handle.await.unwrap().expect("abort success");
    }
}

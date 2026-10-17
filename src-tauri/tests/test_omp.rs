use std::{
    collections::HashMap,
    fs,
    sync::{
        atomic::AtomicBool,
        Arc, Mutex,
    },
};

use base64::prelude::*;
use bide_lib::omp::{
    process_line, route_frame, ChunkAssembler, OmpSession, RpcChunk,
    MAX_REASSEMBLED_FRAME_BYTES,
};
use serde_json::Value;
use tokio::sync::{broadcast, oneshot};

#[test]
fn test_fixture_parsing_and_routing() {
    let fixture_paths = [
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

    for fixture in fixture_paths {
        let content = fs::read_to_string(fixture)
            .or_else(|_| fs::read_to_string(format!("src-tauri/{fixture}")))
            .unwrap_or_else(|e| panic!("Failed to read {fixture}: {e}"));

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let opt_frame = process_line(line, &mut assembler).expect("Valid JSON line");
            if let Some(frame) = opt_frame {
                route_frame(
                    frame.clone(),
                    &pending,
                    &tauri_channels,
                    &broadcast_tx,
                    &ready_tx_cell,
                    &is_settled,
                    &is_streaming,
                );

                if frame.get("type").and_then(|v| v.as_str()) != Some("response") {
                    let rx_frame = broadcast_rx.try_recv().expect("Frame sent to broadcast");
                    assert_eq!(rx_frame["type"], frame["type"]);
                }
            }
        }
    }
}

#[test]
fn test_rpc_chunk_reassembly_in_and_out_of_order() {
    let mut assembler = ChunkAssembler::new();
    let raw_json = r#"{"id":"chunk-test","items":["alpha","beta","gamma"],"value":12345}"#;
    let bytes = raw_json.as_bytes();

    let split1 = bytes.len() / 3;
    let split2 = (bytes.len() * 2) / 3;

    let chunk0 = RpcChunk {
        chunk_type: "rpc_chunk".to_string(),
        chunk_id: "c-1".to_string(),
        index: 0,
        count: 3,
        byte_length: bytes.len(),
        data: BASE64_STANDARD.encode(&bytes[..split1]),
    };
    let chunk1 = RpcChunk {
        chunk_type: "rpc_chunk".to_string(),
        chunk_id: "c-1".to_string(),
        index: 1,
        count: 3,
        byte_length: bytes.len(),
        data: BASE64_STANDARD.encode(&bytes[split1..split2]),
    };
    let chunk2 = RpcChunk {
        chunk_type: "rpc_chunk".to_string(),
        chunk_id: "c-1".to_string(),
        index: 2,
        count: 3,
        byte_length: bytes.len(),
        data: BASE64_STANDARD.encode(&bytes[split2..]),
    };

    // Out-of-order: chunk 1, chunk 2, chunk 0
    assert!(assembler.handle_chunk(chunk1).unwrap().is_none());
    assert!(assembler.handle_chunk(chunk2).unwrap().is_none());
    let reassembled = assembler.handle_chunk(chunk0).unwrap().expect("reassembled json");

    assert_eq!(reassembled["id"], "chunk-test");
    assert_eq!(reassembled["value"], 12345);
    assert_eq!(reassembled["items"][1], "beta");
}

#[test]
fn test_rpc_chunk_cap() {
    let mut assembler = ChunkAssembler::new();
    let chunk = RpcChunk {
        chunk_type: "rpc_chunk".to_string(),
        chunk_id: "oversize".to_string(),
        index: 0,
        count: 1,
        byte_length: MAX_REASSEMBLED_FRAME_BYTES + 10,
        data: BASE64_STANDARD.encode(b"{}"),
    };

    let result = assembler.handle_chunk(chunk);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("64 MiB limit"));
}

#[tokio::test]
async fn test_request_response_correlation() {
    let (session, mut stdin_rx, mock_stdout_tx) = OmpSession::new_mock();

    let task = tokio::spawn(async move {
        let mut map = serde_json::Map::new();
        map.insert("type".to_string(), Value::String("get_state".to_string()));
        session.request(map).await
    });

    let stdin_line = stdin_rx.recv().await.expect("stdin command");
    let stdin_val: Value = serde_json::from_str(&stdin_line).unwrap();
    let req_id = stdin_val["id"].as_str().unwrap().to_string();

    let response = serde_json::json!({
        "id": req_id,
        "type": "response",
        "command": "get_state",
        "success": true,
        "data": {
            "sessionFile": "/path/session.jsonl",
            "isSettled": true,
            "isStreaming": false
        }
    });

    mock_stdout_tx
        .send(serde_json::to_string(&response).unwrap() + "\n")
        .await
        .unwrap();

    let val = task.await.unwrap().expect("request resolved");
    assert_eq!(val["sessionFile"], "/path/session.jsonl");
    assert_eq!(val["isSettled"], true);
}

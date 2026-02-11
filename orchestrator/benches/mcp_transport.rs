use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ironclaw_orchestrator::mcp::transport::Transport;
use ironclaw_orchestrator::mcp::{McpRequest, StdioTransport};
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use tempfile::Builder;
use tokio::runtime::Runtime;

fn bench_transport_roundtrip(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    #[cfg(unix)]
    {
        // Create a mock server script
        let mut script_file = Builder::new()
            .prefix("mcp_mock_server")
            .suffix(".sh")
            .tempfile()
            .unwrap();

        let script_path = script_file.path().to_owned();
        let script_content = r#"#!/bin/bash
while read line; do
  # Extract ID from line if possible, or just default to 1
  # Simple echo of a success response
  echo '{"jsonrpc":"2.0","id":1,"result":{"status":"ok"}}'
done
"#;

        script_file.write_all(script_content.as_bytes()).unwrap();

        // Make executable
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();

        // Close the file handle to avoid ETXTBSY
        let temp_path = script_file.into_temp_path();
        let script_path_str = temp_path.to_str().unwrap().to_string();

        c.bench_function("mcp_transport_roundtrip", |b| {
            let _guard = rt.enter();

            // Spawn the transport once to benchmark the transport communication itself,
            // rather than process spawning/teardown overhead.
            // This also allows us to benefit from the buffer reuse optimization.
            let mut transport =
                rt.block_on(async { StdioTransport::spawn(&script_path_str, &[]).await.unwrap() });

            b.iter(|| {
                rt.block_on(async {
                    let request =
                        McpRequest::new(1, "test", Some(serde_json::json!({"foo": "bar"})));
                    transport.send(black_box(&request)).await.unwrap();
                    let _ = transport.recv().await.unwrap();
                })
            });

            // Clean up
            rt.block_on(async {
                transport.kill().await.unwrap();
            });
        });
    }

    #[cfg(not(unix))]
    {
        println!("Skipping benchmark on non-unix platform");
    }
}

criterion_group!(benches, bench_transport_roundtrip);
criterion_main!(benches);

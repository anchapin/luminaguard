use criterion::{criterion_group, criterion_main, Criterion};
use luminaguard_orchestrator::mcp::{McpClient, StdioTransport};
use std::env;
use std::hint::black_box;

fn bench_startup(c: &mut Criterion) {
    // Allow overriding the benchmark command to keep this portable across environments.
    // Defaults to "echo" for a cheap, benign process if no override is provided.
    let command = env::var("MCP_STARTUP_BENCH_CMD").unwrap_or_else(|_| "echo".to_string());
    let command_args_raw: Vec<String> = env::var("MCP_STARTUP_BENCH_ARGS")
        .ok()
        .map(|v| v.split_whitespace().map(|s| s.to_string()).collect())
        .unwrap_or_else(Vec::new);
    let command_args: Vec<&str> = command_args_raw.iter().map(|s| s.as_str()).collect();

    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for benchmark");

    // Benchmark: Spawn stdio transport
    c.bench_function("mcp_startup_stdio_spawn", |b| {
        let _guard = rt.enter();
        b.iter(|| {
            let transport = rt.block_on(async {
                StdioTransport::spawn(&command, &command_args)
                    .await
                    .expect("Failed to spawn stdio transport for benchmark")
            });
            black_box(transport);
        });
    });

    // Benchmark: Create client (includes spawn)
    c.bench_function("mcp_startup_client_full", |b| {
        let _guard = rt.enter();
        b.iter(|| {
            let client = rt.block_on(async {
                let transport = StdioTransport::spawn(&command, &command_args)
                    .await
                    .expect("Failed to spawn stdio transport for benchmark");
                McpClient::new(transport)
            });
            black_box(client);
        });
    });
}

criterion_group!(benches, bench_startup);
criterion_main!(benches);

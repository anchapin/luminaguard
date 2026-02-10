use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ironclaw_orchestrator::mcp::{McpClient, StdioTransport};

fn bench_startup(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Benchmark: Spawn stdio transport
    c.bench_function("mcp_startup_stdio_spawn", |b| {
        let _guard = rt.enter();
        b.iter(|| {
            let transport =
                rt.block_on(async { StdioTransport::spawn("echo", &[]).await.unwrap() });
            black_box(transport);
        });
    });

    // Benchmark: Create client (includes spawn)
    c.bench_function("mcp_startup_client_full", |b| {
        let _guard = rt.enter();
        b.iter(|| {
            let client = rt.block_on(async {
                let transport = StdioTransport::spawn("echo", &[]).await.unwrap();
                McpClient::new(transport)
            });
            black_box(client);
        });
    });
}

criterion_group!(benches, bench_startup);
criterion_main!(benches);

#[path = "../tests/features/support/shared.rs"]
mod support;

use std::{
    path::Path,
    time::{Duration, Instant},
};

use criterion::{BenchmarkId, Criterion, criterion_group};
use crossterm::event::{MouseButton, MouseEventKind};
use serde_json::json;
use shodan::settings::build_settings;
use support::{
    CapturedRequest, RecordedProvider, TestApplication, TestHome, create_home, read_bottom_line,
    read_screen, receive_request, run_test_application_with_size, send_response_bytes,
};

#[derive(Clone, Copy)]
enum RecordedConversation {
    Chat,
    Read,
    TechnicalDebt,
    TechnicalDebtReads,
}

struct Conversation {
    application: TestApplication,
    _home: TestHome,
    scroll_up: bool,
    _pending_request: Option<CapturedRequest>,
}

impl Conversation {
    async fn build(turns: usize, recording: RecordedConversation) -> Self {
        let openai = matches!(
            recording,
            RecordedConversation::TechnicalDebt | RecordedConversation::TechnicalDebtReads
        );
        let provider = RecordedProvider::new();
        let (address, mut requests) = if openai {
            provider.start_openai().await
        } else {
            provider.start().await
        };
        let provider_name = if openai { "openai" } else { "llama" };
        let home = create_home(
            json!({"default_provider": provider_name, "providers": {provider_name: {"address": address, "default_model": "model", "models": [{"name": "model"}]}}}),
        );
        if openai {
            let credentials = json!({"type":"oauth", "access":"recorded-access", "refresh":"recorded-refresh", "expires":4_102_444_800_000_u64, "clientId":"recorded-client", "scopes":["chatgpt.tokens.use.direct"]});
            std::fs::write(
                home.path().join(".shodan/openai.json"),
                credentials.to_string(),
            )
            .expect("the benchmark credentials can be written");
        }
        let settings = build_settings(home.path().to_owned(), Vec::new())
            .expect("the benchmark settings are valid");
        let mut application = run_test_application_with_size(settings, 120, 24);
        application.wait_for_next_frame().await;
        let mut pending_request = None;
        for _ in 0..turns {
            let prompt = if openai {
                "what do you think is biggest tech debt? write plan with all details"
            } else {
                "a"
            };
            application.send_message(prompt);
            let responses: &[&str] = match recording {
                RecordedConversation::Chat => &[include_str!(
                    "../tests/features/fixtures/llama/chat/lfm3-asteroids-plan.txt"
                )],
                RecordedConversation::Read => &[
                    include_str!(
                        "../tests/features/fixtures/llama/tools/user_reads_file/response-1.txt"
                    ),
                    include_str!(
                        "../tests/features/fixtures/llama/tools/user_reads_file/response-2.txt"
                    ),
                ],
                RecordedConversation::TechnicalDebt => &[
                    include_str!("fixtures/technical_debt/response-1.txt"),
                    include_str!("fixtures/technical_debt/response-2.txt"),
                    include_str!("fixtures/technical_debt/response-3.txt"),
                    include_str!("fixtures/technical_debt/response-4.txt"),
                    include_str!("fixtures/technical_debt/response-5.txt"),
                    include_str!("fixtures/technical_debt/response-6.txt"),
                ],
                RecordedConversation::TechnicalDebtReads => {
                    &[include_str!("fixtures/technical_debt/response-3.txt")]
                }
            };
            for (position, response) in responses.iter().enumerate() {
                let request = receive_request(&mut requests).await;
                if matches!(recording, RecordedConversation::TechnicalDebt) && position == 5 {
                    let results = request.body["input"]
                        .as_array()
                        .expect("the recorded conversation has input")
                        .iter()
                        .filter(|item| item["type"] == "function_call_output")
                        .count();
                    assert_eq!(results, 15);
                }
                send_response_bytes(&request, response).await;
            }
            if matches!(recording, RecordedConversation::TechnicalDebtReads) {
                let request = receive_request(&mut requests).await;
                let results = request.body["input"]
                    .as_array()
                    .expect("the recorded conversation has input")
                    .iter()
                    .filter(|item| item["type"] == "function_call_output")
                    .count();
                assert_eq!(results, 5);
                application.wait_for_next_frame().await;
                pending_request = Some(request);
            } else {
                application
                    .wait_for_screen(|screen| read_bottom_line(screen).contains('⏱'))
                    .await;
            }
        }
        Self {
            application,
            _home: home,
            scroll_up: true,
            _pending_request: pending_request,
        }
    }

    async fn scroll(&mut self) {
        let kind = if self.scroll_up {
            MouseEventKind::ScrollUp
        } else {
            MouseEventKind::ScrollDown
        };
        self.scroll_up = !self.scroll_up;
        self.application.scroll_mouse(kind, 10, 10);
        tokio::time::timeout(
            Duration::from_secs(5),
            self.application.wait_for_next_frame(),
        )
        .await
        .expect("scrolling produces a frame");
    }

    async fn drag(&mut self, column: u16) {
        self.application
            .scroll_mouse(MouseEventKind::Drag(MouseButton::Left), column, 18);
        tokio::time::timeout(
            Duration::from_secs(5),
            self.application.wait_for_next_frame(),
        )
        .await
        .expect("dragging produces a frame");
    }
}

fn benchmark_conversation_redraw(criterion: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("the benchmark runtime starts");
    let mut group = criterion.benchmark_group("conversation_scroll");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    for (name, turns, recording) in [
        ("chat", 20, RecordedConversation::Chat),
        ("read", 5, RecordedConversation::Read),
        ("technical_debt", 1, RecordedConversation::TechnicalDebt),
        (
            "technical_debt_reads",
            1,
            RecordedConversation::TechnicalDebtReads,
        ),
    ] {
        let mut conversation = None;
        group.bench_with_input(BenchmarkId::new(name, turns), &turns, |benchmark, _| {
            let conversation = conversation
                .get_or_insert_with(|| runtime.block_on(Conversation::build(turns, recording)));
            benchmark.iter_custom(|iterations| {
                let started = Instant::now();
                runtime.block_on(async {
                    for _ in 0..iterations {
                        conversation.scroll().await;
                    }
                });
                started.elapsed()
            });
        });
    }
    group.finish();
    let mut group = criterion.benchmark_group("technical_debt_selection_drag");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    let mut conversation = None;
    let mut column = 100;
    group.bench_function("drag", |benchmark| {
        let conversation = conversation.get_or_insert_with(|| {
            let mut conversation = runtime.block_on(Conversation::build(
                1,
                RecordedConversation::TechnicalDebtReads,
            ));
            conversation
                .application
                .scroll_mouse(MouseEventKind::Down(MouseButton::Left), 0, 1);
            runtime.block_on(conversation.application.wait_for_next_frame());
            conversation
        });
        benchmark.iter_custom(|iterations| {
            let started = Instant::now();
            runtime.block_on(async {
                for _ in 0..iterations {
                    conversation.drag(column).await;
                    column = if column == 100 { 90 } else { 100 };
                }
            });
            started.elapsed()
        });
    });
    group.finish();
}

fn measure_session_restore(home: &Path) {
    let runtime = tokio::runtime::Runtime::new().expect("the benchmark runtime starts");
    let started = Instant::now();
    let settings = build_settings(
        home.to_owned(),
        vec!["--session".to_owned(), "0".to_owned()],
    )
    .expect("the saved session settings are valid");
    let application = runtime.enter();
    let mut restored = run_test_application_with_size(settings, 120, 24);
    drop(application);
    let screen = runtime.block_on(restored.wait_for_next_frame());
    let elapsed = started.elapsed();
    let screen = read_screen(&screen);
    assert!(
        screen.contains("Inspecting the repo"),
        "the restored response appears in the first frame: {screen}"
    );
    println!("{}", elapsed.as_nanos());
}

fn prepare_saved_session() -> TestHome {
    let home = create_home(
        json!({"default_provider": "openai", "providers": {"openai": {"address": "http://127.0.0.1:1", "default_model": "model", "models": [{"name": "model"}]}}}),
    );
    let credentials = json!({"type":"oauth", "access":"recorded-access", "refresh":"recorded-refresh", "expires":4_102_444_800_000_u64, "clientId":"recorded-client", "scopes":["chatgpt.tokens.use.direct"]});
    std::fs::write(
        home.path().join(".shodan/openai.json"),
        credentials.to_string(),
    )
    .expect("the benchmark credentials can be written");
    let directory = home.path().join(".shodan/sessions").join(
        std::env::current_dir()
            .expect("the project directory exists")
            .strip_prefix("/")
            .expect("the project directory is absolute"),
    );
    std::fs::create_dir_all(&directory).expect("the session directory can be created");
    let contents = include_str!("fixtures/technical_debt/session.jsonl");
    std::fs::write(
        directory.join("00000000-0000-4000-8000-000000000001.jsonl"),
        contents,
    )
    .expect("the benchmark session can be written");
    let snapshot: serde_json::Value =
        serde_json::from_str(contents.lines().last().expect("the session has a snapshot"))
            .expect("the snapshot is valid");
    let read_results = snapshot["messages"]
        .as_array()
        .expect("the snapshot has messages")
        .iter()
        .filter(|message| message["ToolResult"]["display"]["Read"].is_object())
        .count();
    assert!(
        read_results >= 5,
        "the session contains representative file reads: {read_results}"
    );
    eprintln!(
        "restore workload: {} bytes, {read_results} file reads",
        contents.len()
    );
    home
}

fn benchmark_session_restore(criterion: &mut Criterion) {
    let home = std::sync::OnceLock::new();
    let mut group = criterion.benchmark_group("session_restore");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    group.bench_function("cold_process", |benchmark| {
        let home = home.get_or_init(prepare_saved_session);
        benchmark.iter_custom(|iterations| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iterations {
                let output = std::process::Command::new(
                    std::env::current_exe().expect("the benchmark executable exists"),
                )
                .env("SHODAN_RESTORE_BENCHMARK_HOME", home.path())
                .output()
                .expect("the restore worker starts");
                assert!(
                    output.status.success(),
                    "restore worker failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let nanoseconds: u64 = std::str::from_utf8(&output.stdout)
                    .expect("the worker output is text")
                    .trim()
                    .parse()
                    .expect("the worker reports elapsed nanoseconds");
                elapsed += Duration::from_nanos(nanoseconds);
            }
            elapsed
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    benchmark_conversation_redraw,
    benchmark_session_restore
);

fn main() {
    if let Some(home) = std::env::var_os("SHODAN_RESTORE_BENCHMARK_HOME") {
        measure_session_restore(Path::new(&home));
    } else {
        benches();
        Criterion::default().configure_from_args().final_summary();
    }
}

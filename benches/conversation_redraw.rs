use std::{
    convert::Infallible,
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, header},
    response::IntoResponse,
    routing::post,
};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
    backend::{Backend, ClearType, TestBackend, WindowSize},
    buffer::Cell,
    layout::{Position, Size},
};
use shodan::{application, interface::TerminalMode, settings::build_settings};
use tokio::{
    net::TcpListener,
    sync::{mpsc, watch},
};
use tokio_stream::wrappers::UnboundedReceiverStream;

struct ObservedTerminal {
    backend: TestBackend,
    frames: watch::Sender<(usize, bool)>,
}

impl Backend for ObservedTerminal {
    type Error = Infallible;

    fn draw<'a, Content>(&mut self, content: Content) -> Result<(), Self::Error>
    where
        Content: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.backend.draw(content)
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.backend.hide_cursor()
    }
    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.backend.show_cursor()
    }
    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        self.backend.get_cursor_position()
    }
    fn set_cursor_position<Coordinate: Into<Position>>(
        &mut self,
        position: Coordinate,
    ) -> Result<(), Self::Error> {
        self.backend.set_cursor_position(position)
    }
    fn clear(&mut self) -> Result<(), Self::Error> {
        self.backend.clear()
    }
    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        self.backend.clear_region(clear_type)
    }
    fn size(&self) -> Result<Size, Self::Error> {
        self.backend.size()
    }
    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.backend.window_size()
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.backend.flush()?;
        let buffer = self.backend.buffer();
        let bottom = buffer.area.height.saturating_sub(1);
        let status: String = (0..buffer.area.width)
            .map(|column| buffer[(column, bottom)].symbol())
            .collect();
        self.frames
            .send_modify(|frame| *frame = (frame.0 + 1, status.contains('⏱')));
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum RecordedConversation {
    Chat,
    Read,
    TechnicalDebt,
    TechnicalDebtReads,
}

struct ProviderState {
    requests: AtomicUsize,
    conversation: RecordedConversation,
    completed_requests: watch::Sender<usize>,
}

async fn serve_response(
    State(state): State<Arc<ProviderState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let request = state.requests.fetch_add(1, Ordering::SeqCst);
    let response = match state.conversation {
        RecordedConversation::Chat => {
            include_str!("../tests/features/fixtures/llama/chat/lfm3-asteroids-plan.txt")
        }
        RecordedConversation::Read if request.is_multiple_of(2) => {
            include_str!("../tests/features/fixtures/llama/tools/user_reads_file/response-1.txt")
        }
        RecordedConversation::Read => {
            include_str!("../tests/features/fixtures/llama/tools/user_reads_file/response-2.txt")
        }
        RecordedConversation::TechnicalDebtReads => {
            if request == 1 {
                let input = body["input"]
                    .as_array()
                    .expect("the recorded conversation has input");
                let tool_results = input
                    .iter()
                    .filter(|item| item["type"] == "function_call_output")
                    .count();
                assert_eq!(tool_results, 5, "all recorded file reads completed");
                state.completed_requests.send_replace(request + 1);
                std::future::pending::<()>().await;
            }
            include_str!("fixtures/technical_debt/response-3.txt")
        }
        RecordedConversation::TechnicalDebt => {
            if request == 5 {
                let input = body["input"]
                    .as_array()
                    .expect("the recorded conversation has input");
                let tool_results = input
                    .iter()
                    .filter(|item| item["type"] == "function_call_output")
                    .count();
                assert_eq!(tool_results, 15, "all recorded tool calls completed");
            }
            [
                include_str!("fixtures/technical_debt/response-1.txt"),
                include_str!("fixtures/technical_debt/response-2.txt"),
                include_str!("fixtures/technical_debt/response-3.txt"),
                include_str!("fixtures/technical_debt/response-4.txt"),
                include_str!("fixtures/technical_debt/response-5.txt"),
                include_str!("fixtures/technical_debt/response-6.txt"),
            ][request]
        }
    };
    state.completed_requests.send_replace(request + 1);
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/event-stream"),
        )],
        response,
    )
}

struct Conversation {
    events: mpsc::UnboundedSender<io::Result<Event>>,
    frames: watch::Receiver<(usize, bool)>,
    application: tokio::task::JoinHandle<Result<(), application::ApplicationError>>,
    server: tokio::task::JoinHandle<()>,
    home: std::path::PathBuf,
    scroll_up: bool,
}

impl Drop for Conversation {
    fn drop(&mut self) {
        self.application.abort();
        self.server.abort();
        std::fs::remove_dir_all(&self.home).expect("the benchmark home can be removed");
    }
}

fn create_configuration(
    directory: &std::path::Path,
    address: std::net::SocketAddr,
    recorded_conversation: RecordedConversation,
) {
    let configuration = match recorded_conversation {
        RecordedConversation::TechnicalDebt | RecordedConversation::TechnicalDebtReads => {
            let credentials = serde_json::json!({"type":"oauth", "access":"recorded-access", "refresh":"recorded-refresh", "expires":4_102_444_800_000_u64, "clientId":"recorded-client", "scopes":["chatgpt.tokens.use.direct"]});
            std::fs::write(directory.join("openai.json"), credentials.to_string())
                .expect("the benchmark credentials can be written");
            serde_json::json!({"default_provider":"openai", "providers":{"openai":{"address":format!("http://{address}"), "default_model":"model", "models":[{"name":"model"}]}}})
        }
        RecordedConversation::Chat | RecordedConversation::Read => {
            serde_json::json!({"default_provider":"llama", "providers":{"llama":{"address":format!("http://{address}"), "default_model":"model", "models":[{"name":"model"}]}}})
        }
    };
    std::fs::write(directory.join("settings.json"), configuration.to_string())
        .expect("the benchmark settings can be written");
}

impl Conversation {
    async fn build(turns: usize, recorded_conversation: RecordedConversation) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("the benchmark provider binds");
        let address = listener
            .local_addr()
            .expect("the benchmark provider has an address");
        let (completed_requests, mut requests) = watch::channel(0);
        let state = Arc::new(ProviderState {
            requests: AtomicUsize::new(0),
            conversation: recorded_conversation,
            completed_requests,
        });
        let router = Router::new()
            .route("/v1/chat/completions", post(serve_response))
            .route("/v1/responses", post(serve_response))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("the benchmark provider serves requests");
        });
        let home = std::env::temp_dir().join(format!("shodan-benchmark-{}", uuid::Uuid::new_v4()));
        let directory = home.join(".shodan");
        std::fs::create_dir_all(&directory).expect("the benchmark home can be created");
        create_configuration(&directory, address, recorded_conversation);
        let settings =
            build_settings(home.clone(), Vec::new()).expect("the benchmark settings are valid");
        let (frames, frame_receiver) = watch::channel((0, false));
        let backend = ObservedTerminal {
            backend: TestBackend::new(120, 24),
            frames,
        };
        let (events, event_receiver) = mpsc::unbounded_channel();
        let application = tokio::spawn(application::run(settings, || {
            (
                backend,
                UnboundedReceiverStream::new(event_receiver),
                TerminalMode::Unmanaged,
            )
        }));
        let mut conversation = Self {
            events,
            frames: frame_receiver,
            application,
            server,
            home,
            scroll_up: true,
        };
        conversation
            .frames
            .changed()
            .await
            .expect("the initial frame is drawn");
        for turn in 0..turns {
            let prompt = match recorded_conversation {
                RecordedConversation::TechnicalDebt | RecordedConversation::TechnicalDebtReads => {
                    "what do you think is biggest tech debt? write plan with all details"
                }
                RecordedConversation::Chat | RecordedConversation::Read => "a",
            };
            for character in prompt.chars() {
                conversation.send_key(KeyCode::Char(character));
            }
            conversation.send_key(KeyCode::Enter);
            let requests_per_turn = match recorded_conversation {
                RecordedConversation::Chat => 1,
                RecordedConversation::Read | RecordedConversation::TechnicalDebtReads => 2,
                RecordedConversation::TechnicalDebt => 6,
            };
            let expected_requests = (turn + 1) * requests_per_turn;
            requests
                .wait_for(|count| *count >= expected_requests)
                .await
                .expect("the recorded response is requested");
            conversation
                .wait_for_response_frame(recorded_conversation)
                .await;
        }
        conversation.frames.borrow_and_update();
        conversation
    }

    async fn wait_for_response_frame(&mut self, recorded_conversation: RecordedConversation) {
        let response_is_pending = matches!(
            recorded_conversation,
            RecordedConversation::TechnicalDebtReads
        );
        if response_is_pending {
            self.frames.borrow_and_update();
        }
        loop {
            self.frames
                .changed()
                .await
                .expect("the application draws a frame");
            if response_is_pending || self.frames.borrow_and_update().1 {
                break;
            }
        }
    }

    fn send_key(&self, code: KeyCode) {
        self.events
            .send(Ok(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))))
            .expect("the application receives input");
    }

    async fn send_mouse(&mut self, kind: MouseEventKind, column: u16, row: u16) {
        let mouse = MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        self.events
            .send(Ok(Event::Mouse(mouse)))
            .expect("the application receives mouse input");
        self.frames
            .changed()
            .await
            .expect("the application draws the mouse frame");
    }

    async fn scroll(&mut self) {
        let kind = if self.scroll_up {
            MouseEventKind::ScrollUp
        } else {
            MouseEventKind::ScrollDown
        };
        self.scroll_up = !self.scroll_up;
        self.send_mouse(kind, 10, 10).await;
    }
}

fn benchmark_conversation_redraw(criterion: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("the benchmark runtime starts");
    let mut group = criterion.benchmark_group("conversation_scroll");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    for (name, turns, recorded_conversation) in [
        ("empty", 0, RecordedConversation::Chat),
        ("chat", 1, RecordedConversation::Chat),
        ("chat", 5, RecordedConversation::Chat),
        ("chat", 20, RecordedConversation::Chat),
        ("read", 5, RecordedConversation::Read),
        ("technical_debt", 1, RecordedConversation::TechnicalDebt),
        (
            "technical_debt_reads",
            1,
            RecordedConversation::TechnicalDebtReads,
        ),
    ] {
        group.bench_with_input(BenchmarkId::new(name, turns), &turns, |benchmark, _| {
            let mut conversation =
                runtime.block_on(Conversation::build(turns, recorded_conversation));
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

    criterion.bench_function("technical_debt_selection_drag", |benchmark| {
        let mut conversation = runtime.block_on(Conversation::build(
            1,
            RecordedConversation::TechnicalDebtReads,
        ));
        runtime.block_on(conversation.send_mouse(MouseEventKind::Down(MouseButton::Left), 0, 1));
        let mut column = 100;
        benchmark.iter_custom(|iterations| {
            let started = Instant::now();
            runtime.block_on(async {
                for _ in 0..iterations {
                    conversation
                        .send_mouse(MouseEventKind::Drag(MouseButton::Left), column, 18)
                        .await;
                    column = if column == 100 { 90 } else { 100 };
                }
            });
            started.elapsed()
        });
    });
}

criterion_group!(benches, benchmark_conversation_redraw);
criterion_main!(benches);

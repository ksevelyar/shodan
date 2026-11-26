use std::{fs, path::Path};

use super::super::support::{CapturedRequest, send_response_bytes};

pub(crate) async fn send_scenario_response(
    request: &CapturedRequest,
    scenario: &str,
    response_number: usize,
) {
    let directory = Path::new("tests/features/fixtures/llama/tools").join(scenario);
    let path = directory.join(format!("response-{response_number}.txt"));
    let response =
        fs::read_to_string(&path).expect("the scenario has a recorded provider response");
    send_response_bytes(request, &response).await;
}
pub(crate) fn read_tool_history(screen: &ratatui::buffer::Buffer) -> String {
    (0..screen.area.height.saturating_sub(4))
        .filter(|&row| screen[(0, row)].bg != ratatui::style::Color::Rgb(55, 55, 55))
        .map(|row| {
            (0..screen.area.width)
                .map(|column| screen[(column, row)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

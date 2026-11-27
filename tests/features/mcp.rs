use super::{
    openai::create_openai_home,
    support::{
        MockedBlenderMcp, RecordedProvider, read_history, receive_request,
        run_test_application_with_server, send_response_bytes,
    },
};

#[tokio::test]
async fn user_inspects_blender_scene() {
    let (address, mut requests) = RecordedProvider::new().start_openai().await;
    let home = create_openai_home(&address);
    let mocked_blender = MockedBlenderMcp::new(home.path());
    let settings = shodan::settings::build_settings(home.path().to_owned(), Vec::new())
        .expect("settings can be built")
        .with_blender_server_command(Some(mocked_blender.command()));
    let mut application = run_test_application_with_server(settings, 120, 24);
    application.send_message("Describe the open Blender scene. Call blender_get_scene_info with an empty object as its arguments, then answer using its result.");

    let first_request = receive_request(&mut requests).await;
    assert!(
        first_request.body["tools"]
            .as_array()
            .expect("tools are available")
            .iter()
            .any(|tool| tool["name"] == "blender_get_scene_info")
    );
    send_response_bytes(
        &first_request,
        include_str!("fixtures/openai/mcp/user_inspects_blender_scene/response-1.txt"),
    )
    .await;

    let second_request = receive_request(&mut requests).await;
    let input = second_request.body["input"]
        .as_array()
        .expect("input is available");
    assert!(input.iter().any(|item| item["type"] == "function_call" && item["name"] == "blender_get_scene_info"));
    assert!(
        input
            .iter()
            .any(|item| item["type"] == "function_call_output"
                && item["output"].as_str().is_some_and(
                    |output| output.contains("Cube | mesh") && output.contains("Light | light")
                ))
    );
    let screen = application
        .wait_for_screen(|screen| {
            let history = read_history(screen);
            history.contains("blender_get_scene_info")
                && history.contains("Cube")
                && history.contains("Light")
        })
        .await;
    assert!(read_history(&screen).contains("Camera"));
    send_response_bytes(
        &second_request,
        include_str!("fixtures/openai/mcp/user_inspects_blender_scene/response-2.txt"),
    )
    .await;
    application
        .wait_for_screen(|screen| {
            read_history(screen).contains("The open scene contains three objects")
        })
        .await;

    let received_requests = mocked_blender.received_requests();
    assert!(
        received_requests
            .iter()
            .any(|request| request["method"] == "tools/call"
                && request["params"]["name"] == "get_scene_info")
    );
}

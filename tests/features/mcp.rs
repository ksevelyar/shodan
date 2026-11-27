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

#[cfg(unix)]
#[tokio::test]
async fn user_starts_shodan_with_diagnostic_message_from_mcp_server() {
    use std::os::unix::fs::PermissionsExt;

    let (address, mut requests) = RecordedProvider::new().start_openai().await;
    let home = create_openai_home(&address);
    let mocked_blender = MockedBlenderMcp::new(home.path());
    let command = mocked_blender.command();
    let server_path = home.path().join("blender-mcp-server");
    std::fs::write(
        &server_path,
        format!(
            "#!/bin/sh\necho 'MCP startup diagnostic' >&2\nexec '{}' '{}' '{}' '{}'\n",
            command[0], command[1], command[2], command[3]
        ),
    )
    .expect("the MCP server can be created");
    std::fs::set_permissions(&server_path, std::fs::Permissions::from_mode(0o755))
        .expect("the MCP server can be executed");
    let search_path = format!(
        "{}:{}",
        home.path().display(),
        std::env::var("PATH").expect("the executable search path is available")
    );
    let process = tokio::process::Command::new(env!("CARGO_BIN_EXE_shodan"))
        .env("HOME", home.path())
        .env("PATH", search_path)
        .args(["--prompt", "Describe the open Blender scene."])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("Shodan starts");
    let request = receive_request(&mut requests).await;
    assert!(
        request.body["tools"]
            .as_array()
            .expect("tools are available")
            .iter()
            .any(|tool| tool["name"] == "blender_get_scene_info")
    );
    send_response_bytes(
        &request,
        include_str!("fixtures/openai/mcp/user_inspects_blender_scene/response-2.txt"),
    )
    .await;
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        process.wait_with_output(),
    )
    .await
    .expect("Shodan exits promptly")
    .expect("Shodan exits");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

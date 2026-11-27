use std::{process::Stdio, sync::Arc, time::Duration};

use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ContentBlock, Tool},
    service::{RoleClient, RunningService},
    transport::TokioChildProcess,
};
use serde_json::{Value, json};
use tokio::process::Command;

pub(crate) struct Connection {
    service: RunningService<RoleClient, ()>,
    tools: Vec<Tool>,
}

impl Connection {
    pub(crate) async fn connect(command: &[String]) -> Result<Arc<Self>, String> {
        let (executable, arguments) = command.split_first().ok_or("MCP server command is empty")?;
        let mut process = Command::new(executable);
        process.args(arguments);
        let (transport, _) = TokioChildProcess::builder(process)
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?;
        let service = tokio::time::timeout(Duration::from_secs(10), ().serve(transport))
            .await
            .map_err(|_| "MCP initialization timed out".to_owned())?
            .map_err(|error| error.to_string())?;
        let tools = tokio::time::timeout(Duration::from_secs(10), service.list_all_tools())
            .await
            .map_err(|_| "MCP tool discovery timed out".to_owned())?
            .map_err(|error| error.to_string())?;
        Ok(Arc::new(Self { service, tools }))
    }

    pub(crate) fn definitions(&self, prefix: &str) -> Vec<Value> {
        self.tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": format!("{prefix}{}", tool.name),
                        "description": tool.description.as_deref().unwrap_or_default(),
                        "parameters": tool.input_schema.as_ref()
                    }
                })
            })
            .collect()
    }

    pub(crate) fn contains(&self, name: &str) -> bool {
        self.tools.iter().any(|tool| tool.name == name)
    }

    pub(crate) async fn call(&self, name: &str, arguments: &str) -> (String, bool) {
        let result = async {
            let tool = self
                .tools
                .iter()
                .find(|tool| tool.name == name)
                .ok_or_else(|| format!("Unknown MCP tool: {name}"))?;
            let arguments = serde_json::from_str(arguments)
                .map_err(|error| format!("Invalid MCP arguments: {error}"))?;
            let result = self
                .service
                .call_tool(
                    CallToolRequestParams::new(tool.name.to_string()).with_arguments(arguments),
                )
                .await
                .map_err(|error| format!("MCP tool call failed: {error}"))?;
            let mut content = Vec::new();
            for item in result.content {
                match item {
                    ContentBlock::Text(text) => content.push(text.text),
                    _ => return Err("MCP tool returned unsupported content".to_owned()),
                }
            }
            let text = content.join("\n");
            if result.is_error.unwrap_or(false) {
                Err(format!("MCP tool failed: {text}"))
            } else {
                Ok(text)
            }
        }
        .await;
        match result {
            Ok(content) => (content, true),
            Err(error) => (format!("Error: {error}"), false),
        }
    }
}

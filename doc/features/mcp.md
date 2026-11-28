# MCP

## Requirements

* User can use MCP for Blender tools in every context when the connection is established.
* User sees MCP tool calls and their results in the conversation.
* User is informed when the MCP connection or a tool call fails.
* The terminal remains clean when the MCP server writes diagnostic messages.

## User scenarios

### 1. User inspects a Blender scene

* User starts Blender with the MCP for Blender add-on enabled.
* User starts Shodan and asks it to describe the open Blender scene.
* Model calls the discovered scene-inspection tool.
* User sees the tool call and scene information.
* Model responds using the scene information.

### 2. User starts Shodan with a diagnostic message from the MCP server

* User starts Shodan while the MCP server writes a diagnostic message.
* User sees a clean terminal and can use the connected MCP tools.

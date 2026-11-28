import json
import sys


with open(sys.argv[1], encoding="utf-8") as transcript_file:
    transcript = [json.loads(line) for line in transcript_file]

responses = {
    "initialize": transcript[1],
    "tools/list": transcript[4],
    "tools/call": transcript[6],
}

with open(sys.argv[2], "w", encoding="utf-8") as request_file:
    for line in sys.stdin:
        request = json.loads(line)
        request_file.write(json.dumps(request) + "\n")
        request_file.flush()
        method = request["method"]
        if method == "notifications/initialized":
            continue
        if method not in responses:
            raise ValueError(f"Unexpected MCP request: {method}")
        if method == "tools/call":
            parameters = request.get("params", {})
            if parameters.get("name") != "get_scene_info" or parameters.get("arguments") != {}:
                raise ValueError(f"Unexpected MCP tool call: {request}")
        response = dict(responses[method])
        response["id"] = request["id"]
        print(json.dumps(response), flush=True)

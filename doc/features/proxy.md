# Provider proxy

## Requirements

* User can configure a SOCKS5 proxy and select which providers use it in `~/.shodan/settings.json`.
* The initial configuration routes OpenAI through `socks5h://127.0.0.1:2081`.
* Providers not selected for the proxy connect directly.
* An existing configuration without a proxy uses direct connections.
* Shodan preserves proxy settings when the user logs in.
* User sees an actionable error when proxy settings or the proxy connection cannot be used.

## User scenarios

### 1. User starts with the default proxy

* User starts Shodan for the first time.
* The provider configuration routes OpenAI through `socks5h://127.0.0.1:2081`.
* The provider configuration leaves llama using a direct connection.

### 2. User connects to a local server while OpenAI uses a proxy

* User configures a proxy for OpenAI and a local llama server.
* User starts Shodan and sends a message.
* User receives a response from the local server.

### 3. User connects to llama through a proxy

* User configures llama to use a SOCKS5 proxy.
* User starts Shodan and sends a message.
* User receives a response from llama through the proxy.

### 4. User sees a proxy connection error

* User configures OpenAI to use a SOCKS5 proxy.
* User runs `shodan login openai` and authorizes Shodan.
* The proxy cannot connect to OpenAI.
* User sees an error in the conversation area identifying the proxy connection.

### 5. User starts with invalid proxy settings

* User configures an invalid proxy in `~/.shodan/settings.json`.
* User starts Shodan.
* User sees an error identifying the configuration file.

### 6. User connects with an existing configuration

* User has an existing provider configuration without proxy settings.
* User starts Shodan and sends a message to llama.
* User receives a response through a direct connection.
* Shodan preserves the provider configuration.

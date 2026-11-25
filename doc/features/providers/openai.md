# OpenAI provider

## Requirements

* User can log in to OpenAI with `shodan login openai` using a ChatGPT subscription.
* Shodan displays the authorization URL and login instructions in the conversation area.
* Shodan copies the authorization URL to the clipboard when possible and confirms when it succeeds.
* User can complete login through a browser callback or by entering the final redirect URL in the message input.
* User sees login errors in the conversation area.
* After a recoverable login error, user can continue the same login attempt.
* After successful login, user sees the normal chat view.
* OpenAI credentials are stored in `~/.shodan/openai.json` with restricted permissions and reused in later sessions.
* Shodan refreshes expired credentials when possible.
* User sees an actionable error when authentication fails.
* Shodan verifies the identity returned by OpenAI before saving login credentials.
* Login selects OpenAI and its configured default model for the current session.
* OpenAI receives the same [host identifier](https://developers.openai.com/siwc/token-sharing-open-source#a-client-vs-an-agent-host) on every login from the same installation.
* When OpenAI is not configured, login selects its initial default model.
* First login adds OpenAI to the provider configuration without changing the default provider.
* The initial OpenAI model list contains `gpt-6.1-terra` with thinking level `high`, followed by `gpt-6-luna` with thinking level `max`.
* The initial default OpenAI model is `gpt-6.1-terra`.
* Subsequent logins preserve the existing OpenAI provider configuration and update credentials.
* User can chat with configured OpenAI models, receive streamed responses, and use tools.
* OpenAI retains the context of earlier reasoning when the user continues a conversation.

## User scenarios

### 1. User logs in to OpenAI

* User starts with the default llama configuration and runs `shodan login openai`.
* Shodan displays an OpenAI authorization URL in the conversation area and copies it to the clipboard.
* User sees `openai gpt-6.1-terra high` in the status panel.
* Below the URL, user sees `Complete sign-in on this machine, or paste the redirect URL below and press Enter. The authorization URL was copied to your clipboard.`
* User opens the URL in a browser on the same machine, signs in to ChatGPT, and authorizes Shodan.
* The browser redirects to Shodan's local callback.
* Shodan opens the normal chat view.
* Shodan saves credentials in `~/.shodan/openai.json`.
* Shodan adds OpenAI with `gpt-6.1-terra` and `gpt-6-luna` to `~/.shodan/settings.json`.
* OpenAI uses `gpt-6.1-terra` as its default model.
* User sees `openai gpt-6.1-terra high` in the status panel and can send a message.
* The default provider remains `llama`.

### 2. User logs in to OpenAI on a headless machine

* User runs `shodan login openai` on a headless machine without clipboard access.
* Shodan displays an OpenAI authorization URL in the conversation area.
* User sees `Complete sign-in on this machine, or paste the redirect URL below and press Enter.`
* User opens the URL in a browser on another device, signs in to ChatGPT, and authorizes Shodan.
* User copies the final redirect URL from the browser, pastes it into the message input, and presses Enter.
* Shodan saves the credentials, adds the OpenAI provider, and opens the normal chat view.
* User sees `openai gpt-6.1-terra high` in the status panel and can send a message.

### 3. User logs in again

* User has an OpenAI provider configuration with `gpt-6-luna` as the default model and thinking level `max`.
* User runs `shodan login openai`.
* User sees `openai gpt-6-luna max` in the status panel.
* User completes login.
* Shodan updates the credentials and preserves the OpenAI provider configuration.
* User sees `openai gpt-6-luna max` in the status panel and can send a message.
* The configured default provider remains unchanged.

### 4. User chats with the default OpenAI model

* User has logged in to OpenAI and starts Shodan with `--provider openai`.
* User sees `openai gpt-6.1-terra high` in the status panel.
* User sends a message and sees the response stream.
* User sends another message and receives a response that continues the conversation.

### 5. User cycles to Luna

* User has logged in to OpenAI and starts Shodan with `--provider openai`.
* User presses ctrl+space.
* User sees `openai gpt-6-luna max` in the status panel.
* User sends a message and receives a response using thinking level `max`.

### 6. User uses a tool with OpenAI

* User starts an OpenAI session and asks Shodan to read `greeting.txt`.
* The model requests the file and receives its contents.
* User sees the tool call and result followed by the model response.

### 7. User continues with refreshed credentials

* User has expired OpenAI credentials that can be refreshed.
* User starts an OpenAI session and sends a message.
* Shodan refreshes and saves the credentials.
* User receives a response.

### 8. User sees an authentication error

* User starts an OpenAI session with credentials that cannot be refreshed.
* User sees an error directing them to run `shodan login openai`.

### 9. User sees a login error

* User runs `shodan login openai` and authorization fails.
* User sees the authorization error in the conversation area.
* The current provider and model remain selected.

### 10. User watches OpenAI thinking

* User sends a message to an OpenAI model with thinking enabled.
* User sees the model's thinking stream before its response.

### 11. User views OpenAI usage

* User sends a message to OpenAI and receives a response with token usage and cache hits.
* User sees the updated token counts and cache hit percentage in the status panel.

### 12. User aborts an OpenAI response

* User sends a message and sees part of the OpenAI response.
* User presses Esc and sees `Request was aborted`.
* User sends another message and receives a response.

### 13. User sees an OpenAI request error

* User sends a message to OpenAI and the request fails.
* User sees `Request failed` and an explanation of the error.
* User sends another message and receives a response.


### 14. User starts OpenAI login with the initial model

* User starts with the default llama configuration and runs `shodan login openai`.
* User sees `openai gpt-6.1-terra high` in the status panel.

### 15. User starts OpenAI login with a configured model

* User configures OpenAI with `gpt-6-luna` as its default model and thinking level `max`.
* User runs `shodan login openai`.
* User sees `openai gpt-6-luna max` in the status panel.

### 16. User corrects a redirect URL

* User runs `shodan login openai`.
* User enters an invalid redirect URL and sees an error.
* User enters the valid redirect URL.
* Shodan completes login and opens the normal chat view.

### 17. User completes login after an invalid redirect URL

* User runs `shodan login openai`.
* User enters an invalid redirect URL and sees an error.
* User completes authorization through the browser callback.
* Shodan completes login and opens the normal chat view.

### 18. User sees an unavailable callback error

* User runs `shodan login openai` while the callback port is unavailable.
* User sees the callback error in the conversation area.

### 19. User continues login after invalid redirect URL

* User runs `shodan login openai`.
* User enters an invalid redirect URL and sees an error.
* User enters an authorization denial URL and sees the authorization error.

### 20. User receives callback after invalid redirect URL

* User runs `shodan login openai`.
* User enters an invalid redirect URL and sees an error.
* User receives an authorization denial through the browser callback and sees the authorization error.

### 21. User continues after OpenAI thinking

* User receives an OpenAI response with thinking.
* User sends a follow-up message.
* OpenAI receives the earlier reasoning context and responds to the follow-up.

### 22. User sees an invalid OpenAI identity

* User authorizes an OpenAI login.
* OpenAI returns an identity that cannot be verified.
* User sees an authentication error and can start login again.

### 23. User starts OpenAI login again

* User starts OpenAI login twice on the same installation.
* Both authorization requests identify the same installation.
* Shodan preserves the configured settings.


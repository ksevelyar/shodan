# Shodan [![CI](https://github.com/ksevelyar/shodan/actions/workflows/ci.yml/badge.svg)](https://github.com/ksevelyar/shodan/actions/workflows/ci.yml)

![](doc/shodan-1.png)
![](doc/shodan-2.png)

Shodan is a console agent inspired by pi.

It uses switchable system prompts instead of skills. Users can choose the instructions needed for a task and conserve tokens, allowing [local inference](https://github.com/ksevelyar/playground-inference) models to perform better.

## Documentation
* [Architecture](doc/architecture.md)
* [Boundaries](doc/boundaries.md)

### Features
* [Chat](doc/features/chat.md)
* [Context switching](doc/features/context-switching.md)
* [Sessions](doc/features/sessions.md)
* [Run bash commands](doc/features/tools/bash.md)
* [List directory](doc/features/tools/ls.md)
* [Read file](doc/features/tools/read.md)
* [Search file contents](doc/features/tools/grep.md)
* [Find paths](doc/features/tools/find.md)
* [Write file](doc/features/tools/write.md)
* [Plan mode](doc/features/modes/plan.md)
* [llama provider](doc/features/providers/llama.md)
* [OpenAI provider](doc/features/providers/openai.md)
* [Models](doc/features/models.md)
* [Response recording](doc/features/responses-recording.md)
* [Status panel](doc/features/status-panel.md)
* [Interface](doc/features/interface.md)

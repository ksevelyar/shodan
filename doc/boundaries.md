## Documentation
* Requirements must not include implementation details.

## Style
* prefer expect over unwrap to document invariant
* rust edition is 2024, mod.rs forbidden

## Tests
* Provider mocks must use responses recorded from real providers.
* Only feature tests allowed
* add requirements and write user scenarios for this requirements before implementation, prefer TDD
* Every feature test corresponds to one user scenario.
* Name every feature test after its user scenario in snake case, omitting articles. `A user creates a chain` becomes `user_creates_chain`.

# Settings

## Requirements

* Shodan creates `~/.shodan` on startup when needed.
* Shodan creates `~/.shodan/settings.json` when it does not exist.
* Shodan preserves existing settings.
* User sees an actionable error when the settings file is invalid.

## User scenarios

### 1. User starts Shodan for the first time

* User has no `~/.shodan` directory.
* User starts Shodan with Inkling-2-Mini-UD-IQ3_XXS available on the llama router.
* Shodan creates `~/.shodan/settings.json` with llama as the default provider and Inkling-2-Mini-UD-IQ3_XXS as its default and only model.
* User sees `llama Inkling-2-Mini-UD-IQ3_XXS low` in the status panel.

### 2. User starts with an existing Shodan directory

* User has a `~/.shodan` directory without `settings.json`.
* User starts Shodan with Inkling-2-Mini-UD-IQ3_XXS available on the llama router.
* Shodan creates the default provider configuration and selects Inkling-2-Mini-UD-IQ3_XXS.

### 3. User starts with existing settings

* User has an existing `~/.shodan/settings.json`.
* User starts Shodan.
* Shodan preserves the settings file.

### 4. User starts with invalid configuration

* User starts Shodan with an invalid `~/.shodan/settings.json`.
* User sees an error identifying the configuration file.

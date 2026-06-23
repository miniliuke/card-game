# Token Double-Click and AI Budget Design

## Scope

Fix token selection so a user can click the same normal-color supply twice to take two tokens when that bank pile contains at least four tokens. Increase the production AI search budget to stop after either five seconds or 10,000 iterations, whichever happens first.

## Token interaction

- The first click on a normal-color supply keeps the existing different-color selection behavior.
- If exactly that one color is selected, a second click on it immediately queues `TakeTwoSameTokens(color)` when the bank still has at least four tokens.
- The queued action clears the selection buffer. There is no persistent `2/2` confirmation state.
- Repeating a color after multiple different colors have already been selected does not discard the in-progress different-color selection.
- The existing `x2` badge remains available and keeps its direct-submit behavior.
- Keyboard activation follows the same rules as mouse activation.
- The rules layer remains authoritative; UI checks only provide immediate feedback and the queued action is still validated normally.

## AI budget

`MctsConfig::normal()` will use:

- `time_limit = 5 seconds`
- `iteration_limit = Some(10_000)`

The existing search-loop conjunction already implements "first limit reached" behavior. Explicit deterministic configurations created with `for_iterations(n)` remain iteration-only and keep `Duration::MAX`.

## Tests

- A selection helper test will reproduce the current failure: clicking an already-selected color with a bank count of four must return `TakeTwoSameTokens` and clear the buffer.
- Tests will cover the below-four case and preserve different-color selection behavior.
- Mouse and keyboard handlers will share the tested helper to avoid behavior drift.
- A configuration test will assert the five-second and 10,000-iteration production limits.
- The focused tests and full Rust test suite will run after implementation.

## Non-goals

- Do not change Splendor token legality rules.
- Do not force the AI to prefer same-color token actions.
- Do not change rollout depth, evaluation weights, or deterministic test budgets.

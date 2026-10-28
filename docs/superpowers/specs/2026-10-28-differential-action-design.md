# Differential GitHub Action design (2026-10-28)

## Goal
Keep existing snapshot mode as default. Add opt-in differential CI with two levels: a built-in Zcash SDK self-comparison negative control for easy onboarding, and custom caller-supplied compatible runners for genuine wallet comparisons. Defer managed external wallet builds to later.

## Chosen architecture
A composite Action dispatches a small Python entrypoint for safely preserving argument boundaries and handling JSON evidence. It compiles the source-pinned ShieldSpec fuzzer; for the built-in preset it also compiles ShieldSpec's own Zcash SDK runner, using the same binary on both sides. Custom mode runs caller-provided executables using caller-workspace-relative or absolute paths. No arbitrary wallet checkout/build commands.

## Inputs
mode=snapshot|differential (snapshot default); candidate remains required at runtime for snapshot. Differential: preset=zcash-sdk|custom (default zcash-sdk), baseline-runner and candidate-runner required only for custom, seed/cases/scenario-family/policy and optional newline-delimited runner arguments. Validate bounds and allowed values and preserve argument boundaries.

## Outputs and evidence
Keep status 0/1/2; expose report and discovery paths. Save machine-readable campaign report and a minimized discovery if found; upload existing files as a pinned upload-artifact action step even when the campaign exits with a difference. Preserve the original process status in a final step. Clearly label policy-decision differences as observations, not proven privacy vulnerabilities.

## Tests
Test-first script validation, path/argument injection resistance, invalid input, backwards-compatible snapshot, both presets and exit statuses. GitHub Actions integration validates a clean first-party negative control and controlled custom difference including evidence/artifact retention. Rust, Python and site suites remain green.

## Release
Implement through PR and test consumer workflow. Do not move v0.2.0 tag. Publish a future immutable Action tag only after passing CI.

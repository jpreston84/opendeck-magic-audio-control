# Refactor Checklist

## Phase 1 - Shared Constants And Target Model
- [ ] Add shared constants for Master, Mic, unlock timing, monitor timing, and debounce timing.
- [ ] Add a shared `AudioTarget` abstraction for `Master`, `Mic`, and app streams.
- [ ] Keep existing settings fields and persisted JSON shape unchanged.
- [ ] Validate cycle order remains `blank -> Master -> Mic -> streams -> blank`.
- [ ] Validate no-stream behavior after Mic still reaches blank correctly.

## Phase 2 - Target-Based Audio Service
- [ ] Add target-based wrappers for volume, mute, toggle mute, set volume, and adjust volume.
- [ ] Add shared helpers for deriving icon names and display names from current selection.
- [ ] Reuse current `pactl` behavior under the wrapper without changing semantics.
- [ ] Validate volume clamping and mute behavior for Master, Mic, and app streams.

## Phase 3 - State Repository Facade
- [ ] Add a repository facade over persistent selection/button/knob state and runtime lock state.
- Keep `/tmp/opendeck-audio-streams.json` path and field names stable.
- [ ] Migrate call sites to the facade instead of direct `shared_state` access.
- [ ] Validate button/knob registration on appear/disappear.
- [ ] Validate selection persistence across restart.

## Phase 4 - Shared UI Display Helpers
- [ ] Extract shared knob label formatting.
- [ ] Extract shared button and knob update helpers.
- [ ] Extract shared icon rendering decisions for blank, muted, and target-specific states.
- [ ] Keep current visuals unchanged while moving logic.
- [ ] Validate blank, Master, Mic, stream, muted, and unlocked visuals.

## Phase 5 - Thin Action Handlers
- [ ] Reduce cycle action to event wiring plus shared service calls.
- [ ] Reduce volume action to event wiring plus shared service calls.
- [ ] Keep short-press mute, long-press unlock, unlocked cycle refresh, and knob-lock behavior unchanged.
- [ ] Validate property inspector payloads remain unchanged.

## Phase 6 - Simplified Monitor
- [ ] Migrate the monitor to the shared target/state/display helpers.
- [ ] Keep poll interval and knob cooldown behavior unchanged.
- [ ] Ensure mute-only and volume-only display updates both work.
- [ ] Validate linked button icon refresh and blank fallback behavior.

## Final Verification
- [ ] Build cleanly.
- [ ] Verify all user-visible behaviors from the regression list still work.
- [ ] Review for duplicated target branching and duplicated knob label logic.

# Dorfromantik Solver

A tile-placement advisor for the game Dorfromantik. Reads the game's savegame file, renders the hex map, and suggests optimal tile placements based on edge matching, group effects, and quest progress.

## Build & Test

```sh
cargo clippy --all-targets -- -D warnings   # pre-commit hook runs this
cargo test                                    # some tests need savegames in tests/fixtures/
cargo run                                     # main solver UI
cargo run -- path/to/savegame.sav             # load specific save
```

The pre-commit hook runs `cargo fmt`, then `cargo clippy --all-targets -- -D warnings`. Both must pass.

## Architecture

**Binary-only modules** (GUI app):
- `src/app.rs` — main application state, wires everything together
- `src/main.rs` — event loop (winit), input handling
- `src/render/` — wgpu GPU rendering (shader, pipeline, camera, textures)
- `src/ui/` — egui UI (sidebar, placement table, quest labels, status bar)
- `src/game/` — game integration (screenshot detection, navigation, game camera model)
- `src/file_watcher.rs` — savegame file watching and background loading
- `src/game_data.rs` — container for map + analysis results

**Library modules** (also used by examples/tests):
- `src/coords.rs` — type-safe coordinate wrappers (HexPos, WorldPos, ScreenPos, PixelPos)
- `src/hex.rs` — hex grid math (axial coordinates, neighbor positions, world conversion)
- `src/map.rs` — tile map parsed from savegame (tiles, segments, quests)
- `src/data/` — domain types (Terrain, Form, Segment, Side, EdgeProfile, tile tables)
- `src/raw_data.rs` — NRBF savegame deserialization
- `src/group.rs` — connected terrain groups
- `src/group_assignments.rs` — flood-fill group computation
- `src/best_placements.rs` — placement scoring (edge matching, fit chance, group/quest effects)
- `src/tile_frequency.rs` — tile pattern frequency analysis

## Coordinate Spaces

Four distinct types — never mix them:

| Type | Inner | Space | Origin |
|------|-------|-------|--------|
| `HexPos` | `IVec2` | Discrete hex grid (axial coords) | (0,0) = first tile |
| `WorldPos` | `Vec2` | Continuous top-down, 1 hex = 1.5 units wide | hex_to_world(HexPos) |
| `ScreenPos` | `Vec2` | Normalized 0..1 | (0,0)=top-left, (1,1)=bottom-right |
| `PixelPos` | `Vec2` | Window pixel coordinates | (0,0)=top-left |

Conversions go through `hex.rs` (Hex↔World) and camera modules (World↔Screen↔Pixel).

## Two Cameras

- **`render::camera::Camera`** — the solver's UI camera. 2D orthographic pan/zoom of the hex map.
- **`game::game_camera::GameCamera`** — model of the game's 3D perspective camera. Used for screenshot unprojection and viewport detection. Parameters extracted from Dorfromantik's Unity scene files:
  - Pitch: 33deg from horizontal (57deg from vertical in the code's convention)
  - FOV: 30deg vertical
  - Distance: 96 world units (derived from tiles_across=61)

## Game Mechanics (from decompiled Assembly-CSharp.dll)

**Quest target formula:**
```
target = max(ReferenceGroupCount, minTargetCount) + condition.targetValue + DifficultyIncrease(level)
```
- `ReferenceGroupCount`: for "more than" quests = largest open group of that terrain; for "exactly" quests = random from top 4
- `DifficultyIncrease = round(pow(level, expFactor) / levelsNeededPerIncrease * targetValueIncrease * multipliers)`
- Level increases by 1 per fulfilled bubble quest (not flag quests)
- At high levels (~360), difficulty dominates for Forest/Village/Wheat; group size dominates for Water/Rail

**Quest types:** MoreThan (>=), Exact (==), Flag (close group). Forest has no "exactly" variant.

**Unit counts:** Quest targets count visual elements (trees, houses, fields), not segments. The count comes from Element/InstanceableVisual GameObjects on each tile prefab. Our `tile_table.rs` and `form.rs::default_unit_count` approximate these. Known accurate: Size1 Forest=4, Size2 Forest=10. Some quest tile variants may still be off.

## Platform

Linux/Wayland only. Game integration uses:
- `libwayshot` for screenshots (wlr-screencopy protocol)
- `enigo` for mouse input simulation
- `niri-ipc` for window management (niri compositor)
- `opencv` for template matching in viewport detection

**Savegame locations:**
- Active (Steam/Proton): `~/.local/share/Steam/steamapps/compatdata/1455840/pfx/drive_c/users/steamuser/AppData/LocalLow/Toukana Interactive/Dorfromantik/Saves/SaveGame_Classic_2023-02-05-12-06-16.sav`
- Calibration copy: `calibration/savegame.sav` (stale — copied from Steam save at one point, used by examples)
- Test fixtures: `tests/fixtures/` (biggame.sav, mini.sav, dorfromantik.dump)

## Conventions

- No `println!`/`eprintln!` in src/ — use `log::info!`/`warn!`/`error!`
- No `unwrap()`/`expect()`/`panic!()` in non-test code — handle errors, log warnings, use defaults
- No `#[allow(dead_code)]` — delete unused code or prefix unused deserialized fields with `_`
- Write scripts to temp files and execute, don't use inline heredocs in bash
- Use `mv` and `cp` normally (not `cat > redirect`)
- Don't auto-run builds/scripts after editing — wait for the user to ask

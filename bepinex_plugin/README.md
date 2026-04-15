# CamExport — Camera position export for Dorfromantik

Exports the game camera's position, rotation, and FOV every 10 frames
by hardpatching `Assembly-CSharp.dll`. No BepInEx or launch options needed.

## Output

**File**: `<game_dir>/camera_pos.txt` — overwritten every 10 frames.

Format: `x y z rotX rotY rotZ fov` (7 space-separated floats)

Example: `0.0000 5.4464 -8.3867 33.0000 0.0000 0.0000 30.0000`

- `x, z` — camera world position on the ground plane (changes when panning)
- `y` — camera height (changes with zoom)
- `rotX` — pitch (33° = the fixed camera tilt)
- `rotY` — yaw (changes when rotating the view)
- `rotZ` — roll (always 0)
- `fov` — vertical field of view (30°)

## How it works

The patcher uses Mono.Cecil to:
1. Add a static `CamExportPatch.Export()` method to `Assembly-CSharp.dll`
2. Inject a `call CamExportPatch::Export()` at the start of `CameraMovement.LateUpdate()`

The injected method reads `Camera.main.transform.position/eulerAngles` and
`Camera.main.fieldOfView`, formats them as text, and writes to `camera_pos.txt`.

No BepInEx, no doorstop, no DLL proxies, no launch options.

## Install

### 1. Build and run the patcher

```sh
cd bepinex_plugin
dotnet run --project Patcher.csproj
```

This patches `Assembly-CSharp.dll` in place (backs up to `.dll.orig` first).

### 2. Launch the game normally

No Steam launch options needed.

### 3. Verify

```sh
cat ~/.local/share/Steam/steamapps/common/Dorfromantik/camera_pos.txt
```

## Unpatch

Restore the backup:

```sh
cd ~/.local/share/Steam/steamapps/common/Dorfromantik/Dorfromantik_Data/Managed
cp Assembly-CSharp.dll.orig Assembly-CSharp.dll
```

Or verify game files through Steam (right-click → Properties → Local Files → Verify).

## Re-patch after game update

Game updates overwrite `Assembly-CSharp.dll`. Re-run the patcher:

```sh
cd bepinex_plugin
dotnet run --project Patcher.csproj
```

The patcher detects if already patched and skips if so.

## Why not BepInEx?

BepInEx's doorstop DLL proxy (`winhttp.dll`) crashes under Proton Experimental
due to stripped Mono core libraries in Dorfromantik v1.1.5.3. The unstripped
corlibs fix the crash but break the game UI. Hardpatching avoids the entire
doorstop/DLL-proxy/Mono-initialization chain.

# Serious Sam: Shatterverse Save Editor

A portable Windows desktop app for viewing and editing supported fields in Serious Sam: Shatterverse save files. Version **0.4.11 beta**. The app uses a Rust/Tauri backend and keeps save processing on the local computer.

## What the app can do

- Open a `.sav` file and inspect its recognized Unreal GVAS properties.
- Edit currency values already present in the save. Values must be at least 1. A missing currency is shown as unavailable; earn it in-game, then reopen the save before editing it.
- View and change recognized character, weapon, Intel, Codex, challenge, and progression values. The Field Lab lists recognized fields with their stored type and offset; raw edits are marked unsafe.
- Sam, weapon, and Intel unlock toggles interpret the saved `bL` locked flag correctly: `0` means unlocked and `1` means locked.
- Browse owned trinkets, inspect their rarity, rank, level, magnitude, base roll, shape, and decoded secondary stats, and edit the fields the parser recognizes.
- Max one or all owned trinkets to Level 6 and Rank 6. Magnitude increases by 7 percentage points per rank gained. Base roll and all other fields are unchanged.
- Change the rarity suffix for compatible trinkets and replace an existing owned slot with a compatible observation from the Workshop.
- Use the Workshop's bundled observed trinket records, import observations from another save or an exported `.trinket`, and keep imported observations in this app's local browser storage.
- Create or remove a trinket where the save structure supports it. These operations change the save structure and are explicitly high risk.
- Export a trinket record, create and restore timestamped backups, undo staged edits, and save only after a confirmation prompt.

The editor does not upload save files. It checks that a save has not changed on disk since it was opened and creates a backup beside the original before overwriting.

## Known limitations

This is a research editor for a partially reverse-engineered format, not a complete game database or a general Unreal save editor. It parses only known property types and structures. Unknown or unsupported compound data is preserved where possible, but is not understood or safely editable.

- Currency entries are not created by the editor. The game's save may omit a currency until it has been earned; trying to add the missing map entry has broken saves, including attempts to initialize it to 1.
- The Workshop's trinket observations are examples, not a complete canonical catalog or verified range table. Importing another save adds examples locally; it does not merge that save into the active save.
- Trinket creation requires a compatible owned structure template, including a matching secondary-stat key layout. Adding/removing items resizes serialized arrays and can still corrupt a save.
- Replacing a trinket in an existing slot is the preferred lower-risk option because it preserves the slot layout. It is not guaranteed safe for every save or game version.
- F is an unknown 0/1 flag. Editing it has broken saves; it remains marked unsafe and should be left unchanged unless experimenting on a backup copy. The app does not fully decode Catalyst Boons, equipped/loadout state, or every progress/unlock field. Some names and values remain raw or uncertain.
- Field Lab edits can change values whose meanings, constraints, or valid ranges are unknown. Avoid changing fields you cannot identify.
- Save merging is unsupported. Codex records represent unique entry types; duplicating entries while combining saves has produced invalid saves.
- The game can reject or rewrite a save after an edit even when the editor reports success. A backup helps recovery but does not make a risky edit safe.

## Known save-breaking cases

These are specific failure modes observed during development:

1. **Adding a missing currency:** the currency map changes shape and can corrupt the save. The app now leaves missing currencies read-only and requires existing balances to be at least 1.
2. **Adding or removing trinkets:** the serialized array size and count change. This can produce a save the game cannot load. The app asks for confirmation and makes a backup, but cannot guarantee a valid result.
3. **Merging saves or duplicating Codex entries:** entry types are expected to be unique. Duplicate entries can break save loading or game state. There is no save-merge feature.
4. **Editing unknown raw fields:** changing the wrong value, flag, count, or size can invalidate the serialized data. Treat the entire Field Lab as unsafe unless you know the field.

Keep the game closed while editing. Keep the backup until you have launched the game and verified the edited save in-game. For important saves, work on a copy first.

## Run the portable app

Run `release\Shatterverse Save Editor.exe`. It does not install the editor or create Start Menu entries. Windows WebView2 Runtime is required.

## Build from source

Requirements: Windows 10/11, Node.js with npm, Rust with the MSVC toolchain, Microsoft C++ Build Tools, and WebView2 Runtime.

```powershell
npm install
npm run tauri -- dev
.\build_windows.bat
```

The build script creates the portable executable in `release/`. Generated build files, personal save files, and backups are excluded from Git. Curated research templates remain in `save-templates/`.

## Data and repository

The bundled Workshop catalog contains 61 save-derived observations. Per-instance server IDs were removed. Values and ranges are empirical examples only. The app can import additional observations locally without modifying the active save.

**GitHub repository:** link not published yet. The app sidebar has a placeholder for the public repository URL.

Project credit: **xcibe95x**.

This project is independent and is not affiliated with or endorsed by Croteam or Devolver Digital. The current `LICENSE` file is a reminder only; select and add an actual license before publishing.

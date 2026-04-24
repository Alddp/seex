# SeEx Imported LCSC Parts Export Design

Date: 2026-04-24

## Goal

Extend the existing `Imported` page so a user can export scanned `LCSC Part` values to a text file for later one-click import workflows.

This feature exports only the scanned `LCSC Part` identifiers.

It does not export symbol names, schematic instances, or full library metadata.

## User Outcome

A user can open `Imported`, confirm which parts already exist in the current `nlbn` library, and write the corresponding `LCSC Part` list to a stable file path.

That exported file becomes a handoff artifact for later batch import or re-import flows.

## Scope

In scope:

- extend the existing `Imported` page
- export the currently scanned `LCSC Part` values to a `.txt` file
- persist a dedicated export path in app config
- provide `Browse`, `Apply`, and `Export` interactions
- deduplicate exported `LCSC Part` values
- keep export output deterministic and easy to consume by later tooling

Out of scope:

- automatic export after every refresh or `nlbn` run
- exporting CSV, JSON, or richer structured formats
- exporting `symbol_name` alongside `LCSC Part`
- adding edit or delete actions for imported symbols
- reading the export file back into SeEx in this change

## Alternatives

### Recommended: Dedicated Persisted Export Path

Add an `Imported`-page export path field and store it in the existing app config.

Pros:

- supports true one-click repeat export
- matches the app's existing save-path pattern
- gives the user explicit control over file location

Cons:

- adds one more persisted path setting
- slightly increases page complexity

### Alternative: Save Dialog Every Time

Show only an `Export` button and prompt for a file path on every export.

Pros:

- smallest implementation
- no new persisted config field

Cons:

- not suitable for repeat one-click workflows
- forces extra interaction each time

### Alternative: Automatic Background Export

Rewrite a fixed file whenever imported symbols are refreshed or `nlbn` export succeeds.

Pros:

- minimal repeated user action

Cons:

- implicit overwrite behavior
- higher surprise factor
- harder to understand when data changed

## Recommendation

Implement a dedicated persisted export path on the `Imported` page.

This best matches the user's stated goal of saving scanned `LCSC Part` values for later one-click import, while keeping the export action explicit and predictable.

## Data Source

The exported content should come from the same source used by the `Imported` page:

- scan the active `nlbn` output directory
- collect imported symbol records from `*.kicad_sym`
- read each symbol's `LCSC Part`

Do not introduce a separate imported-parts cache or index.

## Output Format

Write a plain text file with:

- UTF-8 text
- one `LCSC Part` per line
- no timestamps
- no symbol names
- no blank trailing records

Example:

```text
C2040
C7470135
C123456
```

## Deduping And Ordering

Before writing the file:

- deduplicate by exact `LCSC Part`
- preserve the stable ordering already produced by the imported-symbol scan

Because the imported-symbol list is already sorted by:

1. `lcsc_part`
2. `symbol_name`

the exported part list will naturally be deterministic when the first occurrence of each `LCSC Part` is retained.

## Backend Design

Add a dedicated save path for imported-parts export to monitor-config-backed application state.

New state/config field:

- `imported_parts_save_path: String`

New app-path helpers:

- default file path: app data directory + `imported_lcsc_parts.txt`
- path resolution helper following the same pattern as monitor history/matched files

New controller behavior:

- load imported symbols from the current active `nlbn` output path
- reuse the same backend scanner as the `Imported` page instead of reading browser-side rendered state
- collect unique `LCSC Part` values
- write them to the configured file
- return a user-facing result string such as:
  - `Exported to ...`
  - `No imported LCSC Part values to export`
  - `Export failed: ...`

New Tauri commands:

- `set_imported_parts_save_path`
- `save_imported_parts`

No background task is needed for this feature.

## Frontend Design

Extend the existing `Imported` page instead of creating a new page.

Page additions:

- an export action in the page header area near `Refresh`
- a save-path input row
- `Browse` button
- `Apply` button
- export-result feedback using the existing notice/error style

Behavior:

- the input shows the current persisted export path
- `Browse` opens a save-file dialog filtered to text files
- `Apply` stores the chosen path without exporting
- `Export` writes the current scanned `LCSC Part` list to the configured path
- if there are no imported rows, `Export` is disabled

The imported-symbol table remains unchanged:

- `LCSC Part`
- `Symbol Name`
- row-level `Copy`

## Config Design

Persist the new path in the existing `export_config.json` under monitor settings.

Updated monitor config shape:

```text
monitor: {
  history_save_path: "...",
  matched_save_path: "...",
  imported_parts_save_path: "..."
}
```

Backward compatibility rules:

- if the field is missing, fall back to the default app-data path
- saving config should include the new field once it is set in memory

## Error Handling

Expected outcomes:

- no imported symbols found -> warn-level result, no file write
- configured path parent directory missing -> create it if possible
- file creation/write failure -> error result
- imported-symbol scan failure -> error result with concise cause

The page should stay usable after failures.

`Refresh`, `Browse`, and retry export should continue to work independently.

## Testing

Backend tests:

- config load falls back when `imported_parts_save_path` is absent
- config save includes `imported_parts_save_path`
- default path resolves to `imported_lcsc_parts.txt`
- export writes unique `LCSC Part` values, one per line
- export preserves deterministic order
- export reports empty-state message when no imported parts are found
- export reports write errors clearly

Frontend checks:

- path input syncs from state
- export button disables when imported list is empty
- success, warning, and error messages render in the imported page feedback area
- browse/apply/export controls follow existing SeEx interaction patterns

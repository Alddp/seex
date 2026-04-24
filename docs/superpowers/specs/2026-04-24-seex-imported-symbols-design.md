# SeEx Imported Symbols Design

Date: 2026-04-24

## Goal

Add a dedicated `Imported` page to `seex` that shows symbols already exported through the current `nlbn` output library.

The page will list only:

- `LCSC Part`
- `symbol name`

This feature is a library browser, not a schematic instance browser.

It does not attempt to show placed schematic references such as `R12` or `U3`.

## User Outcome

A user can open `Imported` and immediately answer:

- which symbols already exist in the current `nlbn` library
- which `LCSC Part` each symbol belongs to

The page should help prevent duplicate exports and make it easier to confirm whether a part is already in the local KiCad symbol library.

## Scope

In scope:

- Add a new top-level page in the Tauri UI: `Imported`
- Scan the current `nlbn` output directory for `*.kicad_sym`
- Parse symbol entries and extract:
  - symbol name
  - `LCSC Part`
- Display the resulting list with refresh support
- Provide one-click copy for `LCSC Part`
- Refresh the list when:
  - the user opens the page
  - the user clicks `Refresh`
  - an `nlbn` export finishes successfully while the page is active

Out of scope:

- Parsing KiCad schematic instance files
- Showing actual placed references such as `R12`
- Building a generic KiCad library explorer for arbitrary paths
- Maintaining a separate export index database
- Editing or deleting imported symbols from the page

## Alternatives

### Recommended: Scan Real `.kicad_sym` Files

Use the current `nlbn` output directory as the single source of truth and scan actual KiCad symbol library files on demand.

Pros:

- always reflects the real exported library
- no risk of stale metadata
- fits the current `seex` export model

Cons:

- requires a small parser
- refresh cost is slightly higher than reading a cache

### Alternative: Maintain a SeEx-Owned Index

Record imported symbols into a JSON file after each export and render from that file.

Pros:

- simple page reads
- no library parsing at render time

Cons:

- drifts from reality if the library is edited manually
- introduces a second source of truth

### Alternative: Browse Arbitrary Symbol Libraries

Let the user select any `.kicad_sym` file and browse it.

Pros:

- more flexible long term

Cons:

- expands scope beyond the current workflow
- adds UI and path management that is not necessary for v1

## Recommendation

Implement the recommended approach: scan real `.kicad_sym` files from the active `nlbn` output directory.

This matches the existing export workflow, keeps the data trustworthy, and avoids introducing a sync problem between a cached index and the actual KiCad library.

## Data Source

The page will use the current configured `nlbn_output_path` from application state.

Scan rules:

- look only in the output root directory
- collect every file matching `*.kicad_sym`
- ignore non-symbol-library files

If no matching symbol library exists:

- return an empty list
- show a friendly empty state in the UI

## Data Model

Add a lightweight response type for imported symbols:

- `lcsc_part: String`
- `symbol_name: String`

The page response should also include the scanned directory path so the UI can explain where data came from.

Example response shape:

```text
{
  scanned_path: "...",
  items: [
    { lcsc_part: "C2040", symbol_name: "74HC00_C2040" }
  ]
}
```

## Parsing Strategy

Do not implement a full KiCad S-expression parser for v1.

Instead use a focused symbol-library scanner that understands the subset needed here:

- detect each top-level `symbol "..."` block
- capture the symbol name from the block header
- scan nested `property` blocks inside that symbol
- extract the property whose key is `LCSC Part`

Rules:

- include an item only when both `symbol name` and `LCSC Part` are present
- ignore symbols that do not contain `LCSC Part`
- tolerate unrelated properties and nested graphics sections
- tolerate multiple `.kicad_sym` files in the directory

The scanner should be deterministic and read-only.

## Deduping And Ordering

The backend should deduplicate by the tuple:

- `lcsc_part`
- `symbol_name`

Sort the final list by:

1. `lcsc_part`
2. `symbol_name`

This gives stable output across refreshes and makes repeated lookup easier.

## Backend Design

Add a new Tauri-side module dedicated to imported symbol scanning.

Responsibilities:

- resolve the current `nlbn` output path from controller state
- enumerate `.kicad_sym` files
- parse imported symbol records
- return a serializable response for the frontend

Add one new command:

- `get_imported_symbols`

Command behavior:

- reads current app state
- scans the library path
- returns parsed records or a clear error message

No config mutation happens in this command.

## Frontend Design

Add a new top-level page and sidebar item:

- `Imported`

Page layout:

- header with title and a short description
- scanned directory path line
- `Refresh` button
- main card containing the imported symbol list

List columns:

- `LCSC Part`
- `Symbol Name`
- row action: `Copy`

Page states:

- loading
- empty
- error
- populated list

Interaction:

- entering the page triggers a load
- `Refresh` triggers a reload
- clicking `Copy` copies the row’s `LCSC Part`

## Refresh Behavior

The page reloads when:

- the page becomes active
- the user clicks `Refresh`
- `nlbn` export finishes successfully and the active page is `Imported`

It does not poll the filesystem continuously.

## Error Handling

Expected failure cases:

- configured `nlbn` output directory does not exist
- output directory exists but contains no `*.kicad_sym`
- symbol library file cannot be read
- symbol library file has malformed content

UI behavior:

- missing library file or empty library -> empty state
- unreadable or malformed file -> error state with a concise message

The page should remain usable after an error; `Refresh` should retry from scratch.

## Testing

Backend tests:

- parses a minimal `kicad_sym` fixture with one symbol
- parses multiple symbols from one file
- ignores symbols without `LCSC Part`
- merges symbols from multiple library files
- deduplicates identical entries
- sorts output deterministically

Frontend coverage:

- page switch triggers load
- refresh button triggers reload
- copy button sends the expected text
- empty and error states render the correct message

## Out-Of-Scope Follow-Ups

Potential later extensions:

- search and filter on `LCSC Part` or symbol name
- open the scanned library file in Finder
- show source library filename
- display additional metadata such as `Manufacturer`
- parse actual KiCad schematic instances in a separate feature

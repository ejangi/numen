# Numen Developer & Agent Handover Guide (AGENT.md)

This document provides developer-level documentation detailing the key technical trade-offs, system assumptions, and architectural design decisions made during the pair-programming session between the User and the AI Agent (**Antigravity**).

---

## 1. Design Decisions & Trade-Offs

### Python-Rust Boundary: PySide6 + PyO3/Maturin
* **Decision**: We chose to compile the Rust parsing engine as a native CPython extension (`numen_engine`) using **PyO3** and **Maturin**, rather than a full C++ Qt6 setup or Rust bindings like `cxx-qt`/`qmetaobject-rs`.
* **Rationale**:
  - Arch Linux was equipped with a fresh Python 3.14 toolchain, but lacked heavy compilation dependencies like CMake or global development libraries.
  - Maturin compiles Rust code directly using standard `cargo` toolchains and places the `.so` binary in a local `.venv` automatically. This bypasses the need for large, fragile C++ linking and packaging scripts.
  - Performance remains high: the UI thread runs Python (Qt6) and handles event dispatching, while heavy string parsing and AST calculations are offloaded to a background thread accessing compiled Rust code.

### Layout Sync: Custom `ResultsCanvas` Rendering
* **Decision**: Rather than using two separate synchronized `QPlainTextEdit` scroll areas, we built a custom `ResultsCanvas` widget on the right that repaints calculations based on the document layout geometry of the notepad.
* **Rationale**:
  - Synchronizing two scroll areas falls out of alignment when lines wrap (e.g., a long expression wraps to 3 lines on the left, but its evaluated single-line value only occupies 1 line on the right).
  - By querying `notepad.firstVisibleBlock()`, translating the coordinates with `contentOffset()`, and querying `blockBoundingGeometry(block).height()`, the right canvas paints each result at the *exact* vertical midpoint or coordinate matching the notepad's visual block layout, guaranteeing pixel-perfect alignment.
  - Repaints are triggered on scrolling, resizing, text modification, and cursor movement.

### Non-Intrusive Prose Filtering via PEG Ordered Choices
* **Decision**: All non-math text (prose) is matched either as a `#` / `//` `comment` (which consumes the rest of the line), or as general `prose` (which matches alphabetic and symbol words).
* **Rationale**:
  - By using ordered choices in `line = { SOI ~ (expr | comment | prose)* ~ EOI }`, we try to parse mathematical expressions (`expr`) first.
  - If a sequence does not form valid math (like the parenthesized comment `# 4. Active Currencies (Multi-Currency Support)` or an inline prose keyword like `"distance in miles"`), it backtracks and is matched as `comment` or `prose`.
  - In `eval.rs`, when a general word matches `Expr::Variable`, it is looked up in the symbol table. If undefined, it resolves to `ResultValue::Empty`. This allows standalone words to be silently ignored during line summation, preventing parsing errors.
  - Undefined variables used *within* math operators (e.g. `NoahYears2Go + LaraYears2Goo` where `LaraYears2Goo` is a typo) correctly fail with a type mismatch error (`Cannot perform Add on Number and Empty`), making error-reporting robust for actual calculations.

### Arbitrary Named Variable Bindings
* **Decision**: Implemented an explicit assignment rule `assignment = { identifier ~ "=" ~ expr }` in the Pest grammar and a persistent symbol table (`HashMap<String, ResultValue>`) inside the evaluator.
* **Rationale**:
  - Allows users to bind results to case-insensitive, alphanumeric names (e.g. `NoahYears2Go = 8`) and reference them in subsequent calculations.
  - Variable names are normalized to lowercase during parsing and lookup to ensure case-insensitive usability.

### Local-First Document Storage Drawer & Auto-Save
* **Decision**: Added a sliding document drawer layout (QFrame + QListWidget) inside the QSplitter, storing files as `.md` files in `~/Documents/numen/` on Linux.
* **Rationale**:
  - Auto-Save is triggered debounced on text changes: any editor edits are saved directly back to the active `.md` file, providing a zero-config, local-first note storage.
  - A loading lock (`self.is_loading_doc`) prevents auto-saving blank states when loading files.
  - Files are automatically listed and sorted in descending order of modification time (`mtime`) on startup, letting the user immediately resume work on their most recent document.

---

## 2. Key Code Modules

* **`src/grammar.pest`**: Defines the grammar rules. Order of choice is `expr | comment | prose`.
* **`src/parser.rs`**: Builds a `Vec<Expr>` representing the line. Handles expressions, variable assignments, conversions, and binary operators.
* **`src/units.rs`**: Tracks length (`m`, `km`, `miles`), time (`s`, `min`, `h`), and mass (`g`, `kg`, `lbs`) conversions.
* **`src/currency.rs`**: Pivot-based cross conversions. Falls back to a static hardcoded map if `~/.config/numen/currencies.json` doesn't exist.
* **`src/eval.rs`**: Line-by-line evaluator. Carries the symbol table for named variable lookup, evaluates conversions and binary operations, and handles `ResultValue::Empty` propagation.
* **`numen_gui.py`**: The GUI application featuring the document drawer list, auto-save hooks, debounced math evaluation, and PySide thread workers stored in a tracker set to prevent premature garbage-collection warnings.

---

## 3. System & Build Assumptions

- **Python Interpreter**: Python 3.14+.
- **PyO3 Build Environment**: Requires `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1` flag in compilation since PyO3 0.21's static ABI check caps at Python 3.12.
- **Offline First**: All basic math works offline. Currency updates are performed using Python's standard `urllib` library in a background worker thread on startup.

---

## 4. Future Roadmap for Next Agents

If you are a future agent tasked with expanding this codebase, consider implementing:
1. **Rich Highlighting**: Add a syntax highlighter subclassing `QSyntaxHighlighter` to the left notepad to colorize numeric values, operators, units, comments, and variable line/name references differently.
2. **Advanced Units**: Support more dimensional scales (e.g. data storage `MB`, `GB`; temperature `C`, `F`, `K`; velocity `mph`, `kmh`).
3. **Sidebar Renaming/Deletion**: Allow right-clicking on list items in the drawer to rename or delete documents directly from the UI.
4. **Dark Mode Toggle**: Include a settings button to switch between dark and light modes.

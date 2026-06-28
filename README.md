# Numen — Natural Language Notepad Calculator

Numen is an open-source, Linux-native app that allows you to quickly write and calculate with natural language. It features a reactive, synchronized split-pane UI where the left pane is a rich natural-language notepad and the right pane displays real-time, line-by-line mathematical evaluation.

```
┌────────────────────────────────────────────────────────┐
│  NUMEN                                                 │
├────────────┬──────────────────────────┬────────────────┤
│            │ Salary $5000 - $1200 tax │         $3800  │
│ [Welcome]  │ Health insurance: $350   │          $350  │
│ [Budget]   │ Rent: $1500              │         $1500  │
│ [Trip]     │ Total monthly = line5+6  │         $1850  │
│            │ Savings = line4 - line7  │         $1950  │
└────────────┴──────────────────────────┴────────────────┘
```

---

## Key Features

1. **Document Drawer Sidebar Panel**:
   - Integrates directly as a collapsible/resizable panel in the horizontal split view.
   - Houses a list of available files in your `~/Documents/numen/` directory, loaded as `.md` files.
   - Automatically sorts files in descending order based on their modification date (`mtime`) on disk.
   - Right-click context menu options to **Rename** or **Delete** (with permanent deletion confirmation dialogs).
2. **Auto-Save**:
   - Automatically saves Notepad edits back to their respective markdown files on disk using debounced keystroke save-locks.
3. **Arbitrary Named Variable Bindings**:
   - Allows declaring variables inline (e.g. `NoahYears2Go = 8`, `LaraYears2Go = 11`).
   - Resolves variable names case-insensitively in subsequent lines (e.g. `KidsYears2Go = NoahYears2Go + LaraYears2Go` prints `19`).
4. **Natural Language Math Parsing**:
   - Automatically ignores irrelevant prose and isolates numerical operands/operators (e.g. `"Salary $5000 - $1200 tax"` evaluates to `$3800`).
5. **Inter-line Variable Referencing**:
   - Allows referencing previous lines explicitly using `lineN` (1-indexed) or implicitly using `ans` (e.g. `"line1 * 12"`, `"ans + 100"`).
6. **Unit & Dimensional Analysis**:
   - Native support for length, time, and mass dimensional conversions (e.g. `"300 meters + 2 kilometers in miles"`, `"3 hours 20 mins + 45 mins"`).
7. **Active Offline-First Currencies**:
   - Conversions using a local cache file (`~/.config/numen/currencies.json`) updated via background HTTP threads from a public exchange API on startup. Includes hardcoded offline fallbacks.
8. **Block-Height Sync**:
   - A custom-painted `ResultsCanvas` aligned perfectly with the text blocks of the `QPlainTextEdit` editor. If a line of text wraps, the calculation result remains aligned with the bottom of that text block.

---

## Tech Stack & Architecture

- **Core Engine (Rust)**:
  - **Pest**: PEG (Parsing Expression Grammar) parser to tokenize math grammar.
  - **Serde**: For currency JSON caching.
  - **PyO3 & Maturin**: Compiles the Rust engine into a native CPython extension module (`numen_engine`).
- **UI Layer (PySide6)**:
  - Built using Qt6 Python bindings to create a desktop GUI with a customized slate/violet accent dark theme.

---

## Grammar Parser Logic (Pest)

Prose and comments are isolated cleanly using ordered choices inside the [src/grammar.pest](src/grammar.pest) grammar definition:

```pest
// A line matches a sequence of expressions, comments, or prose words
line = { SOI ~ (expr | comment | prose)* ~ EOI }

// Comments start with # or // and consume until the end of the line
comment = @{ ("#" | "//") ~ (!EOI ~ ANY)* }

// Prose is any sequence of alphabetic/punctuation characters
prose = @{ (ALPHABETIC | PUNCTUATION | SYMBOL)+ }
```

Evaluating `expr` first ensures that mathematical formulas are eagerly matched. If a word or block does not form valid math, it backtracks and matches as `comment` or `prose`. In `eval.rs`, undefined variables resolve to `ResultValue::Empty`, which is skipped during line summation (allowing prose to pass silently) but triggers a type error if used inside binary operations (e.g. `NoahYears2Go + LaraYears2Goo`).

---

## Directory Layout

```
numen/
├── Cargo.toml          # Rust configuration
├── pyproject.toml      # Maturin project metadata
├── README.md           # This documentation file
├── AGENT.md            # Developer handover/architectural documentation
├── numen_gui.py        # PySide6 desktop interface
└── src/
    ├── lib.rs          # PyO3 module bindings & unit tests
    ├── grammar.pest    # Pest parser grammar
    ├── parser.rs       # Grammar parsing and AST building
    ├── eval.rs         # Mathematical evaluation and variable solver
    ├── units.rs        # Length/Time/Mass conversion registry
    └── currency.rs     # Exchange rate cached conversions
```

---

## How to Build and Run

### 1. Setup Virtual Environment
```bash
python3 -m venv .venv
source .venv/bin/activate
pip install PySide6 maturin
```

### 2. Compile the Rust Engine (Development)
Build and install the compiled Rust dynamic library as an editable Python package inside your virtual environment:
```bash
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop
```

### 3. Run the App
```bash
python numen_gui.py
```

### 4. Run Unit Tests
Run the Rust engine parser and evaluation unit tests:
```bash
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 cargo test
```

---

## Distribution Packaging

### Build Python Wheel (.whl)
To compile the Rust engine in release mode (fully optimized) and bundle it into a distributable Python wheel package:
```bash
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin build --release
```
The wheel will be created under `target/wheels/`.

### Package Standalone Desktop Binary (PyInstaller)
To package the Python runtime, PySide6 GUI, and compiled Rust engine into a **single standalone Linux binary executable** that can run on any compatible system without Python/Rust/Qt6 dependencies:

1. Install PyInstaller in the virtual environment:
   ```bash
   pip install pyinstaller
   ```

2. Compile the executable:
   ```bash
   pyinstaller --onefile --windowed --name numen numen_gui.py
   ```

3. Locate the binary:
   The compiled standalone binary is placed in:
   ```
   dist/numen
   ```
   To run it directly:
   ```bash
   ./dist/numen
   ```

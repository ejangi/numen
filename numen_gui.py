import sys
import os
import json
import urllib.request
import threading
from PySide6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QHBoxLayout, QVBoxLayout,
    QPlainTextEdit, QPushButton, QLabel, QSplitter, QFrame, QToolTip,
    QListWidget, QListWidgetItem, QInputDialog, QMenu, QMessageBox
)
from PySide6.QtGui import (
    QPainter, QColor, QFont, QPen, QTextCursor, QTextBlockFormat, QIcon, QFontDatabase
)
from PySide6.QtCore import (
    Qt, QRectF, QTimer, QThread, Signal, QObject, QPoint, QEvent
)

# Try to import the compiled Rust engine
try:
    import numen_engine
except ImportError:
    print("Warning: numen_engine Rust module not found in python path. Please compile using Maturin.")

# Asynchronous worker to execute calculations in Rust
class EvaluationWorker(QThread):
    finished = Signal(list, int)

    def __init__(self, lines, cache_path, gen_id):
        super().__init__()
        self.lines = lines
        self.cache_path = cache_path
        self.gen_id = gen_id

    def run(self):
        try:
            # Execute Rust module evaluation
            results = numen_engine.evaluate(self.lines, self.cache_path)
            self.finished.emit(results, self.gen_id)
        except Exception as e:
            # Emit error messages for all lines if execution fails
            self.finished.emit([f"Error: {str(e)}"] * len(self.lines), self.gen_id)


# Custom painted widget that renders result strings aligned with editor blocks
class ResultsCanvas(QWidget):
    def __init__(self, notepad, parent=None):
        super().__init__(parent)
        self.notepad = notepad
        self.results = []
        self.setMinimumWidth(180)
        self.setMouseTracking(True)

        # Harmonized premium palette (violet accent and slate colors)
        self.bg_color = QColor("#0f0f11")
        self.text_color = QColor("#a78bfa")     # Soft Violet
        self.error_color = QColor("#f87171")    # Soft Red
        self.line_color = QColor("#27272a")     # Border color

        # Connect notepad scrollbar and update events to repaint this canvas
        self.notepad.verticalScrollBar().valueChanged.connect(self.update)
        self.notepad.textChanged.connect(self.update)
        self.notepad.cursorPositionChanged.connect(self.update)

    def set_results(self, results):
        self.results = results
        self.update()

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.fillRect(event.rect(), self.bg_color)

        # Draw left border line
        painter.setPen(QPen(self.line_color, 1))
        painter.drawLine(0, 0, 0, self.height())

        # Set up matching typography
        font = self.notepad.font()
        painter.setFont(font)

        block = self.notepad.firstVisibleBlock()
        content_offset = self.notepad.contentOffset()
        margin = self.notepad.document().documentMargin()

        while block.isValid():
            block_number = block.blockNumber()
            if block.isVisible():
                block_geometry = self.notepad.blockBoundingGeometry(block)
                top = block_geometry.translated(content_offset).top()
                height = block_geometry.height()

                # Only draw if block is visible within view boundary
                if top + height >= 0 and top <= self.height():
                    if block_number < len(self.results):
                        result = self.results[block_number]
                        if result:
                            # Highlight errors vs values
                            if result.startswith("Error:"):
                                painter.setPen(self.error_color)
                                display_text = "Error ⚠️"
                            else:
                                painter.setPen(self.text_color)
                                display_text = result

                            # Get baseline of the last line of the block
                            text_layout = block.layout()
                            line_count = text_layout.lineCount()
                            if line_count > 0:
                                last_line = text_layout.lineAt(line_count - 1)
                                baseline_y = top + last_line.position().y() + last_line.ascent()
                                
                                # Right-align by measuring text width
                                font_metrics = painter.fontMetrics()
                                text_width = font_metrics.horizontalAdvance(display_text)
                                x = self.width() - 25 - text_width
                                
                                painter.drawText(QPoint(int(x), int(baseline_y)), display_text)

            block = block.next()

    def mouseMoveEvent(self, event):
        # Hover tooltip for errors
        pos = event.position().toPoint()
        block = self.notepad.firstVisibleBlock()
        content_offset = self.notepad.contentOffset()
        margin = self.notepad.document().documentMargin()

        while block.isValid():
            if block.isVisible():
                block_geometry = self.notepad.blockBoundingGeometry(block)
                top = block_geometry.translated(content_offset).top() + margin
                height = block_geometry.height()

                if top <= pos.y() <= top + height:
                    block_num = block.blockNumber()
                    if block_num < len(self.results):
                        result = self.results[block_num]
                        if result.startswith("Error:"):
                            # Strip "Error:" prefix for cleaner tooltips
                            clean_error = result.replace("Error: ", "").strip()
                            QToolTip.showText(event.globalPosition().toPoint(), clean_error, self)
                            return
            block = block.next()
        QToolTip.hideText()


class NumenWindow(QMainWindow):
    # Safe signals for updating UI from background thread fetches
    rates_status_signal = Signal(str, bool)

    def __init__(self):
        super().__init__()
        self.setWindowTitle("Numen — Natural Language Notepad Calculator")
        self.resize(1024, 768)

        # Set Config paths
        self.config_dir = os.path.expanduser("~/.config/numen")
        self.cache_path = os.path.join(self.config_dir, "currencies.json")

        # Document folder
        self.docs_dir = os.path.expanduser("~/Documents/numen")
        os.makedirs(self.docs_dir, exist_ok=True)
        self.current_doc_path = None
        self.is_loading_doc = False

        self.generation_id = 0
        self.workers = set()
        self.results = []

        # Setup main layouts
        self.setup_ui()
        self.apply_premium_styles()

        # Connect signals
        self.notepad.textChanged.connect(self.on_text_changed)
        self.rates_status_signal.connect(self.on_rates_status_updated)

        # Load documents list and load first doc (or default Welcome.md)
        docs = self.load_document_list()
        if docs:
            self.doc_list.setCurrentRow(0)
            self.load_document(docs[0][1])
        else:
            welcome_path = os.path.join(self.docs_dir, "Welcome.md")
            try:
                with open(welcome_path, "w") as f:
                    f.write(self.get_welcome_template_text())
            except Exception as e:
                print(f"Failed to create default welcome doc: {e}")
            self.load_document_list()
            self.doc_list.setCurrentRow(0)
            self.load_document(welcome_path)

        # Trigger background currency fetch at startup
        self.fetch_currencies()

        # Keyboard shortcuts for UI Zooming
        self.setup_zoom_shortcuts()

    def setup_zoom_shortcuts(self):
        from PySide6.QtGui import QKeySequence, QShortcut

        # Ctrl + Plus, Ctrl + Shift + Plus, and Ctrl + Equal for Zoom In
        self.shortcut_zoom_in = QShortcut(QKeySequence("Ctrl++"), self)
        self.shortcut_zoom_in.activated.connect(self.zoom_in)
        self.shortcut_zoom_in_shift = QShortcut(QKeySequence("Ctrl+Shift+="), self)
        self.shortcut_zoom_in_shift.activated.connect(self.zoom_in)
        self.shortcut_zoom_in_equal = QShortcut(QKeySequence("Ctrl+="), self)
        self.shortcut_zoom_in_equal.activated.connect(self.zoom_in)

        # Ctrl + Minus and Ctrl + Underscore for Zoom Out
        self.shortcut_zoom_out = QShortcut(QKeySequence("Ctrl+-"), self)
        self.shortcut_zoom_out.activated.connect(self.zoom_out)
        self.shortcut_zoom_out_shift = QShortcut(QKeySequence("Ctrl+_"), self)
        self.shortcut_zoom_out_shift.activated.connect(self.zoom_out)

        # Ctrl + 0 to Reset Zoom
        self.shortcut_zoom_reset = QShortcut(QKeySequence("Ctrl+0"), self)
        self.shortcut_zoom_reset.activated.connect(self.zoom_reset)

        # Install event filter to capture Ctrl + Mouse Wheel zooming
        self.notepad.installEventFilter(self)

    def eventFilter(self, obj, event):
        if obj == self.notepad and event.type() == QEvent.Wheel:
            if event.modifiers() & Qt.ControlModifier:
                if event.angleDelta().y() > 0:
                    self.zoom_in()
                else:
                    self.zoom_out()
                return True # Consume the event
        return super().eventFilter(obj, event)

    def zoom_in(self):
        self.notepad.zoomIn(1)
        self.results_canvas.update()

    def zoom_out(self):
        self.notepad.zoomOut(1)
        self.results_canvas.update()

    def zoom_reset(self):
        font = self.notepad.font()
        font.setPointSizeF(12.0)
        self.notepad.setFont(font)

        block_format = QTextBlockFormat()
        block_format.setLineHeight(145.0, QTextBlockFormat.ProportionalHeight.value)
        cursor = self.notepad.textCursor()
        cursor.select(QTextCursor.Document)
        cursor.setBlockFormat(block_format)

        self.results_canvas.update()

    def setup_ui(self):
        central_widget = QWidget(self)
        self.setCentralWidget(central_widget)

        main_layout = QVBoxLayout(central_widget)
        main_layout.setContentsMargins(0, 0, 0, 0)
        main_layout.setSpacing(0)

        # Custom header bar
        header = QFrame(self)
        header.setObjectName("header")
        header.setFixedHeight(64)
        header_layout = QHBoxLayout(header)
        header_layout.setContentsMargins(20, 0, 20, 0)

        title = QLabel("NUMEN", header)
        title.setObjectName("title")
        title.setFont(QFont("Outfit", 16, QFont.Bold))
        header_layout.addWidget(title)

        header_layout.addStretch()

        # Action buttons removed for a clean, minimalist layout

        main_layout.addWidget(header)

        # Split pane (Drawer, Notepad, and results)
        splitter = QSplitter(Qt.Horizontal, self)
        splitter.setObjectName("splitter")

        # Drawer panel (left pane)
        self.drawer = QFrame(self)
        self.drawer.setObjectName("drawer")
        self.drawer.setFixedWidth(220)
        drawer_layout = QVBoxLayout(self.drawer)
        drawer_layout.setContentsMargins(12, 16, 12, 16)
        drawer_layout.setSpacing(12)

        self.btn_new_doc = QPushButton("+ New Document", self.drawer)
        self.btn_new_doc.setObjectName("btn_new_doc")
        self.btn_new_doc.clicked.connect(self.on_new_document_clicked)

        self.doc_list = QListWidget(self.drawer)
        self.doc_list.setObjectName("doc_list")
        self.doc_list.itemClicked.connect(self.on_document_selected)
        self.doc_list.setContextMenuPolicy(Qt.CustomContextMenu)
        self.doc_list.customContextMenuRequested.connect(self.show_context_menu)

        drawer_layout.addWidget(self.btn_new_doc)
        drawer_layout.addWidget(self.doc_list)

        # Notepad editor (middle pane)
        self.notepad = QPlainTextEdit(self)
        self.notepad.setObjectName("notepad")
        self.notepad.setLineWrapMode(QPlainTextEdit.WidgetWidth)
        self.notepad.document().setDocumentMargin(15)

        # Set JetBrains Mono / Hack / Monospace
        font = QFont()
        font.setFamilies(["JetBrains Mono", "Fira Code", "DejaVu Sans Mono", "monospace"])
        font.setPointSizeF(12.0)
        self.notepad.setFont(font)
        
        # Set Line Height/Spacing to 140%
        block_format = QTextBlockFormat()
        block_format.setLineHeight(145.0, QTextBlockFormat.ProportionalHeight.value)
        self.notepad.textCursor().setBlockFormat(block_format)

        # Custom-painted evaluation results canvas (right pane)
        self.results_canvas = ResultsCanvas(self.notepad, self)
        self.results_canvas.setObjectName("results_canvas")

        splitter.addWidget(self.drawer)
        splitter.addWidget(self.notepad)
        splitter.addWidget(self.results_canvas)
        splitter.setStretchFactor(0, 0)
        splitter.setStretchFactor(1, 3)
        splitter.setStretchFactor(2, 1)

        main_layout.addWidget(splitter)

        # Debounce timer
        self.debounce_timer = QTimer(self)
        self.debounce_timer.setSingleShot(True)
        self.debounce_timer.timeout.connect(self.trigger_evaluation)

        # Check existing exchange rate age removed

    def apply_premium_styles(self):
        # CSS Styling for a dark theme with glassmorphism/harmonic colors
        self.setStyleSheet("""
            QMainWindow {
                background-color: #0b0b0d;
            }
            #header {
                background-color: #0f0f12;
                border-bottom: 1px solid #1f1f23;
            }
            #title {
                color: #ffffff;
                letter-spacing: 2px;
            }

            QPushButton {
                background-color: #1c1c21;
                color: #e4e4e7;
                border: 1px solid #2e2e35;
                padding: 6px 14px;
                border-radius: 6px;
                font-family: 'Inter', sans-serif;
                font-size: 10pt;
                font-weight: 500;
            }
            QPushButton:hover {
                background-color: #27272a;
                border-color: #3f3f46;
            }
            QPushButton:pressed {
                background-color: #18181b;
            }
            #drawer {
                background-color: #0d0d10;
                border-right: 1px solid #1c1c21;
            }
            #btn_new_doc {
                background-color: #5b21b6;
                border-color: #4c1d95;
                color: #f5f3ff;
                font-weight: 600;
                padding: 8px 12px;
            }
            #btn_new_doc:hover {
                background-color: #6d28d9;
                border-color: #5b21b6;
            }
            #btn_new_doc:pressed {
                background-color: #4c1d95;
            }
            QListWidget {
                background-color: transparent;
                border: none;
                color: #a1a1aa;
                font-family: 'Inter', sans-serif;
                font-size: 10pt;
            }
            QListWidget::item {
                padding: 8px 12px;
                border-radius: 6px;
                margin-bottom: 2px;
            }
            QListWidget::item:hover {
                background-color: #1c1c21;
                color: #f4f4f5;
            }
            QListWidget::item:selected {
                background-color: #2e1065;
                color: #d8b4fe;
                font-weight: 600;
            }
            #splitter::handle {
                background-color: #1f1f23;
            }
            QPlainTextEdit {
                background-color: #0b0b0d;
                color: #e4e4e7;
                border: none;
                selection-background-color: #3f3f46;
            }
            QToolTip {
                background-color: #18181b;
                color: #f87171;
                border: 1px solid #2e2e35;
                border-radius: 4px;
                padding: 6px;
                font-family: 'Inter', sans-serif;
                font-size: 10pt;
            }
            QMenu {
                background-color: #1c1c21;
                color: #e4e4e7;
                border: 1px solid #2e2e35;
                border-radius: 6px;
                padding: 4px;
            }
            QMenu::item {
                padding: 6px 20px;
                border-radius: 4px;
                font-family: 'Inter', sans-serif;
                font-size: 10pt;
            }
            QMenu::item:selected {
                background-color: #2e1065;
                color: #d8b4fe;
            }
        """)

    def get_welcome_template_text(self):
        return (
            "# Numen — Natural Language Notepad Calculator\n\n"
            "# 1. Natural Language Math\n"
            "Salary $5000 - $1200 taxes\n"
            "Health insurance: $350\n"
            "Rent: $1500\n"
            "Total monthly expenses = line5 + line6\n"
            "Remaining disposable savings = line4 - line7\n\n"
            "# 2. Percentages & Increments\n"
            "Original investment: $10000\n"
            "Profit: 15% of line11\n"
            "Portfolio value = line11 + 15%\n\n"
            "# 3. Units & Dimensional Conversions\n"
            "We walked 3.5 miles in meters\n"
            "Then another 2 kilometers\n"
            "Total distance in miles: line16 + line17 in miles\n"
            "Time spent: 1 hour 45 mins + 30 mins\n\n"
            "# 4. Active Currencies (Multi-Currency Support)\n"
            "Hotel booking: €450 in USD\n"
            "Train tickets: 12000 JPY in USD\n"
            "Total trip cost = line22 + line23\n"
        )

    def load_welcome_template(self):
        demo_text = self.get_welcome_template_text()
        self.notepad.setPlainText(demo_text)

        # Re-apply line height formatting
        block_format = QTextBlockFormat()
        block_format.setLineHeight(145.0, QTextBlockFormat.ProportionalHeight.value)
        cursor = self.notepad.textCursor()
        cursor.select(QTextCursor.Document)
        cursor.setBlockFormat(block_format)

    def load_document_list(self):
        self.doc_list.clear()
        
        docs = []
        if os.path.exists(self.docs_dir):
            for f in os.listdir(self.docs_dir):
                if f.endswith(".md"):
                    path = os.path.join(self.docs_dir, f)
                    mtime = os.path.getmtime(path)
                    docs.append((f, path, mtime))
                    
        docs.sort(key=lambda x: x[2], reverse=True)
        
        for name, path, _ in docs:
            item = QListWidgetItem(name[:-3]) # Strip ".md"
            item.setData(Qt.UserRole, path)
            self.doc_list.addItem(item)
            
        return docs

    def on_document_selected(self, item):
        path = item.data(Qt.UserRole)
        self.load_document(path)

    def load_document(self, path):
        if not os.path.exists(path):
            return
            
        self.is_loading_doc = True
        self.current_doc_path = path
        
        try:
            with open(path, "r") as f:
                content = f.read()
        except Exception as e:
            content = f"# Error\n\nFailed to load document: {e}"
            
        self.notepad.setPlainText(content)
        self.is_loading_doc = False
        
        # Re-apply line height formatting
        block_format = QTextBlockFormat()
        block_format.setLineHeight(145.0, QTextBlockFormat.ProportionalHeight.value)
        cursor = self.notepad.textCursor()
        cursor.select(QTextCursor.Document)
        cursor.setBlockFormat(block_format)
        
        self.trigger_evaluation()

    def on_new_document_clicked(self):
        name, ok = QInputDialog.getText(self, "New Document", "Document Name:")
        if ok and name.strip():
            filename = name.strip()
            if filename.endswith(".md"):
                filename = filename[:-3]
            filename = filename.replace("/", "_").replace("\\", "_")
            filename += ".md"
            
            filepath = os.path.join(self.docs_dir, filename)
            
            try:
                with open(filepath, "w") as f:
                    f.write(f"# {name.strip()}\n\n")
            except Exception as e:
                print(f"Failed to create new document file: {e}")
                return
                
            self.load_document_list()
            
            # Select the newly created document
            for i in range(self.doc_list.count()):
                item = self.doc_list.item(i)
                if item.data(Qt.UserRole) == filepath:
                    self.doc_list.setCurrentItem(item)
                    self.load_document(filepath)
                    break

    def show_context_menu(self, pos):
        item = self.doc_list.itemAt(pos)
        if item:
            menu = QMenu(self)
            rename_action = menu.addAction("Rename")
            delete_action = menu.addAction("Delete")
            
            action = menu.exec(self.doc_list.mapToGlobal(pos))
            if action == rename_action:
                self.rename_document(item)
            elif action == delete_action:
                self.delete_document(item)

    def rename_document(self, item):
        old_path = item.data(Qt.UserRole)
        old_name = item.text()
        
        new_name, ok = QInputDialog.getText(self, "Rename Document", "New name:", text=old_name)
        if ok and new_name.strip():
            new_filename = new_name.strip()
            if new_filename.endswith(".md"):
                new_filename = new_filename[:-3]
            new_filename = new_filename.replace("/", "_").replace("\\", "_")
            new_filename += ".md"
            
            new_path = os.path.join(self.docs_dir, new_filename)
            if new_path == old_path:
                return
                
            if os.path.exists(new_path):
                QMessageBox.warning(self, "Rename Document", "A document with this name already exists.")
                return
                
            try:
                os.rename(old_path, new_path)
                if self.current_doc_path == old_path:
                    self.current_doc_path = new_path
                    
                self.load_document_list()
                
                # Reselect the renamed item in list
                for i in range(self.doc_list.count()):
                    li = self.doc_list.item(i)
                    if li.data(Qt.UserRole) == new_path:
                        self.doc_list.setCurrentItem(li)
                        break
            except Exception as e:
                QMessageBox.critical(self, "Rename Document", f"Failed to rename file: {e}")

    def delete_document(self, item):
        path = item.data(Qt.UserRole)
        filename = item.text()
        
        reply = QMessageBox.question(
            self,
            "Delete Document",
            f"Are you sure you want to permanently delete '{filename}'?",
            QMessageBox.Yes | QMessageBox.No,
            QMessageBox.No
        )
        
        if reply == QMessageBox.Yes:
            try:
                os.remove(path)
                if self.current_doc_path == path:
                    self.current_doc_path = None
                    self.is_loading_doc = True
                    self.notepad.setPlainText("")
                    self.is_loading_doc = False
                    
                docs = self.load_document_list()
                if docs:
                    self.doc_list.setCurrentRow(0)
                    self.load_document(docs[0][1])
                else:
                    self.results = []
                    self.results_canvas.set_results([])
            except Exception as e:
                QMessageBox.critical(self, "Delete Document", f"Failed to delete file: {e}")

    def clear_notepad(self):
        self.notepad.setPlainText("")

    def on_text_changed(self):
        # Debounce the keystroke so we don't compile/evaluate on every tiny keyup
        self.debounce_timer.start(30)

    def trigger_evaluation(self):
        # Auto-save current document back to markdown file
        if self.current_doc_path and not self.is_loading_doc:
            content = self.notepad.toPlainText()
            try:
                with open(self.current_doc_path, "w") as f:
                    f.write(content)
            except Exception as e:
                print(f"Failed to auto-save document: {e}")

        lines = self.notepad.toPlainText().split('\n')
        self.generation_id += 1

        # Use our PySide thread worker to run Rust evaluations asynchronously
        worker = EvaluationWorker(lines, self.cache_path, self.generation_id)
        worker.finished.connect(self.on_evaluation_finished)
        self.workers.add(worker)
        worker.start()

    def on_evaluation_finished(self, results, gen_id):
        # Clean up the worker from the active set
        sender = self.sender()
        if sender in self.workers:
            self.workers.remove(sender)

        # Ensure we only use results matching the latest state request
        if gen_id == self.generation_id:
            self.results = results
            self.results_canvas.set_results(results)

    def fetch_currencies(self):
        def run_fetch():
            try:
                url = "https://open.er-api.com/v6/latest/USD"
                req = urllib.request.Request(url, headers={'User-Agent': 'NumenCalculator'})
                with urllib.request.urlopen(req, timeout=8) as response:
                    data = json.loads(response.read().decode())
                    if "rates" in data:
                        os.makedirs(self.config_dir, exist_ok=True)
                        with open(self.cache_path, "w") as f:
                            json.dump(data, f)
                        self.rates_status_signal.emit("Rates updated successfully!", True)
                    else:
                        self.rates_status_signal.emit("Failed: invalid API format", False)
            except Exception as e:
                self.rates_status_signal.emit(f"Fetch failed: {str(e)}", False)

        threading.Thread(target=run_fetch, daemon=True).start()

    def on_rates_status_updated(self, message, success):
        if success:
            # Trigger full recalculation since rates changed
            self.trigger_evaluation()


def main():
    app = QApplication(sys.argv)
    
    window = NumenWindow()
    window.show()
    sys.exit(app.exec())

if __name__ == "__main__":
    main()

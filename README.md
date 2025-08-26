# Screen Capture and Analysis

Demo utility for high performance screen capture and analysis. Integrates with local AI models for Optical Character Recognition (OCR) and image analysis, turning a simple screenshot utility into a potent information extraction tool.

This tool is built in Rust and leverages the `scap` library for screen capture, `ocrs` for text recognition, and can connect to a local [LM Studio](https://lmstudio.ai/) instance for advanced image analysis with vision-capable Large Language Models (LLMs).

## Features

- **List Screens & Windows**: Enumerate all available displays and windows, providing IDs and other metadata.
- **Screen Capture**: Capture a full screenshot of a specific display.
- **Window Capture**: Capture a specific application window.
- **OCR Text Extraction**: Extract text from any captured image using the `--ocr` flag.
- **LLM Image Analysis**: Send captures to a local LLM for detailed analysis using the `--analyze` flag.
- **Custom Prompts**: Guide the LLM's analysis with your own `--prompt`.
- **Flexible Output**: Save captures to a file or process them directly.

## Prerequisites

- **Rust**: Ensure you have a recent version of Rust and Cargo installed. You can get it from [rust-lang.org](https://www.rust-lang.org).
- **macOS**: This tool uses macOS-specific APIs for window and screen information, so it is not cross-platform.
- **Screen Recording Permissions**: You will need to grant your terminal application screen recording permissions in `System Settings > Privacy & Security > Screen Recording`. `captest` will prompt you on first run if it doesn't have permissions.

### Optional Prerequisites

- **LM Studio**: For the `--analyze` feature, you need [LM Studio](https://lmstudio.ai/) running with a vision-compatible model (e.g., LLaVA) loaded and the server started.
- **OCR Models**: For the `--ocr` feature, you need to have the `ocrs` model files. You can download them by cloning the `ocrs` repository and running the download script:
  ```bash
  # In a separate directory
  git clone https://github.com/robertknight/ocrs.git
  cd ocrs/ocrs/examples
  ./download-models.sh
  ```
  Then, you will need to copy the `text-detection.rten` and `text-recognition.rten` files to the root of the `captest` project directory or specify their path.

## Installation & Building

1.  **Clone the repository**:
    ```bash
    git clone <repository_url>
    cd captest
    ```

2.  **Build the project**:
    `captest` has local path dependencies (`scap`, `ocrs`). Ensure they are located at `../scap` and `../ocrs/ocrs` relative to the `captest` directory, or update the paths in `Cargo.toml`.

    ```bash
    cargo build --release
    ```
    The executable will be available at `./target/release/captest`.

## Usage

The tool is operated via subcommands.

### List available targets

**List all displays:**
```bash
./target/release/captest list
```

**List all windows:**
This provides detailed information about open windows, including their index, ID, owner, and geometry.
```bash
./target/release/captest list-windows
```

### Capture a screen or window

**Capture the primary screen (screen 0) and save it:**
```bash
./target/release/captest capture 0 --output my_screenshot.jpg
```

**Capture a specific window (e.g., window 5) and save it:**
```bash
./target/release/captest capture-window 5 --output window_capture.jpg
```

### Analyze and Extract Information

**Capture a screen and extract text using OCR:**
```bash
./target/release/captest capture 0 --ocr
```
The captured text will be printed to the console.

**Capture a window and have a local LLM analyze it:**
Make sure your LM Studio server is running on `http://localhost:1234`.
```bash
./target/release/captest capture-window 3 --analyze
```

**Use a custom prompt for analysis:**
```bash
./target/release/captest capture-window 3 --analyze --prompt "What is the main color scheme of this UI?"
```

## How It Works

- **Capture**: `scap` is used to access the screen and window frame buffers.
- **Image Handling**: Captured frames (in BGRA format) are converted to RGB and then encoded as JPEG files.
- **OCR**: The RGB image data is fed into the `ocrs` engine, which detects text regions, groups them into lines, and recognizes the characters.
- **LLM Analysis**: The JPEG image is base64 encoded and sent to the LM Studio OpenAI-compatible API endpoint with a user-provided or default prompt.
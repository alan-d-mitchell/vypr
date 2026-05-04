# Vypr

Vypr is a Python syntax compatible interpreter with **strong type enforcement**.  

---

## 🚀 Current Status

Vypr features a custom stack based bytecode VM and is current capable of compiling and executing "complex",
Python like programs wiyh high performance

The language successfully supports gradual typing, variable type locking, f-strings, list comprehension and a
suite of standard string and list manipulation methods.

---

## 🛠️ Installation

### 🐧 Linux

#### Part 1: Prerequisites

1.  **Install Rust:** Run the installation command provided at [rustup.rs](https://rustup.rs/)

#### Part 2: Building and Installing

1.  **Clone the Repository:** Open your terminal and grab the source code:
    ```bash
    git clone [https://github.com/your-username/vypr.git](https://github.com/your-username/vypr.git)
    cd vypr
    ```
2.  **Build the Project:** Compile the interpreter and CLI in release mode:
    ```bash
    cargo build --release
    ```
3.  **System Path Configuration:** Create a symbolic link to make the `vypr` command globally accessible. (Ensure `~/.local/bin` is in your system `$PATH`):
    ```bash
    ln -s $(pwd)/target/release/vypr ~/.local/bin/vypr
    ```

🎉 **You're all set!** Write some scripts and test out the compiler `--emit` tools.

### 🪟 Windows

#### Part 1: Prerequisites

1.  **Install Rust:** Download and install the Rust toolchain from [rustup.rs](https://rustup.rs/).
2.  **Install C++ Build Tools:** Download the Visual Studio C++ Build Tools. During the installation process, ensure you select the "Desktop development with C++" workload.

#### Part 2: Building from Source

1.  **Clone the Repository:** Open your terminal or PowerShell and grab the source code:
    ```bash
    git clone [https://github.com/your-username/vypr.git](https://github.com/your-username/vypr.git)
    cd vypr
    ```
2.  **Build the Project:** Compile the interpreter in release mode using Cargo:
    ```bash
    cargo build --release
    ```

#### Part 3: System Path Configuration

To run the `vypr` command globally from any terminal, you need to create a symbolic link to your WindowsApps folder (which is already on your system PATH).

1.  Open a **new PowerShell window as Administrator**.
2.  Run the following command. Make sure you have `$env:USERPROFILE` set and replace the `path\to` with the path to your cloned repository:
    ```powershell
    New-Item -ItemType SymbolicLink `
      -Path "$env:USERPROFILE\AppData\Local\Microsoft\WindowsApps\vypr.exe" `
      -Target "$env:USERPROFILE\path\to\vypr\target\release\vypr.exe"
    ```

🎉 **You're all set!** Write some scripts and test out the compiler `--emit` tools.

# Vypr

Vypr is a high-performance, Python-syntax-compatible programming language powered by a custom stack-based Virtual Machine. It marries the clean, highly readable syntax of Python with the safety and predictability of **strong type enforcement**. 

If you love Python's aesthetic but want the structural safety of a compiled, typed language, Vypr is built for you.

---

## ✨ Key Features

Vypr is actively in development but already supports a robust suite of modern language features:

* **Gradual Typing:** This is Vypr's core philosophy. Unlike standard Python where a variable can silently mutate from an `int` to a `str`, Vypr variables are strictly "locked" to their type **ONLY** if they are annotated (e.g., `age: int = 20`). If you try to assign a string to `age` later, the VM will catch it. However, you can still opt into standard dynamic behavior omitting the type annotation.
* **Custom Bytecode VM:** Code doesn't just run; it is lowered through a sophisticated multi-pass compiler pipeline (AST -> High-Level IR -> Mid-Level IR) before being executed on a fast, custom stack-based Virtual Machine.
* **First-Class Quality of Life:** Native support for modern developer favorites, including **f-strings**, **list comprehensions**, and a suite of built-in string and list manipulation methods.
* **Native Modules:** Built-in module imports (like `time` and `canvas`), allowing you to write programs using a standard library.
* **Lexical Scoping & Global Hoisting:** Functions and global variables are intelligently hoisted and scoped, ensuring your code executes exactly how you structure it.

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
3.  **System Path Configuration:** Create a symbolic link to make the `vypr` command globally accessible. *(Ensure `~/.local/bin` is in your system `$PATH`)*:
    ```bash
    ln -s $(pwd)/target/release/vypr ~/.local/bin/vypr
    ```

---

### 🪟 Windows

#### Part 1: Prerequisites
1.  **Install Rust:** Download and install the Rust toolchain from [rustup.rs](https://rustup.rs/).
2.  **Install C++ Build Tools:** Download the Visual Studio C++ Build Tools. During the installation process, ensure you select the **"Desktop development with C++"** workload.

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
2.  Run the following command. Make sure you have `$env:USERPROFILE` set and replace `path\to` with the actual path to your cloned repository:
    ```powershell
    New-Item -ItemType SymbolicLink `
      -Path "$env:USERPROFILE\AppData\Local\Microsoft\WindowsApps\vypr.exe" `
      -Target "$env:USERPROFILE\path\to\vypr\target\release\vypr.exe"
    ```

🎉 **You're all set!** Write some scripts and test out the compiler tools (e.g., `vypr script.vypr`).

## Examples

Check the `examples/` folder for example vypr programs and explanations of the type system.

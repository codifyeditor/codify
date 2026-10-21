# Codify Native

Codify Native is a rebuilt, Windows-first, **native Win32 + Rust** code editor. It intentionally avoids Electron, a browser webview, React, and Monaco to keep baseline memory use low.

## Current features

- Native Windows window and controls with a dark, VS Code-inspired layout
- File explorer for the selected workspace (lists direct children; opens folders and files)
- Open file, open folder, save, save as, and keyboard shortcuts (`Ctrl+O`, `Ctrl+S`, `F5`)
- Lightweight editor and output panel
- Run files using installed runtimes where supported: Python, Node.js, TypeScript via `npx tsx`, Rust via `rustc`, C/C++ via GCC/G++, Java, Go, Ruby, PHP, PowerShell, batch/CMD, Bash, and Ceasy (`ceasy` on PATH)
- Extensions menu opens Open VSX in the default browser
- Optimized release profile (`opt-level = "z"`, LTO, stripped symbols)
- GitHub Actions builds both `codify.exe` and a Windows setup installer (`Codify-Setup.exe`)

## RAM goals

The engineering targets are **50 MB idle**, **100 MB normal editing**, and **400 MB for large workspaces**. These are targets, not measured claims: the app has not yet been compiled/profiled on Windows. Native controls avoid a bundled Chromium engine, but actual usage depends on Windows, document size, and any launched compiler/runtime. Running language servers and compilers can use more memory than the editor itself.

## What is intentionally not bundled

Codify does not bundle every compiler, runtime, debugger, or language server. Doing so would increase installer size and memory use. Execution uses tools installed on the user's PC. For example, install Ceasy and make sure the `ceasy` command is on `PATH` to run `.ce` and `.ceasy` files. The current editor is a lightweight text editor and does not yet have token-level syntax coloring, IntelliSense, debugging, or a VS Code-compatible extension host. The extensions menu links to Open VSX; it does not load extensions inside Codify yet.

## Build on Windows

Install the stable Rust toolchain from <https://rustup.rs>, then in this directory run:

```powershell
cargo build --release
```

The executable will be `target\release\codify.exe`.

To make the installer locally, install Inno Setup 6 and run:

```powershell
cargo build --release
iscc installer.iss
```

The installer is created at `Output\Codify-Setup.exe`.

## Build from GitHub

Push this project to a GitHub repository. The workflow `.github/workflows/build-windows.yml` runs on pushes and can attach both the app and installer to a release when you push a version tag such as `v0.2.0`. On the Actions page, download the `codify-windows-installer` artifact from a successful run.

## Security note

The Run command executes the active file with the local user's permissions. Only run code you trust.

## License

MIT

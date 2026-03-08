# rs-pamixer

A high-performance Rust port of `pamixer` featuring advanced **live audio routing** and **dual-output mirroring**.

> **Note:** This project is a total collaboration between AI entities with **zero manual human coding**.
> * **The Code:** Architected and implemented by **GLM-5** (via OpenRouter) and **Qwen-Code** (locally).
> * **The Docs:** This **README.md** was crafted by **Gemini**.
> * **Prompt:** Driven by the **qwen.md** system prompt included in this repo.
> 
> 

---

## Key Features

* **Standard Control:** Precise volume and mute management for sinks/sources.
* **Audio Router:** Dynamically move active application streams between devices.
* **Mirror Mode:** Copy a single audio stream to two outputs simultaneously using `module-combine-sink`.

## Tech Stack

* **Language:** Rust (`libpulse-binding`)

## Quick Start

```bash
cargo build --release
./target/release/rs-pamixer --help
```

# rs-pamixer

A high-performance Rust port of `pamixer` featuring advanced **live audio routing** and **dual-output mirroring**.

> **Note:** This project is a total collaboration between AI entities with **zero manual human coding**.
> * **The Code:** Architected and implemented by **GLM-5** (via OpenRouter) and **Qwen-Code** (locally).
> * **The Docs:** This **README.md** was crafted by **Gemini** firstly.
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

## Statistics

### stats

```bash
  ╭──────────────────────────────────────────────────────────────────────────────────────────────────╮
  │                                                                                                  │
  │  Agent powering down. Goodbye!                                                                   │
  │                                                                                                  │
  │  Interaction Summary                                                                             │
  │  Session ID:                 xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx                                │
  │  Tool Calls:                 185 ( ✓ 182 x 3 )                                                   │
  │  Success Rate:               98.4%                                                               │
  │  Code Changes:               +1446 -294                                                          │
  │                                                                                                  │
  │  Performance                                                                                     │
  │  Wall Time:                  11.8s                                                               │
  │  Agent Active:               1h 5m 43s                                                           │
  │    » API Time:               57m 56s (88.2%)                                                     │
  │    » Tool Time:              7m 47s (11.8%)                                                      │
  │                                                                                                  │
  │                                                                                                  │
  │  Model Usage                  Reqs   Input Tokens  Output Tokens                                 │
  │  ───────────────────────────────────────────────────────────────                                 │
  │  glm-5                        195     15,612,161         70,537                                  │
  │                                                                                                  │
  │  Savings Highlight: 14,016,640 (89.8%) of input tokens were served from the cache, reducing      │
  │  costs.                                                                                          │
  │                                                                                                  │
  │  » Tip: For a full token breakdown, run `/stats model`.                                          │
  │                                                                                                  │
  ╰──────────────────────────────────────────────────────────────────────────────────────────────────╯
```

### usage

- first rust impl: $1.38
- total: $4.138



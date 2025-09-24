# PrismaOS

**PrismaOS** is an experimental operating system written in Rust, built to explore a new model of how displays, windows, and applications interact. It is not another Linux distribution, nor a Windows clone. PrismaOS is a ground-up rethinking of what an OS should look like when parallelism, safety, and graphics performance are first-class design principles.

---

## 🎯 Goals

### 1. Per-Display Compositors

Each display should have its own compositor instance, capable of driving ultra-high-resolution panels independently. This makes it possible to scale to many displays without hitting single-threaded bottlenecks.

### 2. True Parallelism

The kernel, compositor, and userland services should avoid centralized event loops wherever possible. Work should be spread across all CPU cores, taking advantage of async and multithreading in every subsystem.

### 3. Exclusive Fullscreen, Without Compromise

Applications should be able to request true exclusive fullscreen access — but only for the display they target. Unlike existing systems, this won’t require all displays to reset or reload, and transitions should be seamless.

### 4. Safety With Rust

PrismaOS is written in Rust, with unsafe code reduced to the bare minimum required for hardware interaction. The goal is a system where panics are contained and recovery is possible, rather than leading to a total crash.

### 5. Clean UI Philosophy

The UI is envisioned as a balance between the fluid, animated experience of macOS and the clarity of Windows 11 — but built on a fresh, modern, multithreaded foundation that avoids legacy cruft.

### 6. Minimal Legacy, Maximum Clarity

PrismaOS does not aim to inherit the UNIX or Windows userland model. The only major legacy component intentionally supported is ELF as the binary format. Everything else is designed to be clean, modern, and purpose-built.

---

## 🧪 What PrismaOS Is (and Isn’t)

* **Is:** A research OS focused on threading, compositing, and graphics pipelines.
* **Isn’t:** A drop-in Linux replacement, a production-ready system, or a stable daily driver.

---

## 💡 Inspiration

* **macOS**: For its heavily multithreaded, GPU-accelerated compositor.
* **Windows**: For its fullscreen hijack model — but redesigned per-display.
* **Linux**: For its adoption of ELF as a standard binary format.

---

## 🚧 Status

PrismaOS is in active design and development. Expect rapid changes, instability, and missing features.

---

## 🤝 Contributing

Contributions are welcome from anyone interested in operating systems, graphics, or Rust systems programming. The focus is on experimenting with new models, not cloning existing ones.

---

## 📜 License

PrismaOS is licensed under MIT.
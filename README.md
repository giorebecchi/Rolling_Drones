# 🚁 Rolling Drones

> *A multi-threaded distributed systems simulator where packets fly through virtual skies*

## 🌐 Overview

Welcome to a cutting-edge simulation environment where **clients**, **servers**, and **drones** come together in a symphony of concurrent communication. This project demonstrates the power of Rust's threading model combined with real-time visualization to create a robust network topology simulator.

## 🚀 Quick Start

```bash
# Recommended way to run (optimized performance)
cargo run --release
```
```bash
# Run with specific features
cargo run --release --features <feature_name>
```

## 🎯 Features & Topologies

### `--features max`
The ultimate stress test! Leverages high-performance servers:
- 🔧 Default servers by **@X-baia**
- 🚀 Optional servers by **@malchioman** to have TextServers combined with MediaServers!

### `--features chat`
Deploy a chat ecosystem:
- 💬 **3 ChatClients** (individual threads)
- 🗨️ **1 ChatServer** facilitating real-time communication
- Perfect for testing concurrent message handling

### `--features web`
Spin up a web services topology:
- 🌐 **1 Web Browser** client
- 📄 **1 TextServer** - The intelligent metadata broker
- 🎬 **2 MediaServers** - Content delivery specialists
- Smart routing: TextServer analyzes requests and directs traffic to the appropriate MediaServer, and should find the best route!

### `--features full`
**The complete package!** Combines all topologies into one massive simulation:
- All chat components
- All web components
- Maximum concurrency demonstration

## 🏗️ Architecture Highlights

### 🧵 **Multi-Threading Mastery**
Every component runs in its own thread:
- **Clients**: Independent request generators
- **Servers**: Concurrent request handlers
- **Drones**: The packet delivery workforce
- **Simulation Controller**: The omniscient orchestrator

### 📡 **Communication Infrastructure**
- **crossbeam_channel**: High-performance message passing
- **Drone Network**: Virtual packet carriers threading through the topology
- **Lazy<Arc<RwLock>>**: Ensures thread-safe synchronization between the Simulation Controller and GUI

### 🎮 **Simulation Controller**
The brain of the operation:
- 🗺️ Maintains complete topology knowledge
- 📊 Tracks every packet in flight
- 🔄 Bridges backend simulation with frontend visualization
- 🔒 Thread-safe data sharing via `RwLock`

### 🎨 **Real-Time Visualization**
Powered by:
- **Bevy**: High-performance game engine for smooth rendering
- **bevy_egui**: Immediate mode GUI for statistics and controls
- Watch packets fly through your network in real-time!

## 💡 Why This Project?

This simulator pushes Rust's concurrency model to its limits while providing visual feedback of complex distributed systems behavior. Perfect for:
- Understanding distributed systems concepts
- Learning concurrent programming patterns
- Visualizing network topology behavior

---

*Built with 🦀 Rust | Visualized with 🎮 Bevy*

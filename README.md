<h1 align="center">🌌 celestial ✨</h1>
<h3 align="center">A path timing tool for NieR:Automata speedrunning</h3>

Thank you to [Woeful Wolf](https://github.com/WoefulWolf/) again for helping me with Rust and for providing the base components that make this tool work!

Fist release coming soon!

# Features
- Precice Timing of routes
- Automatic comparison and sorting of times
- 3D visualization with depth-buffer rendering

# Planned Features
Functionality:
- teleportation

Quality of Life:
- toggleable no-depth-buffer overlay

If you have an idea for a feature please feel free to message me on discord!\
I'm also happy to receive pull requests :)

# Installation
1. Download the [latest release](https://github.com/Hellbufl/celestial/releases)
2. Extract it in your game folder next to NieRAutomata.exe
3. Rename "celestial.dll" to one of the [Supported Files](#supported-files)

# Supported Files
| Game          | Working Proxies                           |
| ---           | ---                                       |
| NieR:Automata | `dxgi.dll`, `d3d11.dll`, `dinput8.dll`    |

<!---
# Usage
- Toggle the menu with the default keybind `HOME` (look up the other keybinds in the Config tab)
-->

### Comparisons (default)
- Create a Path Collection with the + button
- Activate collections with their record button
- Place a start and an end trigger
- Start recording a path by leaving the start trigger and finish by entering the end trigger
- The finished path will be added to all active collections
- Highlight a path by clicking on the time
- Use the buttons labeled "M" ("Mute") and "S" ("Solo") to hide / only show the selected path

After multiple recordings with the same set of triggers, the different paths will be sorted from fastest to slowest within their collection and colored on a gradient (default: green -> red) with the fastest being highlighted (default: gold).

### Separate Paths
- Activate "Direct Mode" in the Config tab
- The "Create Trigger" keybinds now directly start and end the recording respectively

<!---
# Known Issues
-->
<h1 align="center">🌌 celestial ✨</h1>
<h3 align="center">A path timing tool for NieR:Automata speedrunning</h3>

Thank you to [Woeful Wolf](https://github.com/WoefulWolf/) again for helping me with Rust and for providing the base components that make this tool work!

# Features
- Precice Timing of routes
- Automatic comparison and sorting of times
- 3D visualization with depth-buffer rendering

# Planned Features
UI:
- changing collection order
- moving paths between collections
- popup messages and generally more info in-app

Quality of Life:
- saving position of timer in config (it's really hard for some reason to get the window position)

If you have an idea for a feature please feel free to message me on discord!\
I'm also happy to receive pull requests :)

# Installation
1. Download the [latest release](https://github.com/Hellbufl/celestial/releases)
2. Put it in your game folder next to NieRAutomata.exe
3. Rename "celestial.dll" to one of the [Supported Files](#supported-files)

# Supported Files
| Game          | Working Proxies                           |
| ---           | ---                                       |
| NieR:Automata | `dxgi.dll`, `d3d11.dll`, `dinput8.dll`    |

# Usage
- Look up default keybinds in the config tab

### General Stuff
- Create a Path Collection with the "+" button
- Activate a collection with its record button
- Place a start and an end trigger
- Start recording a path by leaving the start trigger and finish by entering the end trigger
- The finished path will be added to the active collection
- Highlight a path by clicking on the time
- Use the buttons labeled "M" ("Mute") and "S" ("Solo") to hide / only show the selected path

After multiple recordings with the same set of triggers, the different paths will be sorted from fastest to slowest within their collection and colored on a gradient (default: green -> red) with the fastest being highlighted (default: gold).

### High Pass Filters
There are two types of filters marked by the little up-arrow.
If you activate the filter mode on a collection, that collection will only accept a new path if it is faster than all paths in that collection.
You can also set the filter on a specific time by right clicking on it. Now only paths faster than this time will be accepted and the filter will stay fixed.

<!--
### Separate Paths
- Activate "Direct Mode" in the Config tab
- The "Create Trigger" keybinds now directly start and end the recording respectively
-->

<!--
# Known Issues
-->

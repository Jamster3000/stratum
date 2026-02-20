# stratum
A block based voxel game. I created this to experiment with rust and my limited knowledge on it. Nothing much going on with it right now (but rust is becoming one of my more favourite language)

This is the start of a game called "Stratum" (not the board game). It's a block based game, similar workings to that of Minecraft, Infiniminer, Hytale, and so on (those three are the main inspiration and pain point focus). I've got a huge amount of ideas and concepts going on for this. I want something more realistic than the above games whilst keeping the blocky feel (remembering that `realism /= fun` is sometimes a real equation that has to be carefully balanced). Additionally, unlike games mentioned before, I would like this to be runable on some low end hardware and easier on multiplayer support and setup.

I will more closely document ideas in the wiki, that I have rather than blerting everything out here.

# How it works
There's currently nothing to this at all. Just an exe (see the release page) -> Run this, and it should immediately boot up into the "game" (pending on antivirus hatred for the exe). 

# What features does it have/techniques used to make this
Just to give you an idea of what different techniques I've used so far to make it work and as effecient as it is right now:

- **Atlas Textures**: There are 4 textures right now, on start up, if the atlas doesn't exist it will build it from all the textures present and create a position map in `atlas.ron`
- **Greedy Meshing**: Reduces the triangles effeciently turning them into larger quads. There is a two-pass run on this.
- **Frustum**: Chunks (blocks included) are only rendered in camera view, anything not in view of the camera isn't rendered.
- **LOD**: LOD is a new thing still to me (I thought it was something slightly different) but this is somewhat implemented.
- **Shader**: Because of how bevy (the game library) works, a shader was required to make greedy meshing and the textures work.

## features
There are some basic features to this so far

- **Block RON files**: blocks are configured in ron files (not programmically) and have a file watcher to make it easy to change block configurations at runtime.
- **Biome RON files**: Same thing for blocks, except right now the biomes aren't really being used (except the name)
- **Settings RON file**: There are settings to configure keybinds and basic performance related features (e.g., render distance - Each chunk is 32x32x32 btw).
- **Block interaction**: All blocks are the same, but can be broken and placed instantly (There's a bug where they can't be placed on the top y-level).
- **Atmosphere**: Currently using Bevy_atmosphere to make this atmosphere, then using shaders and directional light to light the world up (Plans to change this after upgrading newer Bevy versions).

# Terminal commands
- **Flamemgraph**: `cargo flamegraph --bin startum --profile flamegraph`
- **Generate Documentation**: `cargo doc --no-deps --open`
- **Generate Release**: `cargo run --release`
- **Generate Debug**: `cargo run --features bevy/dynamic_linking`
- **Clippy**: `cargo clippy -- -W clippy::pedantic`
- **LLVM IR**: `cargo llvm-lines --bin stratum | Select-String "stratum" | Sort-Object -Property Line -Descending | Out-File -FilePath "llvm_lines_sorted.txt"`
- **Benchmarking**: `cargo bench`

# Screenshots
Please note that nothing below is a final design or anything, just plans and draft ideas.

## Wireframe UI design drafts
### Start Menu
<img width="1917" height="1073" alt="image" src="https://github.com/user-attachments/assets/8e6ce31f-bde4-448b-b0bd-88f479435a20" />

## Play game Menu
<img width="1919" height="1078" alt="image" src="https://github.com/user-attachments/assets/db27a683-f28e-4db1-a677-7ade3784c2f0" />

## Game overlay (HUD)
<img width="1919" height="1076" alt="image" src="https://github.com/user-attachments/assets/56fceac6-a290-496c-8e5c-44be7eb9f2fc" />
> Note: I plan on changing these three vertical bars to be in segments (e.g., split each bar into 4-6 segments each with their own segments)

## Inventory (no storage)
<img width="1918" height="1066" alt="image" src="https://github.com/user-attachments/assets/f2d93f9e-df96-4f6f-81f3-20d1fac2bf62" />
> Note: This is a draft example of what it looks like when the player has no storage besides their own hands and pocket slots.

## Inventory (with storage)
<img width="1915" height="1073" alt="image" src="https://github.com/user-attachments/assets/bca27433-5dcd-4126-bfdc-05c6c513ce31" />
> Note: This is a draft example of what it looks like when the player IS equipped/wearing storage items (e.g., Backpack, Pouches, Satchels, quiver). These are just examples as a backpack and quiver will 100% not be possible to have on at the same time.

## Colours used
<img width="1915" height="1076" alt="image" src="https://github.com/user-attachments/assets/413f3b65-bbcd-436a-acce-7469076b5547" />

# Contribution
I very much welcome contributions from anyone. Your help can make a big difference in improving this project, and help me out a lot.

## How to Contribute
**Report Issues**: Found a bug or have a suggestion? Open an issue on the issue tracker and describe it clearly.

**Fix Bugs or Add Features**: Submit a pull request (PR) with your changes. Make sure to:
  - Follow existing code styles where possible
  - All files must have comment documentation at the top and each public function must also have it's own set of comment documentation.

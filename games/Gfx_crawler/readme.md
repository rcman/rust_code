This is a basic starting point for a graphical dungeon crawler in Rust using Bevy. It’s a simple 3D top-down/isometric view (you can adjust the camera for more first-person feel by changing the transform). The dungeon is randomly generated with a central room and some scattered floors. The player is a red sphere that moves with WASD, with basic collision against walls.
To run it:
1.  Create a new Rust project: cargo new dungeon_crawler
2.  Replace Cargo.toml with the above.
3.  Replace src/main.rs with the above.
4.  Run cargo run.
This isn’t a full Dungeon Master clone (no monsters, inventory, puzzles yet), but it’s a graphical foundation you can build on. Add more features like enemies (spawn entities with AI systems), items, or raycast for first-person rendering. For true first-person, consider Bevy’s FPS controller examples. Let me know if you want expansions!
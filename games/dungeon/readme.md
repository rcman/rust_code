This is a simple console-based dungeon crawler in Rust, inspired by Dungeon Master. It features:
•  Procedural Generation: A basic room with walls, a monster, and an exit.
•  Player Movement: Use w/a/s/d to move around the grid.
•  Combat: Bumping into a monster triggers a simple attack (player deals damage implicitly by “defeating” it, but takes damage in return).
•  Win/Lose: Reach ‘E’ to win, HP <=0 to lose.
•  Rendering: ASCII art map with player ‘@’.
To run it:
1.  Create a new Rust project: cargo new dungeon_crawler
2.  Replace src/main.rs with the code above.
3.  Add rand = "0.8.5" to Cargo.toml.
4.  Run cargo run.
This is a minimal version—expand it with inventory, multiple rooms, real-time elements (using threads), or graphics (e.g., via ggez crate) for a fuller Dungeon Master feel! If you’d like enhancements, let me know.
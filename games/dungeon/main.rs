// Cargo.toml
// [package]
// name = "dungeon_crawler"
// version = "0.1.0"
// edition = "2021"

// [dependencies]
// rand = "0.8.5"  // For random number generation (e.g., monster placement)

// main.rs
use rand::Rng;
use std::io::{self, Write};

// Tile types for the dungeon
#[derive(Clone, Copy, PartialEq)]
enum Tile {
    Wall,
    Floor,
    Door,
    Exit,
    Monster,
}

// Player struct
#[derive(Clone)]
struct Player {
    x: usize,
    y: usize,
    hp: i32,
    max_hp: i32,
    attack: i32,
}

// Game struct to hold state
struct Game {
    map: Vec<Vec<Tile>>,
    player: Player,
    width: usize,
    height: usize,
}

impl Game {
    // Create a new game with a simple procedural dungeon
    fn new(width: usize, height: usize) -> Self {
        let mut rng = rand::thread_rng();
        let mut map = vec![vec![Tile::Wall; width]; height];

        // Generate a simple room-based dungeon
        let room_x = 2;
        let room_y = 2;
        let room_w = width - 4;
        let room_h = height - 4;
        for y in room_y..room_y + room_h {
            for x in room_x..room_x + room_w {
                map[y][x] = Tile::Floor;
            }
        }

        // Add some walls inside for structure
        for _ in 0..5 {
            let wx = rng.gen_range(room_x + 1..room_x + room_w - 1);
            let wy = rng.gen_range(room_y + 1..room_y + room_h - 1);
            map[wy][wx] = Tile::Wall;
        }

        // Place exit
        let ex = room_x + room_w - 2;
        let ey = room_y + room_h - 2;
        map[ey][ex] = Tile::Exit;

        // Place a monster
        let mx = rng.gen_range(room_x + 1..room_x + room_w - 1);
        let my = rng.gen_range(room_y + 1..room_y + room_h - 1);
        if map[my][mx] == Tile::Floor {
            map[my][mx] = Tile::Monster;
        }

        // Place player
        let px = room_x + 1;
        let py = room_y + 1;
        map[py][px] = Tile::Floor;  // Ensure starting spot is floor

        let player = Player {
            x: px,
            y: py,
            hp: 100,
            max_hp: 100,
            attack: 20,
        };

        Self {
            map,
            player,
            width,
            height,
        }
    }

    // Render the map to console
    fn render(&self) {
        for y in 0..self.height {
            for x in 0..self.width {
                let tile = if x == self.player.x && y == self.player.y {
                    '@'  // Player
                } else {
                    match self.map[y][x] {
                        Tile::Wall => '#',
                        Tile::Floor => '.',
                        Tile::Door => '+',
                        Tile::Exit => 'E',
                        Tile::Monster => 'M',
                    }
                };
                print!("{}", tile);
            }
            println!();
        }
        println!("HP: {}/{}", self.player.hp, self.player.max_hp);
        println!("Commands: w/a/s/d to move, q to quit");
    }

    // Handle player move
    fn move_player(&mut self, dx: isize, dy: isize) -> bool {
        let new_x = self.player.x as isize + dx;
        let new_y = self.player.y as isize + dy;
        if new_x < 0 || new_x >= self.width as isize || new_y < 0 || new_y >= self.height as isize {
            return false;
        }
        let nx = new_x as usize;
        let ny = new_y as usize;
        match self.map[ny][nx] {
            Tile::Wall => false,
            Tile::Monster => {
                // Simple combat: player attacks monster
                self.player.hp -= 10;  // Monster hits back
                if self.player.hp <= 0 {
                    println!("You died!");
                    return true;  // Game over
                }
                println!("You attacked the monster! It hit back.");
                true  // Stay in place after combat
            }
            Tile::Exit => {
                println!("You found the exit! You win!");
                true  // Game over
            }
            _ => {
                self.player.x = nx;
                self.player.y = ny;
                true
            }
        }
    }

    // Run the game loop
    fn run(&mut self) {
        loop {
            self.render();
            print!("Enter command: ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim().to_lowercase();

            match input.as_str() {
                "w" => {
                    if !self.move_player(0, -1) {
                        println!("Can't go there!");
                    }
                }
                "s" => {
                    if !self.move_player(0, 1) {
                        println!("Can't go there!");
                    }
                }
                "a" => {
                    if !self.move_player(-1, 0) {
                        println!("Can't go there!");
                    }
                }
                "d" => {
                    if !self.move_player(1, 0) {
                        println!("Can't go there!");
                    }
                }
                "q" => {
                    println!("Thanks for playing!");
                    break;
                }
                _ => println!("Invalid command!"),
            }

            if self.player.hp <= 0 {
                break;
            }
        }
    }
}

fn main() {
    let mut game = Game::new(20, 10);
    println!("Welcome to Rust Dungeon Crawler - Inspired by Dungeon Master!");
    println!("Explore the dungeon, fight monsters, find the exit.");
    game.run();
}
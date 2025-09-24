use bevy::prelude::*;
use rand::Rng;

// Constants for the game
const DUNGEON_WIDTH: usize = 20;
const DUNGEON_HEIGHT: usize = 20;
const TILE_SIZE: f32 = 32.0;
const PLAYER_SPEED: f32 = 100.0;

// Tile types
#[derive(Clone, Copy, PartialEq)]
enum TileType {
    Wall,
    Floor,
    Door,
}

// Dungeon map: 2D vector of TileType
type DungeonMap = Vec<Vec<TileType>>;

// Components
#[derive(Component)]
struct Player;

#[derive(Component)]
struct Position {
    x: i32,
    y: i32,
}

#[derive(Component)]
struct Tile {
    tile_type: TileType,
}

// Resource for the dungeon map
#[derive(Resource)]
struct Dungeon {
    map: DungeonMap,
}

// Simple dungeon generator: random walls and floors, with a starting room
fn generate_dungeon(width: usize, height: usize) -> DungeonMap {
    let mut rng = rand::thread_rng();
    let mut map = vec![vec![TileType::Wall; width]; height];

    // Create a starting room in the center
    let center_x = width / 2;
    let center_y = height / 2;
    let room_size = 5;
    for y in (center_y - room_size / 2)..=(center_y + room_size / 2) {
        for x in (center_x - room_size / 2)..=(center_x + room_size / 2) {
            if x >= 0 && x < width && y >= 0 && y < height {
                map[y][x] = TileType::Floor;
            }
        }
    }

    // Add some random floors to simulate corridors
    for _ in 0..50 {
        let x = rng.gen_range(0..width);
        let y = rng.gen_range(0..height);
        if rng.gen_bool(0.1) {  // 10% chance to make floor
            map[y][x] = TileType::Floor;
        }
    }

    map
}

// Startup system: initialize dungeon, player, and camera
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Insert dungeon resource
    let dungeon = generate_dungeon(DUNGEON_WIDTH, DUNGEON_HEIGHT);
    commands.insert_resource(Dungeon { map: dungeon });

    // Spawn camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, DUNGEON_HEIGHT as f32 * TILE_SIZE, DUNGEON_WIDTH as f32 * TILE_SIZE)
            .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        ..default()
    });

    // Spawn light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // Spawn dungeon tiles
    let floor_handle = meshes.add(Cube::new(TILE_SIZE, 1.0, TILE_SIZE));
    let wall_handle = meshes.add(Cube::new(TILE_SIZE, TILE_SIZE * 2.0, TILE_SIZE));

    let floor_material = materials.add(StandardMaterial {
        base_color: Color::rgb(0.3, 0.2, 0.1),
        ..default()
    });

    let wall_material = materials.add(StandardMaterial {
        base_color: Color::rgb(0.5, 0.5, 0.5),
        ..default()
    });

    let dungeon = commands.insert_resource(Dungeon { map: generate_dungeon(DUNGEON_WIDTH, DUNGEON_HEIGHT) });
    let map = &dungeon.map;  // Note: In real code, access via query or something, but for simplicity

    for y in 0..DUNGEON_HEIGHT {
        for x in 0..DUNGEON_WIDTH {
            let tile_type = map[y][x];
            let mesh = match tile_type {
                TileType::Wall => wall_handle.clone(),
                _ => floor_handle.clone(),
            };
            let material = match tile_type {
                TileType::Wall => wall_material.clone(),
                _ => floor_material.clone(),
            };

            commands.spawn((
                PbrBundle {
                    mesh: mesh,
                    material: material,
                    transform: Transform::from_xyz(
                        x as f32 * TILE_SIZE - (DUNGEON_WIDTH as f32 * TILE_SIZE / 2.0),
                        if tile_type == TileType::Wall { TILE_SIZE / 2.0 } else { -0.5 },
                        y as f32 * TILE_SIZE - (DUNGEON_HEIGHT as f32 * TILE_SIZE / 2.0),
                    ),
                    ..default()
                },
                Tile { tile_type },
            ));
        }
    }

    // Spawn player as a simple sphere
    let player_mesh = meshes.add(Sphere::new(TILE_SIZE * 0.4));
    let player_material = materials.add(StandardMaterial {
        base_color: Color::rgb(1.0, 0.0, 0.0),
        emissive: Color::rgb(0.1, 0.0, 0.0),
        ..default()
    });

    commands.spawn((
        PbrBundle {
            mesh: player_mesh,
            material: player_material,
            transform: Transform::from_xyz(
                0.0 - (DUNGEON_WIDTH as f32 * TILE_SIZE / 2.0),
                TILE_SIZE * 0.5,
                0.0 - (DUNGEON_HEIGHT as f32 * TILE_SIZE / 2.0),
            ),
            ..default()
        },
        Player,
        Position { x: DUNGEON_WIDTH as i32 / 2, y: DUNGEON_HEIGHT as i32 / 2 },
    ));
}

// Player movement system (WASD keys for top-down movement, but camera is isometric-ish)
fn player_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut Position), With<Player>>,
    dungeon: Res<Dungeon>,
) {
    if let Ok((mut transform, mut position)) = player_query.get_single_mut() {
        let mut direction = Vec3::ZERO;

        if keys.pressed(KeyCode::KeyW) {
            direction.z -= 1.0;
        }
        if keys.pressed(KeyCode::KeyS) {
            direction.z += 1.0;
        }
        if keys.pressed(KeyCode::KeyA) {
            direction.x -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) {
            direction.x += 1.0;
        }

        if direction.length() > 0.0 {
            direction = direction.normalize();

            // Check if new position is floor
            let new_x = position.x as f32 + direction.x * (PLAYER_SPEED * time.delta_seconds() / TILE_SIZE);
            let new_y = position.z as f32 + direction.z * (PLAYER_SPEED * time.delta_seconds() / TILE_SIZE);  // Wait, transform.z is y in map

            // Simple collision: round to grid and check
            let grid_x = (new_x + (DUNGEON_WIDTH as f32 / 2.0)) / TILE_SIZE;
            let grid_y = (new_y + (DUNGEON_HEIGHT as f32 / 2.0)) / TILE_SIZE;

            if grid_x >= 0.0 && grid_x < DUNGEON_WIDTH as f32 &&
               grid_y >= 0.0 && grid_y < DUNGEON_HEIGHT as f32 {
                let ix = grid_x as usize;
                let iy = grid_y as usize;
                if dungeon.map[iy][ix] == TileType::Floor {
                    position.x = ix as i32;
                    position.y = iy as i32;
                }
            }

            transform.translation += direction * PLAYER_SPEED * time.delta_seconds();
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(Dungeon { map: generate_dungeon(DUNGEON_WIDTH, DUNGEON_HEIGHT) })  // Insert early
        .add_systems(Startup, setup)
        .add_systems(Update, player_movement)
        .run();
}
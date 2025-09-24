import pygame
import sys
import math
import random
from enum import Enum
from typing import List, Dict, Tuple, Optional

# Initialize Pygame
pygame.init()

# Constants
SCREEN_WIDTH = 1024
SCREEN_HEIGHT = 768
FPS = 60
TILE_SIZE = 32
MAP_WIDTH = SCREEN_WIDTH // TILE_SIZE
MAP_HEIGHT = SCREEN_HEIGHT // TILE_SIZE

# Colors
BLACK = (0, 0, 0)
WHITE = (255, 255, 255)
GREEN = (0, 255, 0)
BLUE = (0, 0, 255)
RED = (255, 0, 0)
YELLOW = (255, 255, 0)
BROWN = (139, 69, 19)
GRAY = (128, 128, 128)

# Enums
class Race(Enum):
    TERRAN = "Terran"
    ZERG = "Zerg"
    PROTOSS = "Protoss"

class UnitType(Enum):
    WORKER = "Worker"
    MARINE = "Marine"
    ZERG_LING = "Zergling"
    PROBE = "Probe"

class BuildingType(Enum):
    COMMAND_CENTER = "Command Center"
    SUPPLY_DEPOT = "Supply Depot"
    BARRACKS = "Barracks"
    HATCHERY = "Hatchery"
    NEXUS = "Nexus"

class ResourceType(Enum):
    MINERALS = "Minerals"
    GAS = "Gas"

class Order(Enum):
    MOVE = "Move"
    HARVEST = "Harvest"
    BUILD = "Build"
    ATTACK = "Attack"
    IDLE = "Idle"

# Simple resource class
class Resource:
    def __init__(self, pos: Tuple[int, int], amount: int = 50, resource_type: ResourceType = ResourceType.MINERALS):
        self.pos = pos
        self.amount = amount
        self.type = resource_type
        self.depleted = False

    def draw(self, screen):
        if not self.depleted:
            color = YELLOW if self.type == ResourceType.GAS else GREEN
            pygame.draw.rect(screen, color, (self.pos[0] * TILE_SIZE, self.pos[1] * TILE_SIZE, TILE_SIZE, TILE_SIZE))

# Base Unit class
class Unit:
    def __init__(self, unit_type: UnitType, pos: Tuple[int, int], player: 'Player', health: int = 100, speed: float = 2.0):
        self.type = unit_type
        self.pos = list(pos)
        self.target_pos = list(pos)
        self.health = health
        self.max_health = health
        self.speed = speed
        self.player = player
        self.order = Order.IDLE
        self.target = None
        self.size = 16  # Pixel size for drawing

    def update(self):
        if self.order == Order.MOVE and self.pos != self.target_pos:
            dx = self.target_pos[0] - self.pos[0]
            dy = self.target_pos[1] - self.pos[1]
            dist = math.hypot(dx, dy)
            if dist > self.speed:
                self.pos[0] += (dx / dist) * self.speed
                self.pos[1] += (dy / dist) * self.speed
            else:
                self.pos = self.target_pos[:]
                self.order = Order.IDLE

        # Simple attack logic if target is enemy unit
        if self.order == Order.ATTACK and self.target and isinstance(self.target, Unit) and self.target.health > 0:
            dist = math.hypot(self.pos[0] - self.target.pos[0], self.pos[1] - self.target.pos[1])
            if dist <= 50:  # Attack range
                self.target.health -= 10  # Damage
                if self.target.health <= 0:
                    self.target = None
                    self.order = Order.IDLE

    def draw(self, screen):
        color = self.player.color
        if self.type == UnitType.WORKER:
            pygame.draw.circle(screen, color, (int(self.pos[0]), int(self.pos[1])), self.size // 2)
        elif self.type == UnitType.MARINE:
            pygame.draw.rect(screen, color, (self.pos[0] - self.size//2, self.pos[1] - self.size//2, self.size, self.size))
        # Health bar
        bar_width = 20
        bar_height = 4
        health_ratio = self.health / self.max_health
        pygame.draw.rect(screen, RED, (self.pos[0] - bar_width//2, self.pos[1] - 10, bar_width, bar_height))
        pygame.draw.rect(screen, GREEN, (self.pos[0] - bar_width//2, self.pos[1] - 10, bar_width * health_ratio, bar_height))

    def issue_order(self, order: Order, target=None):
        self.order = order
        self.target = target
        if order == Order.MOVE:
            self.target_pos = list(target.pos if isinstance(target, (Unit, Building)) else target)

# Building class
class Building:
    def __init__(self, building_type: BuildingType, pos: Tuple[int, int], player: 'Player', health: int = 500):
        self.type = building_type
        self.pos = list(pos)
        self.health = health
        self.max_health = health
        self.player = player
        self.size = 64  # Larger size
        self.construction_time = 0  # For simplicity, instant

    def draw(self, screen):
        color = self.player.color
        rect = pygame.Rect(self.pos[0] - self.size//2, self.pos[1] - self.size//2, self.size, self.size)
        pygame.draw.rect(screen, color, rect)
        # Health bar
        bar_width = 40
        bar_height = 6
        health_ratio = self.health / self.max_health
        pygame.draw.rect(screen, RED, (self.pos[0] - bar_width//2, self.pos[1] - 20, bar_width, bar_height))
        pygame.draw.rect(screen, GREEN, (self.pos[0] - bar_width//2, self.pos[1] - 20, bar_width * health_ratio, bar_height))

    def can_build(self, unit_type: UnitType) -> bool:
        if self.type == BuildingType.BARRACKS and unit_type == UnitType.MARINE:
            return True
        # Add more logic for other races/buildings
        return False

    def train_unit(self, unit_type: UnitType):
        if self.can_build(unit_type) and self.player.minerals >= 50:  # Cost
            self.player.minerals -= 50
            spawn_pos = (self.pos[0] + random.randint(-20, 20), self.pos[1] + random.randint(-20, 20))
            new_unit = Unit(unit_type, spawn_pos, self.player)
            self.player.units.append(new_unit)
            return new_unit
        return None

# Player class
class Player:
    def __init__(self, name: str, race: Race, color: Tuple[int, int, int]):
        self.name = name
        self.race = race
        self.color = color
        self.minerals = 50
        self.gas = 0
        self.supply_used = 0
        self.supply_max = 10
        self.units: List[Unit] = []
        self.buildings: List[Building] = []
        self.resources: List[Resource] = []

    def add_resource(self, resource: Resource):
        self.resources.append(resource)

    def can_afford_supply(self, supply_cost: int) -> bool:
        return self.supply_used + supply_cost <= self.supply_max

    def update_supply(self, delta: int):
        self.supply_used += delta

# Simple map generation
def generate_map(resources: List[Resource]) -> List[List[int]]:
    map_tiles = [[random.choice([0, 1]) for _ in range(MAP_WIDTH)] for _ in range(MAP_HEIGHT)]  # 0: grass, 1: dirt
    # Place resources
    for res in resources:
        map_tiles[res.pos[1]][res.pos[0]] = 2  # Resource tile
    return map_tiles

def draw_map(screen, map_tiles: List[List[int]]):
    for y, row in enumerate(map_tiles):
        for x, tile in enumerate(row):
            color = GREEN if tile == 0 else BROWN if tile == 1 else None
            if color:
                pygame.draw.rect(screen, color, (x * TILE_SIZE, y * TILE_SIZE, TILE_SIZE, TILE_SIZE))

# Main Game class
class StarCraftClone:
    def __init__(self):
        self.screen = pygame.display.set_mode((SCREEN_WIDTH, SCREEN_HEIGHT))
        pygame.display.set_caption("StarCraft Clone - Simplified")
        self.clock = pygame.time.Clock()
        self.running = True

        # Players (single player for demo, Terran vs AI Zerg)
        self.player1 = Player("Human (Terran)", Race.TERRAN, BLUE)  # Human
        self.player2 = Player("AI (Zerg)", Race.ZERG, RED)  # AI

        # Initial buildings
        cc_pos = (5 * TILE_SIZE, 5 * TILE_SIZE)
        self.player1.buildings.append(Building(BuildingType.COMMAND_CENTER, (5, 5), self.player1))
        self.player2.buildings.append(Building(BuildingType.HATCHERY, (MAP_WIDTH - 5, MAP_HEIGHT - 5), self.player2))

        # Initial workers
        self.player1.units.append(Unit(UnitType.WORKER, (5, 6), self.player1))
        self.player2.units.append(Unit(UnitType.WORKER, (MAP_WIDTH - 5, MAP_HEIGHT - 6), self.player2))

        # Resources
        mineral_patches = [
            Resource((10, 10), 1000),
            Resource((MAP_WIDTH - 10, 10), 1000),
        ]
        self.player1.add_resource(mineral_patches[0])
        self.player2.add_resource(mineral_patches[1])
        all_resources = mineral_patches

        # Map
        self.map_tiles = generate_map(all_resources)

        # Selection
        self.selected_units: List[Unit] = []
        self.selecting = False
        self.select_start = (0, 0)

        # Camera (simple, no scrolling for demo)
        self.camera = [0, 0]

        # AI simple behavior
        self.ai_timer = 0

    def handle_events(self):
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                self.running = False
            elif event.type == pygame.MOUSEBUTTONDOWN:
                if event.button == 1:  # Left click
                    mouse_pos = pygame.mouse.get_pos()
                    map_pos = (mouse_pos[0] // TILE_SIZE, mouse_pos[1] // TILE_SIZE)
                    # Deselect if no ctrl
                    if not pygame.key.get_pressed()[pygame.K_LCTRL]:
                        self.selected_units = []
                    self.select_start = mouse_pos
                    self.selecting = True
                    # Check if clicking on unit
                    for unit in self.player1.units:
                        unit_rect = pygame.Rect(unit.pos[0] - unit.size//2, unit.pos[1] - unit.size//2, unit.size, unit.size)
                        if unit_rect.collidepoint(mouse_pos):
                            if pygame.key.get_pressed()[pygame.K_LCTRL]:
                                if unit not in self.selected_units:
                                    self.selected_units.append(unit)
                            else:
                                self.selected_units = [unit]
                            break
                elif event.button == 3:  # Right click for orders
                    if self.selected_units:
                        mouse_pos = pygame.mouse.get_pos()
                        map_pos = (mouse_pos[0] // TILE_SIZE, mouse_pos[1] // TILE_SIZE)
                        # Move order
                        for unit in self.selected_units:
                            unit.issue_order(Order.MOVE, mouse_pos)
                        # Check for harvest if worker
                        for res in self.player1.resources:
                            if math.hypot(mouse_pos[0] - res.pos[0]*TILE_SIZE, mouse_pos[1] - res.pos[1]*TILE_SIZE) < 20:
                                for unit in self.selected_units:
                                    if unit.type == UnitType.WORKER:
                                        unit.issue_order(Order.HARVEST, res)
                                        break
            elif event.type == pygame.MOUSEBUTTONUP:
                if event.button == 1 and self.selecting:
                    self.selecting = False
                    mouse_pos = pygame.mouse.get_pos()
                    # Box selection
                    if abs(mouse_pos[0] - self.select_start[0]) > 10 and abs(mouse_pos[1] - self.select_start[1]) > 10:
                        min_x = min(self.select_start[0], mouse_pos[0])
                        max_x = max(self.select_start[0], mouse_pos[0])
                        min_y = min(self.select_start[1], mouse_pos[1])
                        max_y = max(self.select_start[1], mouse_pos[1])
                        if not pygame.key.get_pressed()[pygame.K_LCTRL]:
                            self.selected_units = []
                        for unit in self.player1.units:
                            unit_rect = pygame.Rect(unit.pos[0] - unit.size//2, unit.pos[1] - unit.size//2, unit.size, unit.size)
                            if unit_rect.colliderect((min_x, min_y, max_x - min_x, max_y - min_y)):
                                self.selected_units.append(unit)
            elif event.type == pygame.KEYDOWN:
                if event.key == pygame.K_b:  # Build barracks (simplified, near selected worker)
                    if self.selected_units and self.selected_units[0].type == UnitType.WORKER and self.player1.minerals >= 150:
                        mouse_pos = pygame.mouse.get_pos()
                        map_pos = (mouse_pos[0] // TILE_SIZE, mouse_pos[1] // TILE_SIZE)
                        self.player1.buildings.append(Building(BuildingType.BARRACKS, map_pos, self.player1))
                        self.player1.minerals -= 150
                elif event.key == pygame.K_t:  # Train marine from barracks
                    for building in self.player1.buildings:
                        if building.type == BuildingType.BARRACKS:
                            building.train_unit(UnitType.MARINE)
                            self.player1.update_supply(1)
                            break

    def update(self):
        # Update units and buildings
        for unit in self.player1.units + self.player2.units:
            unit.update()

        # Simple harvest logic
        for unit in self.player1.units:
            if unit.order == Order.HARVEST and isinstance(unit.target, Resource):
                dist = math.hypot(unit.pos[0] - unit.target.pos[0]*TILE_SIZE, unit.pos[1] - unit.target.pos[1]*TILE_SIZE)
                if dist < 20:
                    unit.target.amount -= 8  # Gather rate
                    if unit.target.amount <= 0:
                        unit.target.depleted = True
                        unit.order = Order.IDLE
                    else:
                        self.player1.minerals += 8
                    # Return to base logic simplified

        # AI behavior (simple)
        self.ai_timer += 1
        if self.ai_timer > 60:  # Every second
            self.ai_timer = 0
            # AI worker harvest
            ai_worker = next((u for u in self.player2.units if u.type == "Worker"), None)
            if ai_worker and self.player2.resources:
                res = self.player2.resources[0]
                if not res.depleted:
                    ai_worker.issue_order(Order.HARVEST, res)
            # AI train zergling occasionally
            ai_hatch = next((b for b in self.player2.buildings if b.type == BuildingType.HATCHERY), None)
            if ai_hatch and random.random() < 0.1 and self.player2.minerals >= 50:
                ai_hatch.train_unit(UnitType.ZERG_LING)
                self.player2.update_supply(1)
                self.player2.minerals -= 50
            # AI move to attack
            if len(self.player2.units) > 2:
                for unit in random.sample([u for u in self.player2.units if u.type != UnitType.WORKER], min(2, len(self.player2.units)-1)):
                    target = random.choice(self.player1.units)
                    if target:
                        unit.issue_order(Order.ATTACK, target)

    def draw(self):
        self.screen.fill(BLACK)
        draw_map(self.screen, self.map_tiles)

        # Draw resources
        for player in [self.player1, self.player2]:
            for res in player.resources:
                res.draw(self.screen)

        # Draw buildings
        for player in [self.player1, self.player2]:
            for building in player.buildings:
                building.draw(self.screen)

        # Draw units
        for player in [self.player1, self.player2]:
            for unit in player.units:
                unit.draw(self.screen)

        # Draw selection box
        if self.selecting:
            mouse_pos = pygame.mouse.get_pos()
            min_x = min(self.select_start[0], mouse_pos[0])
            max_x = max(self.select_start[0], mouse_pos[0])
            min_y = min(self.select_start[1], mouse_pos[1])
            max_y = max(self.select_start[1], mouse_pos[1])
            pygame.draw.rect(self.screen, WHITE, (min_x, min_y, max_x - min_x, max_y - min_y), 2)

        # Draw selection circles
        for unit in self.selected_units:
            pygame.draw.circle(self.screen, WHITE, (int(unit.pos[0]), int(unit.pos[1])), unit.size // 2 + 2, 2)

        # UI: Resources
        font = pygame.font.Font(None, 36)
        text = font.render(f"Minerals: {self.player1.minerals} | Supply: {self.player1.supply_used}/{self.player1.supply_max}", True, WHITE)
        self.screen.blit(text, (10, 10))
        ai_text = font.render(f"AI Minerals: {self.player2.minerals}", True, WHITE)
        self.screen.blit(ai_text, (10, 50))

        # Instructions
        small_font = pygame.font.Font(None, 24)
        instructions = [
            "Left click + drag: Select units (Ctrl for multi)",
            "Right click: Move/Harvest",
            "B: Build Barracks (near worker)",
            "T: Train Marine (from barracks)",
            "Close window to quit"
        ]
        for i, instr in enumerate(instructions):
            text = small_font.render(instr, True, WHITE)
            self.screen.blit(text, (10, SCREEN_HEIGHT - 120 + i * 20))

        pygame.display.flip()

    def run(self):
        while self.running:
            self.handle_events()
            self.update()
            self.draw()
            self.clock.tick(FPS)
        pygame.quit()
        sys.exit()

# Run the game
if __name__ == "__main__":
    game = StarCraftClone()
    game.run()
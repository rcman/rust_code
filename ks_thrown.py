import pygame
import math
import random

# Initialize Pygame
pygame.init()

# Constants
SCREEN_WIDTH = 800
SCREEN_HEIGHT = 600
FPS = 60

# Colors
BLACK = (0, 0, 0)
WHITE = (255, 255, 255)
RED = (255, 0, 0)
GREEN = (0, 255, 0)
BLUE = (0, 0, 255)
BROWN = (139, 69, 19)
GRAY = (128, 128, 128)
YELLOW = (255, 255, 0)

# Game classes
class King:
    def __init__(self, x, y):
        self.x = x
        self.y = y
        self.radius = 20
        self.health = 100
        self.max_health = 100
        self.shoot_cooldown = 0
        self.shoot_delay = 10  # Frames between shots

    def draw(self, screen):
        # Draw king (simple circle for body, line for crown)
        pygame.draw.circle(screen, BLUE, (int(self.x), int(self.y)), self.radius)
        pygame.draw.line(screen, YELLOW, (self.x - 10, self.y - self.radius), (self.x + 10, self.y - self.radius), 3)
        # Health bar
        bar_width = 40
        bar_height = 5
        fill = (self.health / self.max_health) * bar_width
        pygame.draw.rect(screen, RED, (self.x - bar_width // 2, self.y - self.radius - 10, bar_width, bar_height))
        pygame.draw.rect(screen, GREEN, (self.x - bar_width // 2, self.y - self.radius - 10, fill, bar_height))

    def update(self):
        if self.shoot_cooldown > 0:
            self.shoot_cooldown -= 1

    def shoot(self, target_x, target_y, projectiles):
        if self.shoot_cooldown <= 0:
            angle = math.atan2(target_y - self.y, target_x - self.x)
            proj = Projectile(self.x, self.y, angle, is_player=True)
            projectiles.append(proj)
            self.shoot_cooldown = self.shoot_delay

class Tower:
    def __init__(self, x, y):
        self.x = x
        self.y = y
        self.range = 150
        self.shoot_cooldown = 0
        self.shoot_delay = 30  # Slower than king
        self.damage = 20

    def draw(self, screen):
        pygame.draw.rect(screen, GRAY, (self.x - 10, self.y - 10, 20, 20))

    def update(self, enemies, projectiles):
        if self.shoot_cooldown > 0:
            self.shoot_cooldown -= 1
        else:
            # Find nearest enemy in range
            target = None
            min_dist = float('inf')
            for enemy in enemies:
                dist = math.hypot(enemy.x - self.x, enemy.y - self.y)
                if dist < self.range and dist < min_dist:
                    min_dist = dist
                    target = enemy
            if target:
                angle = math.atan2(target.y - self.y, target.x - self.x)
                proj = Projectile(self.x, self.y, angle, damage=self.damage)
                projectiles.append(proj)
                self.shoot_cooldown = self.shoot_delay

class Enemy:
    def __init__(self, start_x, start_y, target_x, target_y):
        self.x = start_x
        self.y = start_y
        self.target_x = target_x
        self.target_y = target_y
        self.speed = 1.5
        self.health = 50
        self.max_health = 50
        self.radius = 15
        self.gold_reward = 10

    def update(self):
        # Move towards throne
        dx = self.target_x - self.x
        dy = self.target_y - self.y
        dist = math.hypot(dx, dy)
        if dist > 0:
            self.x += (dx / dist) * self.speed
            self.y += (dy / dist) * self.speed

    def draw(self, screen):
        pygame.draw.circle(screen, RED, (int(self.x), int(self.y)), self.radius)
        # Health bar
        bar_width = 30
        bar_height = 4
        fill = (self.health / self.max_health) * bar_width
        pygame.draw.rect(screen, RED, (self.x - bar_width // 2, self.y - self.radius - 8, bar_width, bar_height))
        pygame.draw.rect(screen, GREEN, (self.x - bar_width // 2, self.y - self.radius - 8, fill, bar_height))

    def take_damage(self, damage):
        self.health -= damage
        return self.health <= 0

class Projectile:
    def __init__(self, x, y, angle, damage=10, is_player=False):
        self.x = x
        self.y = y
        self.vx = math.cos(angle) * 8
        self.vy = math.sin(angle) * 8
        self.damage = damage
        self.radius = 3 if not is_player else 2
        self.color = YELLOW if not is_player else WHITE

    def update(self, enemies, king):
        self.x += self.vx
        self.y += self.vy
        # Check collision with enemies
        for enemy in enemies[:]:
            dist = math.hypot(self.x - enemy.x, self.y - enemy.y)
            if dist < self.radius + enemy.radius:
                if enemy.take_damage(self.damage):
                    enemies.remove(enemy)
                    return True, enemy.gold_reward if hasattr(enemy, 'gold_reward') else 0
                return True, 0  # Hit but not killed
        # Check if hit king (for enemy projectiles) - but enemies don't shoot yet
        if not hasattr(self, 'is_player') or not self.is_player:
            dist_to_king = math.hypot(self.x - king.x, self.y - king.y)
            if dist_to_king < self.radius + king.radius:
                king.health -= self.damage
                return True, 0
        # Off screen
        if not (0 < self.x < SCREEN_WIDTH and 0 < self.y < SCREEN_HEIGHT):
            return True, 0
        return False, 0

    def draw(self, screen):
        pygame.draw.circle(screen, self.color, (int(self.x), int(self.y)), self.radius)

# Main game function
def main():
    screen = pygame.display.set_mode((SCREEN_WIDTH, SCREEN_HEIGHT))
    pygame.display.set_caption("Thronefall Clone - Build & Defend")
    clock = pygame.time.Clock()

    # Game objects
    king = King(SCREEN_WIDTH // 2, SCREEN_HEIGHT // 2)
    towers = []
    enemies = []
    projectiles = []

    # Thronefall mechanics
    gold = 200
    day = 1
    build_phase = True
    day_timer = 600  # 10 seconds at 60fps for build
    wave_active = False
    gold_earned = 0

    game_over = False
    win_condition = 5  # Win after 5 days
    font = pygame.font.Font(None, 36)
    small_font = pygame.font.Font(None, 24)

    running = True
    mouse_x, mouse_y = 0, 0
    tower_cost = 50

    while running:
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                running = False
            elif event.type == pygame.MOUSEBUTTONDOWN and not game_over:
                if build_phase:
                    # Place tower
                    if gold >= tower_cost:
                        # Simple placement, no grid or overlap check
                        towers.append(Tower(mouse_x, mouse_y))
                        gold -= tower_cost
                else:
                    # Shoot
                    king.shoot(mouse_x, mouse_y, projectiles)
            elif event.type == pygame.KEYDOWN:
                if event.key == pygame.K_r and game_over:
                    # Restart
                    king.health = king.max_health
                    towers.clear()
                    enemies.clear()
                    projectiles.clear()
                    gold = 200
                    day = 1
                    build_phase = True
                    day_timer = 600
                    wave_active = False
                    game_over = False

        if not game_over:
            mouse_x, mouse_y = pygame.mouse.get_pos()

            if build_phase:
                day_timer -= 1
                if day_timer <= 0:
                    build_phase = False
                    wave_active = True
                    # Spawn wave for the night
                    num_enemies = day * 3
                    for _ in range(num_enemies):
                        side = random.choice([0, SCREEN_WIDTH])
                        start_y = random.randint(50, SCREEN_HEIGHT - 50)
                        enemy = Enemy(side, start_y, king.x, king.y)
                        enemies.append(enemy)
            else:
                # Defend phase
                king.update()
                for tower in towers:
                    tower.update(enemies, projectiles)

                for enemy in enemies[:]:
                    enemy.update()
                    # Check if reached king
                    if math.hypot(enemy.x - king.x, enemy.y - king.y) < enemy.radius + king.radius:
                        king.health -= 15  # Contact damage
                        enemies.remove(enemy)

                for proj in projectiles[:]:
                    hit, reward = proj.update(enemies, king)
                    if hit:
                        gold_earned += reward
                        projectiles.remove(proj)

                # Check if wave cleared
                if len(enemies) == 0 and wave_active:
                    wave_active = False
                    build_phase = True
                    day_timer = 600 + (day * 60)  # Longer build times
                    gold += gold_earned + (day * 50)  # Base reward + kills
                    gold_earned = 0
                    day += 1
                    if day > win_condition:
                        # Win!
                        game_over = True
                        # Could add win screen

                # Check game over during defend
                if king.health <= 0:
                    game_over = True

        # Draw everything
        screen.fill(BROWN)  # Ground

        # Draw throne base
        pygame.draw.rect(screen, BLACK, (king.x - 30, king.y - 30, 60, 60))

        king.draw(screen)

        for tower in towers:
            tower.draw(screen)

        for enemy in enemies:
            enemy.draw(screen)

        for proj in projectiles:
            proj.draw(screen)

        # UI
        day_text = font.render(f"Day {day}", True, WHITE)
        screen.blit(day_text, (10, 10))
        gold_text = font.render(f"Gold: {gold}", True, WHITE)
        screen.blit(gold_text, (10, 50))
        health_text = font.render(f"Health: {king.health}", True, WHITE)
        screen.blit(health_text, (10, 90))

        if build_phase:
            phase_text = small_font.render("BUILD PHASE - Click to place towers (50 gold)", True, WHITE)
            screen.blit(phase_text, (10, SCREEN_HEIGHT - 50))
            timer_text = small_font.render(f"Time left: {day_timer // 60 + 1}", True, WHITE)
            screen.blit(timer_text, (10, SCREEN_HEIGHT - 30))
        else:
            phase_text = small_font.render("DEFEND PHASE - Click to shoot!", True, WHITE)
            screen.blit(phase_text, (10, SCREEN_HEIGHT - 50))
            if wave_active:
                enemies_left = small_font.render(f"Enemies left: {len(enemies)}", True, WHITE)
                screen.blit(enemies_left, (10, SCREEN_HEIGHT - 30))

        if game_over:
            if day > win_condition:
                go_text = font.render("Victory! You survived all days! Press R to Restart", True, GREEN)
            else:
                go_text = font.render("Game Over! Press R to Restart", True, RED)
            text_rect = go_text.get_rect(center=(SCREEN_WIDTH // 2, SCREEN_HEIGHT // 2))
            screen.blit(go_text, text_rect)

        pygame.display.flip()
        clock.tick(FPS)

    pygame.quit()

if __name__ == "__main__":
    main()
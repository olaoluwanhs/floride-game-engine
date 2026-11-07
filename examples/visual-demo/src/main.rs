use macroquad::prelude::*;

#[macroquad::main("FPS Core Visual Demo")]
async fn main() {
    loop {
        clear_background(BLACK);
        draw_text("Visual demo window works", 20.0, 40.0, 30.0, WHITE);
        next_frame().await;
    }
}

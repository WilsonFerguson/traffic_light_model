extern crate find_folder;
extern crate piston_window;
use car::LANE_WIDTH;
use piston_window::*;
use rand::{seq::SliceRandom, Rng};
use std::{path, time};

mod car;
mod traffic_light;

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 1280;

pub const USE_ENTRY_TIME: bool = false;

fn draw_map(context: &Context, graphics: &mut G2d) {
    let middle = (WIDTH as f64 / 2.0, HEIGHT as f64 / 2.0);
    [
        [0.0, 0.0],
        [middle.0 + LANE_WIDTH * 2.0, 0.0],
        [0.0, middle.1 + LANE_WIDTH * 2.0],
        [middle.0 + LANE_WIDTH * 2.0, middle.1 + LANE_WIDTH * 2.0],
    ]
    .iter()
    .for_each(|&start| {
        rectangle(
            [0.0, 1.0, 0.0, 1.0],
            [
                start[0],
                start[1],
                WIDTH as f64 / 2.0 - LANE_WIDTH * 2.0,
                HEIGHT as f64 / 2.0 - LANE_WIDTH * 2.0,
            ],
            context.transform,
            graphics,
        );
    });

    let dash_gap_percent = 2.0 / 5.0;
    let num_dashes: u32 = 10;
    let dash_width = 3.0;

    // Horizontal dashes
    let dash_length =
        (middle.0 - LANE_WIDTH * 2.0) / (num_dashes as f64 * (1.0 + dash_gap_percent));
    let dash_gap = dash_length * dash_gap_percent;
    for i in 0..(((middle.0 - LANE_WIDTH * 2.0) / (dash_length + dash_gap)) as u32) {
        let mut start = i as f64 * (dash_length + dash_gap) + dash_gap / 2.0;
        for _ in 0..2 {
            line_from_to(
                [1.0; 4],
                dash_width,
                [start, middle.1],
                [start + dash_length, middle.1],
                context.transform,
                graphics,
            );
            start += middle.0 + LANE_WIDTH * 2.0;
        }
    }

    // Vertical dashes
    let dash_length =
        (middle.1 - LANE_WIDTH * 2.0) / (num_dashes as f64 * (1.0 + dash_gap_percent));
    let dash_gap = dash_length * dash_gap_percent;
    for i in 0..(((middle.1 - LANE_WIDTH * 2.0) / (dash_length + dash_gap)) as u32) {
        let mut start = i as f64 * (dash_length + dash_gap) + dash_gap / 2.0;
        for _ in 0..2 {
            line_from_to(
                [1.0; 4],
                dash_width,
                [middle.0, start],
                [middle.0, start + dash_length],
                context.transform,
                graphics,
            );
            start += middle.1 + LANE_WIDTH * 2.0;
        }
    }

    // Stop lines
    for i in 0..2 {
        line_from_to(
            [1.0; 4],
            2.0,
            [
                WIDTH as f64 / 2.0 - LANE_WIDTH * 2.0 + LANE_WIDTH * 2.0 * i as f64,
                HEIGHT as f64 / 2.0 - LANE_WIDTH * 2.0 + LANE_WIDTH * 4.0 * i as f64,
            ],
            [
                WIDTH as f64 / 2.0 + LANE_WIDTH * 2.0 * i as f64,
                HEIGHT as f64 / 2.0 - LANE_WIDTH * 2.0 + LANE_WIDTH * 4.0 * i as f64,
            ],
            context.transform,
            graphics,
        );
        line_from_to(
            [1.0; 4],
            2.0,
            [
                WIDTH as f64 / 2.0 - LANE_WIDTH * 2.0 + LANE_WIDTH * 4.0 * i as f64,
                HEIGHT as f64 / 2.0 - LANE_WIDTH * 2.0 * i as f64,
            ],
            [
                WIDTH as f64 / 2.0 - LANE_WIDTH * 2.0 + LANE_WIDTH * 4.0 * i as f64,
                HEIGHT as f64 / 2.0 + LANE_WIDTH * 2.0 - LANE_WIDTH * 2.0 * i as f64,
            ],
            context.transform,
            graphics,
        );
    }
}

fn main() {
    let mut window: PistonWindow =
        WindowSettings::new("Insersection Traffic Manager", [WIDTH, HEIGHT])
            .exit_on_esc(true)
            .resizable(false)
            .build()
            .unwrap();

    let assets: path::PathBuf = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("assets")
        .unwrap();
    let mut glyphs: Glyphs = window.load_font(assets.join("Consolas.ttf")).unwrap();

    let mut cars: Vec<car::Car> = Vec::new();
    let mut id: usize = 0;

    let mut spawn_start = time::Instant::now();
    let mut spawn_increment = time::Duration::from_millis(1000);
    let origins = [
        car::Origin::North,
        car::Origin::South,
        car::Origin::East,
        car::Origin::West,
    ];
    let mut origin_index = 0;

    let mut score: usize = 0;

    let mut traffic_light = traffic_light::TrafficLight::new();

    window.set_max_fps(60);
    while let Some(event) = window.next() {
        window.draw_2d(&event, |context, graphics, device| {
            clear([0.1; 4], graphics);

            draw_map(&context, graphics);

            traffic_light.update();
            traffic_light.draw(&context, graphics);

            if spawn_start.elapsed() >= spawn_increment {
                let minimum_time = 650.0; // 550
                spawn_increment = time::Duration::from_millis(
                    (spawn_increment.as_millis() as f64 * 0.9975).max(minimum_time) as u64,
                );

                let mut origin = origins[rand::thread_rng().gen_range(0..origins.len())];
                // 630
                if spawn_increment.as_millis() <= 700 {
                    origin = origins[origin_index];
                    origin_index = (origin_index + 1) % origins.len();
                }
                let direction = car::Direction::from(rand::thread_rng().gen_range(0..=2));
                cars.push(car::Car::new(id, origin, direction));
                traffic_light.add_car(traffic_light::SimplifiedCar::new(origin, direction));
                id += 1;
                if id > 1000 {
                    id = 0;
                }

                spawn_start = time::Instant::now();
            }

            let cars_clone = cars.clone();
            cars.iter_mut().for_each(|car| {
                car.update(&cars_clone, &mut traffic_light, &context, graphics);
            });

            for i in (0..cars.len()).rev() {
                if cars[i].finished {
                    cars.remove(i);
                    score += 1;
                }
            }

            text::Text::new_color([0.0, 0.0, 0.0, 1.0], 20)
                .draw(
                    format!(
                        "Green time: {:.02}s",
                        (traffic_light.green_time().as_millis() as f64 / 1000.0)
                    )
                    .as_str(),
                    &mut glyphs,
                    &context.draw_state,
                    context.transform.trans(20.0, 35.0),
                    graphics,
                )
                .unwrap();
            text::Text::new_color([0.0, 0.0, 0.0, 1.0], 20)
                .draw(
                    format!("Spawn increment: {:?}", spawn_increment).as_str(),
                    &mut glyphs,
                    &context.draw_state,
                    context.transform.trans(20.0, 75.0),
                    graphics,
                )
                .unwrap();
            glyphs.factory.encoder.flush(device);
        });
    }
}

use piston_window::*;
use std::{
    collections::HashMap,
    f64::consts::PI,
    time::{Duration, Instant},
};

use crate::{
    car::{self, LANE_WIDTH, MAX_SPEED},
    HEIGHT, USE_ENTRY_TIME, WIDTH,
};

#[derive(Clone, Copy)]
pub struct SimplifiedCar {
    pub origin: car::Origin,
    pub direction: car::Direction,
}

impl SimplifiedCar {
    pub fn new(origin: car::Origin, direction: car::Direction) -> SimplifiedCar {
        SimplifiedCar { origin, direction }
    }
}

pub struct TrafficLight {
    queue: HashMap<car::Origin, Vec<SimplifiedCar>>,
    last_went: HashMap<car::Origin, Instant>,
    /// When the last time someone turned left was
    last_intersection_obstruction: Instant,
    start: bool,
    green: Option<car::Origin>,
    next_green: Option<car::Origin>,
    red_clearance_time: Duration,
    entry_time: Duration,
    yellow_time: Duration,
    minimum_green_time: Duration,
    green_time: Duration,
    phase_start: Instant,
    red_start: Instant,
    should_switch: bool,
    past_green: Option<car::Origin>,
}

impl TrafficLight {
    pub fn new() -> TrafficLight {
        let mut last_went = HashMap::new();
        last_went.insert(car::Origin::North, Instant::now());
        last_went.insert(car::Origin::South, Instant::now());
        last_went.insert(car::Origin::East, Instant::now());
        last_went.insert(car::Origin::West, Instant::now());
        let mut queue: HashMap<car::Origin, Vec<SimplifiedCar>> = HashMap::new();
        queue.insert(car::Origin::North, Vec::new());
        queue.insert(car::Origin::South, Vec::new());
        queue.insert(car::Origin::East, Vec::new());
        queue.insert(car::Origin::West, Vec::new());
        TrafficLight {
            queue,
            last_went,
            last_intersection_obstruction: Instant::now(),
            start: true,
            green: None,
            next_green: None,
            red_clearance_time: Duration::from_secs(2),
            entry_time: Duration::from_secs(0),
            yellow_time: Duration::from_secs_f64(1.5),
            minimum_green_time: Duration::from_secs_f64(1.2),
            green_time: Duration::from_secs(0),
            phase_start: Instant::now(),
            red_start: Instant::now(),
            should_switch: false,
            past_green: None,
        }
    }

    pub fn update(&mut self) {
        if !self.should_switch {
            self.green_time = self.phase_start.elapsed();
        }

        if self.should_switch_phase() {
            if self.longest_queue().1 >= ((self.current_queue() as f64) * 1.5) as usize {
                self.should_switch = true;
                self.red_start = Instant::now();
                self.past_green = self.green;
                self.green = None;

                self.next_green = Some(self.longest_queue().0);
            }
        }
        // If it's past yellow and the direction that just went will go again, just go back to
        // green
        if self.should_switch
            && self.red_start.elapsed() >= self.yellow_time
            && self.past_green.is_some()
            && self.longest_queue().0 == self.past_green.unwrap()
        {
            self.change_light();
        }
        // // If it's past yellow, change the light
        if self.should_switch && self.red_start.elapsed() >= self.red_clearance_time {
            // self.last_went
            //     .entry(self.green.unwrap())
            //     .and_modify(|x| *x = Instant::now());
            self.change_light();
        }
    }

    fn change_light(&mut self) {
        self.should_switch = false;
        self.phase_start = Instant::now();
        self.green = self.next_green;
        self.next_green = None;
        self.last_intersection_obstruction = Instant::now();
    }

    /// Calculates the entry time of a car into the intersection given the car already in the
    /// intersection and the currently waiting car
    fn calculate_entry_time(&mut self, moving_car: &SimplifiedCar, waiting_car: &SimplifiedCar) {
        let moving_car_path = car::Car::calculate_path(moving_car);
        let waiting_car_path = car::Car::calculate_path(waiting_car);

        let mut closest_distance = f64::MAX;
        let mut moving_path_index: usize = 0;
        let mut waiting_path_index: usize = 0;

        for (i, point) in moving_car_path.iter().enumerate() {
            for (j, other_point) in waiting_car_path.iter().enumerate() {
                let distance =
                    (point.0 - other_point.0).powi(2) + (point.1 - other_point.1).powi(2);
                if distance < closest_distance {
                    closest_distance = distance;
                    moving_path_index = i;
                    waiting_path_index = j;
                }
            }
        }

        // Crude approach
        if closest_distance.sqrt() > car::CAR_WIDTH * 2.0 {
            self.entry_time = Duration::from_millis(u64::MAX);
            return;
        }

        let waiting_car_point = waiting_car_path[waiting_path_index];
        let waiting_car_current_point =
            car::Car::calculate_waiting_point(&waiting_car, &waiting_car_path);

        let distance_to_collision = ((waiting_car_point.0 - waiting_car_current_point.0).powi(2)
            + (waiting_car_point.1 - waiting_car_current_point.1).powi(2))
        .sqrt();

        // Function: d = (1/2)at^2 assuming initial velocity is 0
        // So: t = sqrt(2d/a)
        let frame_duration = 1000.0 / 60.0;
        let num_frames = (2.0 * distance_to_collision / car::ACCELERATION).sqrt();
        self.entry_time = Duration::from_millis((num_frames * frame_duration) as u64);
        println!("Entry time: {:?}", self.entry_time);
    }

    fn calculate_red_clearance_time(&mut self, direction: car::Direction) {
        let straight_distance = LANE_WIDTH * 4.0;
        let left_distance = std::f64::consts::PI * LANE_WIDTH * 3.0 / 2.0;
        let right_distance = std::f64::consts::PI * LANE_WIDTH / 2.0;

        let distance_covered = match direction {
            car::Direction::Straight => straight_distance,
            car::Direction::Left => left_distance,
            car::Direction::Right => right_distance,
        };

        let speed = MAX_SPEED;
        let frame_duration = 1000.0 / 60.0;

        let mut clearance_time =
            Duration::from_millis((distance_covered / speed * frame_duration) as u64)
                + self.yellow_time;

        // let mut clearance_time =
        //     Duration::from_millis((distance_covered / speed * frame_duration) as u64);
        if USE_ENTRY_TIME {
            let final_clearance_time =
                clearance_time.as_millis() as f64 - self.entry_time.as_millis() as f64;
            // TODO: maybe allow negative red clearance time?
            // Note: right now entry time gets set to max f64 if they don't every collide
            if final_clearance_time < 0.0 {
                clearance_time = Duration::from_millis(0);
            } else {
                clearance_time = Duration::from_millis(final_clearance_time as u64);
            }
        }

        // self.red_clearance_time = clearance_time + self.yellow_time;
        self.red_clearance_time = clearance_time;
    }

    fn should_switch_phase(&self) -> bool {
        !self.should_switch
            && (self.phase_start.elapsed() >= self.minimum_green_time || self.current_queue() == 0)
    }

    fn current_queue(&self) -> usize {
        if let Some(green) = self.green {
            let mut len = self.queue.get(&green).unwrap().len();
            for car in self.queue.get(&green.right()).unwrap() {
                if car.direction != car::Direction::Right {
                    break;
                }
                len += 1;
            }
            len
        } else {
            0
        }
    }

    fn longest_queue(&self) -> (car::Origin, usize) {
        let mut queue_lengths: Vec<(car::Origin, usize)> = self
            .queue
            .iter()
            .map(|(origin, queue)| (*origin, queue.len()))
            .collect();

        // Count right turns as part of the queue on their left
        for (origin, cars) in &self.queue {
            for car in cars {
                if car.direction != car::Direction::Right {
                    break;
                }
                let left = origin.left();
                queue_lengths
                    .iter_mut()
                    .find(|(o, _)| *o == left)
                    .unwrap()
                    .1 += 1;
            }
        }

        // Return highest queue length
        queue_lengths
            .into_iter()
            .max_by(|(_, a), (_, b)| a.cmp(b))
            .unwrap()
    }

    pub fn draw(&self, context: &Context, graphics: &mut G2d) {
        let light_radius = 15.0;
        let light_spacing = 10.0;
        let green = [0.24, 0.96, 0.21, 1.0];
        let yellow = [0.92, 0.95, 0.13, 1.0];
        let red = [0.96, 0.19, 0.19, 1.0];
        let dark_green = [0.05, 0.22, 0.04, 1.0];
        let dark_yellow = [0.3, 0.32, 0.04, 1.0];
        let dark_red = [0.34, 0.06, 0.06, 1.0];
        let origins = [
            car::Origin::North,
            car::Origin::East,
            car::Origin::South,
            car::Origin::West,
        ];

        for origin in origins.iter() {
            let final_position = match origin {
                car::Origin::North => Position {
                    x: LANE_WIDTH as i32 * 2,
                    y: LANE_WIDTH as i32 * 2,
                },
                car::Origin::East => Position {
                    x: LANE_WIDTH as i32 * 2,
                    y: LANE_WIDTH as i32 * 2,
                },
                car::Origin::South => Position {
                    x: LANE_WIDTH as i32 * 2,
                    y: LANE_WIDTH as i32 * 2,
                },
                car::Origin::West => Position {
                    x: LANE_WIDTH as i32 * 2,
                    y: LANE_WIDTH as i32 * 2,
                },
            };

            let transform = context
                .transform
                .trans(WIDTH as f64 / 2.0, HEIGHT as f64 / 2.0)
                .rot_rad(match origin {
                    car::Origin::North => PI,
                    car::Origin::East => 3.0 * PI / 2.0,
                    car::Origin::South => 0.0,
                    car::Origin::West => PI / 2.0,
                })
                .trans(final_position.x as f64, final_position.y as f64);
            Rectangle::new_round([0.0, 0.0, 0.0, 1.0], light_radius * 2.5).draw(
                [
                    0.0,
                    0.0,
                    light_radius * 5.0,
                    (light_radius * 2.0 + light_spacing) * 3.0 + light_spacing * 2.0,
                ],
                &context.draw_state,
                transform,
                graphics,
            );
            Rectangle::new_round_border([1.0; 4], light_radius * 2.5, 1.5).draw(
                [
                    0.0,
                    0.0,
                    light_radius * 5.0,
                    (light_radius * 2.0 + light_spacing) * 3.0 + light_spacing * 2.0,
                ],
                &context.draw_state,
                transform,
                graphics,
            );
            let final_red = match self.green() {
                // Light is green, make all other lights red and this one dark_red
                Some(green_light) => {
                    if green_light == *origin {
                        dark_red
                    } else {
                        red
                    }
                }
                None => {
                    // Light is yellow/red
                    if let Some(green_light) = self.past_green() {
                        if green_light == *origin {
                            if self.red_start.elapsed() >= self.yellow_time {
                                red
                            } else {
                                dark_red
                            }
                        } else {
                            red
                        }
                    } else {
                        red
                    }
                }
            };
            let final_yellow = if let Some(green_light) = self.past_green() {
                if green_light == *origin {
                    if self.red_start.elapsed() < self.yellow_time {
                        yellow
                    } else {
                        dark_yellow
                    }
                } else {
                    dark_yellow
                }
            } else {
                dark_yellow
            };
            // Show yellow before turning to green
            let final_yellow = if self.red_start.elapsed() >= self.yellow_time {
                if let Some(green_light) = self.next_green {
                    if green_light == *origin {
                        yellow
                    } else {
                        final_yellow
                    }
                } else {
                    final_yellow
                }
            } else {
                final_yellow
            };
            let final_green = if let Some(green_light) = self.green() {
                if green_light == *origin {
                    green
                } else {
                    dark_green
                }
            } else {
                dark_green
            };
            let colors = [final_red, final_yellow, final_green];
            for i in 0..3 {
                ellipse(
                    colors[i],
                    [
                        light_radius + light_radius * 0.5,
                        light_radius + (i as f64 * (light_radius * 2.0 + light_spacing)),
                        light_radius * 2.0,
                        light_radius * 2.0,
                    ],
                    transform,
                    graphics,
                );
            }

            if (self.green.is_some() && &self.green.unwrap().right() == origin)
                || (self.green.is_none()
                    && self.past_green.is_some()
                    && &self.past_green.unwrap().right() == origin)
            {
                let x = light_radius + light_radius * 0.5;
                let green_y = light_radius + 3.0 * (light_radius * 2.0 + light_spacing)
                    - light_radius * 2.0
                    + 5.0;
                let mut y = green_y;
                if self.phase_start.elapsed() > self.minimum_green_time
                    && self.red_start.elapsed() < self.yellow_time
                {
                    y -= (light_radius * 2.0 - light_spacing) * 2.0;
                }
                if !self.should_switch
                    || (self.should_switch && self.red_start.elapsed() < self.yellow_time)
                {
                    Line::new_round([0.8; 4], 2.0).draw_arrow(
                        [x + 5.0, y, x + light_radius * 2.0 - 5.0, y],
                        10.0,
                        &DrawState::default(),
                        transform,
                        graphics,
                    );
                }
            }
        }
    }

    pub fn add_car(&mut self, car: SimplifiedCar) {
        self.queue.entry(car.origin).or_insert(Vec::new()).push(car);
        if self.start {
            self.green = Some(car.origin);
            self.phase_start = Instant::now();
            self.start = false;
        }
    }

    pub fn remove_car(&mut self, origin: car::Origin, direction: car::Direction) {
        if let Some(queue) = self.queue.get_mut(&origin) {
            queue.remove(0);
            if direction == car::Direction::Left {
                self.last_intersection_obstruction = Instant::now();
            }

            let moving_car = SimplifiedCar { origin, direction };
            let waiting_car = if let Some(green) = self.next_green {
                if let Some(car) = self.queue.get(&green).and_then(|queue| queue.get(0)) {
                    Some(SimplifiedCar {
                        origin: green,
                        direction: car.direction,
                    })
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(waiting_car) = waiting_car {
                self.calculate_entry_time(&moving_car, &waiting_car);
            }

            self.calculate_red_clearance_time(direction);
        }
    }

    pub fn green(&self) -> Option<car::Origin> {
        self.green
    }

    pub fn past_green(&self) -> Option<car::Origin> {
        self.past_green
    }

    /// Returns if the light is green or yellow for the given origin.
    pub fn is_green(&self, origin: car::Origin, direction: car::Direction) -> bool {
        let has_green = self.green == Some(origin);
        let has_yellow =
            self.past_green == Some(origin) && self.red_start.elapsed() < self.yellow_time;

        let using_green = if self.green.is_some() {
            self.green.unwrap()
        } else {
            if self.past_green.is_some() {
                self.past_green.unwrap()
            } else {
                car::Origin::North
            }
        };

        // * 2 for more of a buffer on the clearance time
        let mut short_green = (!self.should_switch
            || (self.should_switch && self.red_start.elapsed() < self.yellow_time))
            && direction != car::Direction::Left
            && using_green == origin.opposite()
            && self.last_intersection_obstruction.elapsed()
                > (self.red_clearance_time - self.yellow_time) * 2
            && self.queue.get(&using_green).unwrap().len() > 0
            && self
                .queue
                .get(&using_green)
                .unwrap()
                .get(0)
                .unwrap()
                .direction
                != car::Direction::Left;
        if short_green && self.green.is_none() && self.past_green.is_none() {
            short_green = false;
        }
        has_green || has_yellow || short_green
    }

    pub fn green_time(&self) -> Duration {
        self.green_time
    }
}

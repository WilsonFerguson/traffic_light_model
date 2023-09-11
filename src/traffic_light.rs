use piston_window::*;
use std::{
    collections::HashMap,
    f64::consts::PI,
    time::{Duration, Instant},
};

use crate::{
    car::{self, Car, LANE_WIDTH, MAX_SPEED},
    ALLOW_GO_ON_YELLOW, ALLOW_MOVING_ON_RED, HEIGHT, USE_ENTRY_TIME, WIDTH,
};

#[derive(Clone, Copy, Debug)]
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
    /// The last car to go through the intersection
    latest_car: Option<SimplifiedCar>,
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
            latest_car: None,
            start: true,
            green: None,
            next_green: None,
            red_clearance_time: Duration::from_secs(2),
            entry_time: Duration::from_secs(0),
            yellow_time: Duration::from_secs_f64(1.5),
            minimum_green_time: Duration::from_secs_f64(2.2),
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
            if self.longest_queue().1 >= ((self.current_queue() as f64) * 1.75) as usize {
                self.should_switch = true;
                self.red_start = Instant::now();
                self.past_green = self.green;
                self.green = None;

                self.next_green = Some(self.longest_queue().0);

                self.calculate_clearance_time();
            }
        }
        // If it's past yellow and the direction that just went will go again, just go back to
        // green
        if self.should_switch
            && self.red_start.elapsed() >= self.yellow_time
            && self.past_green.is_some()
            && self.longest_queue().0 == self.past_green.unwrap()
        {
            self.should_switch = false;
            self.phase_start = Instant::now();
            self.green = self.past_green;
            self.next_green = None;
        }
        // // If it's past yellow, change the light
        if self.should_switch && self.red_start.elapsed() >= self.red_clearance_time {
            self.phase_start = Instant::now();

            self.green = self.next_green;
            self.next_green = None;

            self.last_went
                .entry(self.green.unwrap())
                .and_modify(|x| *x = Instant::now());

            self.should_switch = false;
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

        let mut moving_path_index: usize = 0;
        let mut waiting_path_index: usize = 0;

        for (i, point) in moving_car_path.iter().enumerate() {
            for (j, other_point) in waiting_car_path.iter().enumerate() {
                if i == 0 || j == 0 {
                    continue;
                }

                let rotation = (point.1 - moving_car_path[i - 1].1)
                    .atan2(point.0 - moving_car_path[i - 1].0)
                    .to_degrees();
                let other_rotation = (other_point.1 - waiting_car_path[j - 1].1)
                    .atan2(other_point.0 - waiting_car_path[j - 1].0)
                    .to_degrees();
                if !Car::cars_intersect(*point, rotation, *other_point, other_rotation) {
                    continue;
                }

                moving_path_index = i;
                waiting_path_index = j;
                break;
            }
            if moving_path_index != 0 {
                break;
            }
        }

        // Don't intersect
        if moving_path_index == 0 {
            self.entry_time = Duration::from_secs(100);
            return;
        }

        let end_index = (waiting_path_index - 1)
            .min(waiting_car_path.len() - 1)
            .max(0);
        let distance_to_collision = (car::Car::calculate_waiting_point_index(waiting_car)
            ..=end_index)
            .map(|i| i as f64)
            .reduce(|acc, i| {
                let distance =
                    ((waiting_car_path[i as usize].0 - waiting_car_path[i as usize + 1].0).powi(2)
                        + (waiting_car_path[i as usize].1 - waiting_car_path[i as usize + 1].1)
                            .powi(2))
                    .sqrt();
                acc + distance
            })
            .unwrap_or(0.0);

        // Function: d = (1/2)at^2 assuming initial velocity is 0
        // So: t = sqrt(2d/a)
        let num_frames = (2.0 * distance_to_collision / car::ACCELERATION).sqrt();

        let frame_duration = 1000.0 / 60.0;
        self.entry_time = Duration::from_millis((num_frames * frame_duration) as u64);
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

        let mut clearance_time = distance_covered / speed * frame_duration; // Raw all red time
        if USE_ENTRY_TIME {
            clearance_time -= self.entry_time.as_millis() as f64; // Subtract entry time
        }
        clearance_time += self.yellow_time.as_millis() as f64; // Add in yellow at the start

        // TODO: maybe allow negative red clearance time? (meaning < yellow_time)
        // Note: right now entry time gets set to max f64 if they don't every collide
        // clearance_time = clearance_time.max(self.yellow_time.as_millis() as f64);
        clearance_time = clearance_time.max(0.0);
        self.red_clearance_time = Duration::from_millis(clearance_time as u64);
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
            let show_yellow = if let Some(green_light) = self.past_green() {
                if green_light == *origin && self.red_start.elapsed() < self.yellow_time {
                    true
                } else {
                    false
                }
            } else {
                false
            };
            let final_red = match self.green() {
                // Light is green, make all other lights red and this one dark_red
                Some(green_light) => {
                    if green_light == *origin {
                        dark_red
                    } else {
                        if show_yellow {
                            dark_red
                        } else {
                            red
                        }
                    }
                }
                None => {
                    if show_yellow {
                        dark_red
                    } else {
                        red
                    }
                }
            };
            let final_yellow = if show_yellow { yellow } else { dark_yellow };
            // // Show yellow before turning to green
            // let final_yellow = if self.red_start.elapsed() >= self.yellow_time {
            //     if let Some(green_light) = self.next_green {
            //         if green_light == *origin {
            //             yellow
            //         } else {
            //             final_yellow
            //         }
            //     } else {
            //         final_yellow
            //     }
            // } else {
            //     final_yellow
            // };
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

    pub fn draw_stats(&self, glyphs: &mut Glyphs, context: &Context, graphics: &mut G2d) {
        let lines = [
            format!(
                "Green time: {:.02}s",
                (self.green_time().as_millis() as f64 / 1000.0)
            ),
            format!(
                "Entry time: {:.02}s",
                (self.entry_time.as_millis() as f64 / 1000.0)
            ),
            format!(
                "Red time (w yellow): {:.02}s",
                (self.red_clearance_time.as_millis() as f64 / 1000.0)
            ),
            format!(
                "Red time (w/0 yellow): {:.02}s",
                // (self.red_clearance_time - self.yellow_time).as_millis() as f64 / 1000.0
                (self.red_clearance_time.as_millis() as f64 - self.yellow_time.as_millis() as f64)
                    / 1000.0
            ),
            format!(
                "Next Green: {:?}",
                if let Some(green) = self.next_green {
                    format!("{:?}", green)
                } else {
                    String::from("None")
                }
            ),
            format!(
                "Latest Car: {:?}",
                if let Some(car) = self.latest_car {
                    format!("{:?}", car.direction)
                } else {
                    String::from("None")
                }
            ),
        ];

        for (i, line) in lines.iter().enumerate() {
            text::Text::new_color([0.0, 0.0, 0.0, 1.0], 20)
                .draw(
                    line.as_str(),
                    glyphs,
                    &context.draw_state,
                    context
                        .transform
                        .trans(20.0, 35.0 + ((i + 1) as f64 * 40.0)),
                    graphics,
                )
                .unwrap();
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

            // If they turned right on red, don't use them for clearance time
            if ((self.green.is_some() && Some(origin) != self.green)
                || (self.green.is_none()
                    && self.past_green.is_some()
                    && Some(origin) != self.past_green))
                && direction == car::Direction::Right
            {
                return;
            }
            self.latest_car = Some(SimplifiedCar { origin, direction });
            self.calculate_clearance_time();
        }
    }

    /// Calculates entry time and red clearance time
    fn calculate_clearance_time(&mut self) {
        if let Some(moving_car) = self.latest_car {
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

            self.calculate_red_clearance_time(moving_car.direction);
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
        let has_yellow = self.is_yellow(origin);

        let using_green = if self.green.is_some() {
            self.green.unwrap()
        } else {
            self.past_green.unwrap()
        };
<<<<<<< HEAD

        // * 2 for more of a buffer on the clearance time
        // Short green allows some cars in opposite direction of the current green to go
        let mut short_green = false;
        if ALLOW_MOVING_ON_RED {
            short_green = (!self.should_switch
                || (self.should_switch && self.red_start.elapsed() < self.yellow_time))
                && direction != car::Direction::Left
                && using_green == origin.opposite()
                && self.last_intersection_obstruction.elapsed()
                    > Duration::from_millis(
                        (self.red_clearance_time.as_millis() as f64
                            - self.yellow_time.as_millis() as f64)
                            .max(0.0) as u64
                            * 2,
                    )
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
        }
=======
        let short_green = (!self.should_switch
            || (self.should_switch && self.red_start.elapsed() < self.yellow_time))
            && direction != car::Direction::Left
            && using_green == origin.opposite()
            && self.last_intersection_obstruction.elapsed()
                > self.red_clearance_time - self.yellow_time;
        // && self.queue.get(&using_green).unwrap().len() > 0
        // && self
        //     .queue
        //     .get(&using_green)
        //     .unwrap()
        //     .get(0)
        //     .unwrap()
        //     .direction
        //     != car::Direction::Left;
>>>>>>> parent of c4edcb6 (cars go straight/right when possible touch buggy?)
        has_green || has_yellow || short_green
    }

    /// Returns true if the light is yellow, but only for the first part of the yellow
    /// Only the first part so that cars don't enter the intersection right before it turns red
    pub fn is_yellow(&self, origin: car::Origin) -> bool {
        if ALLOW_GO_ON_YELLOW {
            self.past_green == Some(origin)
                && self.red_start.elapsed().as_millis()
                    < (self.yellow_time.as_millis() as f64 * 0.3) as u128
        } else {
            false
        }
    }

    pub fn green_time(&self) -> Duration {
        self.green_time
    }

    pub fn unpause(&mut self, time_elapsed: Duration) {
        self.last_intersection_obstruction += time_elapsed;
        self.phase_start += time_elapsed;
        self.red_start += time_elapsed;
    }
}

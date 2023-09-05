use piston_window::*;

use crate::{traffic_light::TrafficLight, HEIGHT, WIDTH};

pub const MAX_SPEED: f64 = 5.0;
const ACCELERATION: f64 = 0.15;
const DECELERATION: f64 = 0.3;

const DISTANCE_THRESHOLD: f64 = 5.0;

const CAR_WIDTH: f64 = 50.0; // 75.0, 50
const CAR_HEIGHT: f64 = 33.0; // 50.0, 33
const ARROW_STROKE_WEIGHT: f64 = 2.5; //  5.0, 2.5

pub const LANE_WIDTH: f64 = CAR_HEIGHT * 1.5;

const NUM_PATH_POINTS: usize = 25; // Higher = more accurate path but more expensive

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Origin {
    North,
    South,
    East,
    West,
}

impl Origin {
    pub fn from(i: usize) -> Origin {
        match i {
            0 => Origin::North,
            1 => Origin::South,
            2 => Origin::East,
            3 => Origin::West,
            _ => panic!("Invalid origin"),
        }
    }

    pub fn to(&self) -> usize {
        match self {
            Origin::North => 0,
            Origin::South => 1,
            Origin::East => 2,
            Origin::West => 3,
        }
    }

    pub fn right(&self) -> Origin {
        match self {
            Origin::North => Origin::East,
            Origin::South => Origin::West,
            Origin::East => Origin::South,
            Origin::West => Origin::North,
        }
    }

    pub fn left(&self) -> Origin {
        match self {
            Origin::North => Origin::West,
            Origin::South => Origin::East,
            Origin::East => Origin::North,
            Origin::West => Origin::South,
        }
    }

    pub fn opposite(&self) -> Origin {
        match self {
            Origin::North => Origin::South,
            Origin::South => Origin::North,
            Origin::East => Origin::West,
            Origin::West => Origin::East,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Direction {
    Left,
    Right,
    Straight,
}

impl Direction {
    pub fn from(i: usize) -> Direction {
        match i {
            0 => Direction::Left,
            1 => Direction::Right,
            2 => Direction::Straight,
            _ => panic!("Invalid direction"),
        }
    }
}

#[derive(Clone)]
pub struct Car {
    pub id: usize,
    pub origin: Origin,
    direction: Direction,
    position: (f64, f64),
    rotation: f64,
    target_rotation: f64,
    speed: f64,
    stopped: bool,
    automatically_stopped: bool,
    path: Vec<(f64, f64)>,
    path_index: usize,
    path_index_on_red_change: Option<usize>,
    path_index_at_intersection: usize,
    pub finished: bool,
    through_intersection: bool,
}

impl Car {
    pub fn new(id: usize, origin: Origin, direction: Direction) -> Car {
        let rotation: f64 = match origin {
            Origin::North => 90.0,
            Origin::South => 270.0,
            Origin::East => 180.0,
            Origin::West => 0.0,
        };
        let path: Vec<(f64, f64)> = match direction {
            Direction::Left => generate_left_turn_path(origin),
            Direction::Right => generate_right_turn_path(origin),
            Direction::Straight => generate_straight_path(origin),
        };
        Car {
            id,
            origin,
            direction,
            position: get_position(origin),
            rotation,
            target_rotation: rotation,
            speed: 0.0,
            stopped: false,
            automatically_stopped: false,
            path,
            path_index: 1,
            path_index_on_red_change: None,
            path_index_at_intersection: NUM_PATH_POINTS / 3
                + if direction == Direction::Straight {
                    1
                } else {
                    0
                },
            finished: false,
            through_intersection: false,
        }
    }

    fn get_distance_to_closest_car(&mut self, cars: &Vec<Car>) -> f64 {
        let mut closest_distance = f64::MAX;

        cars.clone()
            .iter()
            .filter(|c| c.origin == self.origin && c.id != self.id)
            .for_each(|c| {
                let (x, y) = self.position;
                let (cx, cy) = c.position;
                match self.origin {
                    Origin::North => {
                        if cy < y {
                            return;
                        }
                    }
                    Origin::South => {
                        if cy > y {
                            return;
                        }
                    }
                    Origin::East => {
                        if cx > x {
                            return;
                        }
                    }
                    Origin::West => {
                        if cx < x {
                            return;
                        }
                    }
                }
                let distance = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                closest_distance = closest_distance.min(distance);
            });

        closest_distance
    }

    fn automatically_stop(&mut self, cars: &Vec<Car>) {
        let closest_distance = self.get_distance_to_closest_car(cars);
        // Make sure cars that are on top of each other don't stop
        if !self.stopped && closest_distance < CAR_WIDTH * 2.0 && closest_distance > 3.0 {
            self.stopped = true;
            self.automatically_stopped = true;
        } else if self.stopped && self.automatically_stopped && closest_distance > CAR_WIDTH * 2.0 {
            self.stopped = false;
            self.automatically_stopped = false;
        }
    }

    fn stop_for_traffic_light(&mut self, traffic_light: &mut TrafficLight) {
        if self.through_intersection {
            self.stopped = false;
            return;
        }

        // Red clearance time
        if traffic_light.green().is_none() && !traffic_light.is_green(self.origin, self.direction) {
            if self.path_index_on_red_change.is_none() {
                self.path_index_on_red_change = Some(self.path_index);
            }
            // Allow cars in intersection to clear
            if let Some(past_green) = traffic_light.past_green() {
                if past_green == self.origin
                    && self.path_index_on_red_change.unwrap() >= self.path_index_at_intersection
                {
                    self.through_intersection = true;
                    self.stopped = false;
                } else if self.path_index == self.path_index_at_intersection {
                    self.stopped = true;
                }
            } else {
                if self.path_index == self.path_index_at_intersection {
                    self.stopped = true;
                }
            }
            return;
        }
        self.path_index_on_red_change = None;

        let mut can_go = traffic_light.is_green(self.origin, self.direction);
        // If it's red but I'm not at the intersection, I can keep going
        if !can_go && self.path_index != self.path_index_at_intersection {
            can_go = true;
        }
        // Protected right turns
        if self.direction == Direction::Right
            && traffic_light.is_green(self.origin.left(), self.direction)
        {
            can_go = true;
        }

        self.stopped = !can_go;
    }

    pub fn update(
        &mut self,
        cars: &Vec<Car>,
        traffic_light: &mut TrafficLight,
        context: &Context,
        graphics: &mut G2d,
    ) {
        if !self.through_intersection && self.past_intersection() {
            self.through_intersection = true;
            traffic_light.remove_car(self.origin, self.direction);
        }

        self.stop_for_traffic_light(traffic_light);
        self.automatically_stop(cars);

        if !self.stopped {
            self.speed += ACCELERATION;
            if self.speed > MAX_SPEED {
                self.speed = MAX_SPEED;
            }
        } else {
            if self.speed > 0.0 {
                self.speed -= DECELERATION;
            } else {
                self.speed = 0.0;
            }
        }

        // Move towards next point in path
        let dx = self.rotation.to_radians().cos() * self.speed;
        let dy = self.rotation.to_radians().sin() * self.speed;
        self.position.0 += dx;
        self.position.1 += dy;

        if self.intersects_point(self.path[self.path_index]) {
            self.path_index += 1;
            if self.path_index >= self.path.len() {
                self.path_index = 0;
                self.finished = true;
            }

            if self.path_index >= 1 {
                let dx = self.path[self.path_index].0 - self.position.0;
                let dy = self.path[self.path_index].1 - self.position.1;
                self.target_rotation = dy.atan2(dx).to_degrees();
            }
        }

        // Rotate towards target rotation
        let mut diff = self.target_rotation - self.rotation;
        if diff > 180.0 {
            diff -= 360.0;
        } else if diff < -180.0 {
            diff += 360.0;
        }
        self.rotation += diff * 0.5;

        self.draw(cars, context, graphics);
    }

    fn past_intersection(&self) -> bool {
        self.path_index > self.path_index_at_intersection
    }

    fn intersects_point(&self, point: (f64, f64)) -> bool {
        let dx = self.position.0 - point.0;
        let dy = self.position.1 - point.1;
        let distance = (dx * dx + dy * dy).sqrt();
        distance < DISTANCE_THRESHOLD
    }

    pub fn intersects_rect(&self, other_vertices: [(f64, f64); 4]) -> bool {
        let my_vertices = self.vertices();
        let my_lines = my_vertices
            .iter()
            .zip(my_vertices.iter().cycle().skip(1))
            .map(|(&a, &b)| (a, b))
            .collect::<Vec<_>>();
        let other_lines = other_vertices
            .iter()
            .zip(other_vertices.iter().cycle().skip(1))
            .map(|(&a, &b)| (a, b))
            .collect::<Vec<_>>();

        my_lines.iter().any(|&line| {
            for other_line in &other_lines {
                if line_intersect(line, &other_line) {
                    return true;
                }
            }
            return false;
        })
    }

    fn get_vertex(&self, vertex: (f64, f64)) -> (f64, f64) {
        (
            self.position.0 + (vertex.0 * self.rotation.to_radians().cos())
                - (vertex.1 * self.rotation.to_radians().sin()),
            self.position.1
                + (vertex.0 * self.rotation.to_radians().sin())
                + (vertex.1 * self.rotation.to_radians().cos()),
        )
    }

    pub fn vertices(&self) -> [(f64, f64); 4] {
        let half_width = CAR_WIDTH / 2.0;
        let half_height = CAR_HEIGHT / 2.0;

        let front_left = (-half_width, -half_height);
        let front_right = (half_width, -half_height);
        let back_left = (-half_width, half_height);
        let back_right = (half_width, half_height);

        [
            self.get_vertex(front_left),
            self.get_vertex(front_right),
            self.get_vertex(back_right),
            self.get_vertex(back_left),
        ]
    }

    pub fn draw(&self, cars: &Vec<Car>, context: &Context, graphics: &mut G2d) {
        let transform = context
            .transform
            .trans(self.position.0, self.position.1)
            .rot_deg(self.rotation);

        let fill_color = if cars
            .iter()
            .filter(|c| c.id != self.id)
            .any(|c| self.intersects_rect(c.vertices()))
        {
            [1.0, 0.0, 0.0, 1.0]
        } else {
            [1.0; 4]
        };
        rectangle_from_to(
            fill_color,
            [-CAR_WIDTH / 2.0, -CAR_HEIGHT / 2.0],
            [CAR_WIDTH / 2.0, CAR_HEIGHT / 2.0],
            transform,
            graphics,
        );

        match self.direction {
            Direction::Straight => Line::new_round([0.0, 0.0, 0.0, 1.0], ARROW_STROKE_WEIGHT)
                .draw_arrow(
                    [-CAR_WIDTH / 2.5, 0.0, CAR_WIDTH / 2.5, 0.0],
                    CAR_HEIGHT / 2.5,
                    &DrawState::default(),
                    transform,
                    graphics,
                ),
            Direction::Left => Line::new_round([0.0, 0.0, 0.0, 1.0], ARROW_STROKE_WEIGHT)
                .draw_arrow(
                    [0.0, CAR_HEIGHT / 2.5, 0.0, -CAR_HEIGHT / 2.5],
                    CAR_HEIGHT / 2.5,
                    &DrawState::default(),
                    transform,
                    graphics,
                ),
            Direction::Right => Line::new_round([0.0, 0.0, 0.0, 1.0], ARROW_STROKE_WEIGHT)
                .draw_arrow(
                    [0.0, -CAR_HEIGHT / 2.5, 0.0, CAR_HEIGHT / 2.5],
                    CAR_HEIGHT / 2.5,
                    &DrawState::default(),
                    transform,
                    graphics,
                ),
        }

        // self.draw_path(context, graphics);
    }

    fn draw_path(&self, context: &Context, graphics: &mut G2d) {
        self.path.iter().for_each(|&point| {
            line_from_to(
                [1.0, 0.0, 0.0, 1.0],
                3.0,
                [point.0 - 2.0, point.1 - 2.0],
                [point.0 + 2.0, point.1 + 2.0],
                context.transform,
                graphics,
            );
        });
    }
}

fn get_position(origin: Origin) -> (f64, f64) {
    let middle = (WIDTH as f64 / 2.0, HEIGHT as f64 / 2.0);
    match origin {
        Origin::North => (middle.0 - LANE_WIDTH, CAR_WIDTH / 2.0),
        Origin::South => (middle.0 + LANE_WIDTH, HEIGHT as f64 - CAR_WIDTH / 2.0),
        Origin::East => (WIDTH as f64 - CAR_WIDTH / 2.0, middle.1 - LANE_WIDTH),
        Origin::West => (CAR_WIDTH / 2.0, middle.1 + LANE_WIDTH),
    }
}

/// Generates the initial straight that all cars have to do before they can turn
fn generate_straight_path_third(origin: Origin) -> Vec<(f64, f64)> {
    let vertical_point_gap = (HEIGHT as f64 / 2.0 - LANE_WIDTH * 2.0 - CAR_WIDTH / 2.0) as f64
        / (NUM_PATH_POINTS / 3) as f64;
    let horizontal_point_gap = (WIDTH as f64 / 2.0 - LANE_WIDTH * 2.0 - CAR_WIDTH / 2.0) as f64
        / (NUM_PATH_POINTS / 3) as f64;
    let position = get_position(origin);

    match origin {
        Origin::North => (0..NUM_PATH_POINTS / 3)
            .map(|i| (position.0, position.1 + i as f64 * vertical_point_gap))
            .collect(),
        Origin::South => (0..NUM_PATH_POINTS / 3)
            .map(|i| (position.0, position.1 - (i as f64 * vertical_point_gap)))
            .collect(),
        Origin::East => (0..NUM_PATH_POINTS / 3)
            .map(|i| (position.0 - (i as f64 * horizontal_point_gap), position.1))
            .collect(),
        Origin::West => (0..NUM_PATH_POINTS / 3)
            .map(|i| (position.0 + (i as f64 * horizontal_point_gap), position.1))
            .collect(),
    }
}

fn generate_left_turn_path(origin: Origin) -> Vec<(f64, f64)> {
    // Initial straight
    let mut path = generate_straight_path_third(origin);

    // Turn
    let turn_origin = match origin {
        Origin::North => (
            WIDTH as f64 / 2.0 + LANE_WIDTH as f64 * 2.0,
            HEIGHT as f64 / 2.0 - LANE_WIDTH as f64 * 2.0,
        ),
        Origin::South => (
            WIDTH as f64 / 2.0 - LANE_WIDTH as f64 * 2.0,
            HEIGHT as f64 / 2.0 + LANE_WIDTH as f64 * 2.0,
        ),
        Origin::East => (
            WIDTH as f64 / 2.0 + LANE_WIDTH as f64 * 2.0,
            HEIGHT as f64 / 2.0 + LANE_WIDTH as f64 * 2.0,
        ),
        Origin::West => (
            WIDTH as f64 / 2.0 - LANE_WIDTH as f64 * 2.0,
            HEIGHT as f64 / 2.0 - LANE_WIDTH as f64 * 2.0,
        ),
    };
    let turn_path = match origin {
        Origin::North => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle = (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI
                    / 2.0
                    - std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 - angle.cos() * LANE_WIDTH * 3.0,
                    turn_origin.1 - angle.sin() * LANE_WIDTH * 3.0,
                )
            })
            .collect::<Vec<_>>(),
        Origin::South => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle = (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI
                    / 2.0
                    + std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 - angle.cos() * LANE_WIDTH * 3.0,
                    turn_origin.1 - angle.sin() * LANE_WIDTH * 3.0,
                )
            })
            .collect::<Vec<_>>(),
        Origin::East => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle = (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI
                    / 2.0
                    + std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 - angle.sin() * LANE_WIDTH * 3.0,
                    turn_origin.1 + angle.cos() * LANE_WIDTH * 3.0,
                )
            })
            .collect::<Vec<_>>(),
        Origin::West => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle = (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI
                    / 2.0
                    + std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 + angle.sin() * LANE_WIDTH * 3.0,
                    turn_origin.1 - angle.cos() * LANE_WIDTH * 3.0,
                )
            })
            .collect::<Vec<_>>(),
    };
    path.extend(turn_path.iter().rev().collect::<Vec<_>>());

    let straight_path = generate_straight_path(match origin {
        Origin::North => Origin::West,
        Origin::South => Origin::East,
        Origin::East => Origin::North,
        Origin::West => Origin::South,
    });
    let straight_path = straight_path
        .iter()
        .skip(NUM_PATH_POINTS * 2 / 3 - 1)
        .collect::<Vec<_>>();

    path.extend(straight_path);
    path
}

fn generate_right_turn_path(origin: Origin) -> Vec<(f64, f64)> {
    // Initial straight
    let mut path = generate_straight_path_third(origin);

    // Turn
    let turn_origin = match origin {
        Origin::North => (
            WIDTH as f64 / 2.0 - LANE_WIDTH * 2.0,
            HEIGHT as f64 / 2.0 - LANE_WIDTH * 2.0,
        ),
        Origin::South => (
            WIDTH as f64 / 2.0 + LANE_WIDTH * 2.0,
            HEIGHT as f64 / 2.0 + LANE_WIDTH * 2.0,
        ),
        Origin::East => (
            WIDTH as f64 / 2.0 + LANE_WIDTH * 2.0,
            HEIGHT as f64 / 2.0 - LANE_WIDTH * 2.0,
        ),
        Origin::West => (
            WIDTH as f64 / 2.0 - LANE_WIDTH * 2.0,
            HEIGHT as f64 / 2.0 + LANE_WIDTH * 2.0,
        ),
    };
    let turn_path = match origin {
        Origin::North => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle =
                    (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 + angle.cos() * LANE_WIDTH,
                    turn_origin.1 + angle.sin() * LANE_WIDTH,
                )
            })
            .collect::<Vec<_>>(),
        Origin::South => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle =
                    (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 - angle.cos() * LANE_WIDTH,
                    turn_origin.1 - angle.sin() * LANE_WIDTH,
                )
            })
            .collect::<Vec<_>>(),
        Origin::East => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle =
                    (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 - angle.sin() * LANE_WIDTH,
                    turn_origin.1 + angle.cos() * LANE_WIDTH,
                )
            })
            .collect::<Vec<_>>(),
        Origin::West => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle =
                    (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 + angle.sin() * LANE_WIDTH,
                    turn_origin.1 - angle.cos() * LANE_WIDTH,
                )
            })
            .collect::<Vec<_>>(),
    };
    path.extend(turn_path);

    let straight_path = generate_straight_path(match origin {
        Origin::North => Origin::East,
        Origin::South => Origin::West,
        Origin::East => Origin::South,
        Origin::West => Origin::North,
    });
    let straight_path = straight_path
        .iter()
        .skip(NUM_PATH_POINTS * 2 / 3 - 1)
        .collect::<Vec<_>>();

    path.extend(straight_path);
    path
}

fn generate_straight_path(origin: Origin) -> Vec<(f64, f64)> {
    let vertical_point_gap = (HEIGHT as f64 + CAR_WIDTH / 2.0) as f64 / NUM_PATH_POINTS as f64;
    let horizontal_point_gap = (WIDTH as f64 + CAR_WIDTH / 2.0) as f64 / NUM_PATH_POINTS as f64;

    let position = get_position(origin);
    match origin {
        Origin::North => {
            let mut path = Vec::new();
            for i in 0..NUM_PATH_POINTS {
                path.push((position.0, position.1 + (i as f64 * vertical_point_gap)));
            }
            path
        }
        Origin::South => {
            let mut path = Vec::new();
            for i in 0..NUM_PATH_POINTS {
                path.push((position.0, position.1 - (i as f64 * vertical_point_gap)));
            }
            path
        }
        Origin::East => {
            let mut path = Vec::new();
            for i in 0..NUM_PATH_POINTS {
                path.push((position.0 - (i as f64 * horizontal_point_gap), position.1));
            }
            path
        }
        Origin::West => {
            let mut path = Vec::new();
            for i in 0..NUM_PATH_POINTS {
                path.push((position.0 + (i as f64 * horizontal_point_gap), position.1));
            }
            path
        }
    }
}

fn ccw(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    (c.1 - a.1) * (b.0 - a.0) > (b.1 - a.1) * (c.0 - a.0)
}

fn line_intersect(line: ((f64, f64), (f64, f64)), other_line: &((f64, f64), (f64, f64))) -> bool {
    let a = line.0;
    let b = line.1;
    let c = other_line.0;
    let d = other_line.1;
    ccw(a, c, d) != ccw(b, c, d) && ccw(a, b, c) != ccw(a, b, d)
}

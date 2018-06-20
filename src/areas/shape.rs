extern crate sdl2;

use evdev::Position;

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Rectangle {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    Triangle {
        a: Position,
        b: Position,
        c: Position,
    },
    Parallelogram {
        base: Position,
        u: Position,
        v: Position,
    },
}

impl Shape {
    fn triangle_area(a: Position, b: Position, c: Position) -> f32 {
        ((a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y)) as f32 / 2.0).abs()
    }

    pub fn contains(&self, position: Position) -> bool {
        match *self {
            Shape::Rectangle {
                x,
                y,
                width,
                height,
                ..
            } => {
                let x_in = position.x >= x && position.x < x + width;
                let y_in = position.y >= y && position.y < y + height;
                x_in && y_in
            }
            Shape::Triangle { a, b, c } => {
                let abp_area = Shape::triangle_area(a, b, position);
                let bcp_area = Shape::triangle_area(b, c, position);
                let cap_area = Shape::triangle_area(c, a, position);
                let abc_area = Shape::triangle_area(a, b, c);
                abp_area + bcp_area + cap_area == abc_area
            }
            Shape::Parallelogram { base, u, v } => {
                let translated_position = Position {
                    x: position.x - base.x,
                    y: position.y - base.y,
                };
                let multiplication_factor = 1.0 / (u.x * v.y - v.x * u.y) as f32;
                let u_component = (translated_position.x * v.y + translated_position.y * (-v.x))
                    as f32 * multiplication_factor;
                let v_component = (translated_position.x * (-u.y) + translated_position.y * u.x)
                    as f32 * multiplication_factor;
                u_component >= 0.0 && u_component <= 1.0 && v_component >= 0.0 && v_component <= 1.0
            }
        }
    }

    pub fn to_polygon(&self, x_factor: f32, y_factor: f32) -> (Box<[i16]>, Box<[i16]>) {
        let (mut xs, mut ys): (Box<[i16]>, Box<[i16]>) = match self {
            Shape::Rectangle {
                x,
                y,
                width,
                height,
                ..
            } => {
                let x1 = *x as i16;
                let y1 = *y as i16;
                let x2 = x1 + *width as i16;
                let y2 = y1 + *height as i16;
                (Box::new([x1, x2, x2, x1]), Box::new([y1, y1, y2, y2]))
            }
            Shape::Triangle { a, b, c } => (
                Box::new([a.x as i16, b.x as i16, c.x as i16]),
                Box::new([a.y as i16, b.y as i16, c.y as i16]),
            ),
            Shape::Parallelogram { base, u, v } => (
                Box::new([
                    base.x as i16,
                    (u.x + base.x) as i16,
                    (u.x + v.x + base.x) as i16,
                    (v.x + base.x) as i16,
                ]),
                Box::new([
                    base.y as i16,
                    (u.y + base.y) as i16,
                    (u.y + v.y + base.y) as i16,
                    (v.y + base.y) as i16,
                ]),
            ),
        };
        for x in xs.iter_mut() {
            *x = (*x as f32 * x_factor) as i16;
        }
        for y in ys.iter_mut() {
            *y = (*y as f32 * y_factor) as i16;
        }
        (xs, ys)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    mod contains {
        use super::*;

        mod rectangle {
            use super::*;

            const RECTANGLE: Shape = Shape::Rectangle {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            };

            #[test]
            fn detects_positions_inside() {
                assert!(RECTANGLE.contains(Position { x: 5, y: 5 }))
            }

            #[test]
            fn returns_false_for_positions_to_the_left() {
                assert!(!RECTANGLE.contains(Position { x: -5, y: 5 }))
            }

            #[test]
            fn returns_false_for_positions_to_the_right() {
                assert!(!RECTANGLE.contains(Position { x: 15, y: 5 }))
            }

            #[test]
            fn returns_false_for_positions_above() {
                assert!(!RECTANGLE.contains(Position { x: 5, y: 15 }))
            }

            #[test]
            fn returns_false_for_positions_below() {
                assert!(!RECTANGLE.contains(Position { x: 5, y: -5 }))
            }
        }

        mod triangle {
            use super::*;

            mod triangle_area {
                use super::*;

                #[test]
                fn returns_the_area_for_a_simple_triangle() {
                    let area = Shape::triangle_area(
                        Position { x: 0, y: 0 },
                        Position { x: 10, y: 0 },
                        Position { x: 0, y: 10 },
                    );
                    assert_eq!(area, 50.0);
                }

                #[test]
                fn returns_the_area_for_a_complicated_triangle() {
                    let area = Shape::triangle_area(
                        Position { x: 0, y: 5 },
                        Position { x: 5, y: 0 },
                        Position { x: 10, y: 10 },
                    );
                    assert_eq!(area, 37.5);
                }

                #[test]
                fn returns_zero_for_an_empty_triangle() {
                    let area = Shape::triangle_area(
                        Position { x: 0, y: 0 },
                        Position { x: 5, y: 5 },
                        Position { x: 10, y: 10 },
                    );
                    assert_eq!(area, 0.0);
                }
            }

            const TRIANGLE: Shape = Shape::Triangle {
                a: Position { x: 5, y: 0 },
                b: Position { x: 10, y: 10 },
                c: Position { x: 0, y: 10 },
            };

            #[test]
            fn detects_positions_inside() {
                assert!(TRIANGLE.contains(Position { x: 5, y: 5 }))
            }

            #[test]
            fn detects_positions_outside_of_the_bounding_box() {
                assert!(!TRIANGLE.contains(Position { x: 15, y: 5 }));
                assert!(!TRIANGLE.contains(Position { x: -5, y: 5 }));
                assert!(!TRIANGLE.contains(Position { x: 5, y: -5 }));
                assert!(!TRIANGLE.contains(Position { x: 5, y: 15 }));
            }

            #[test]
            fn detects_positions_outside_the_triangle() {
                assert!(!TRIANGLE.contains(Position { x: 1, y: 1 }));
                assert!(!TRIANGLE.contains(Position { x: 9, y: 1 }));
                let triangle = Shape::Triangle {
                    a: Position { x: 5, y: 0 },
                    b: Position { x: 10, y: 10 },
                    c: Position { x: 0, y: 5 },
                };
                assert!(!triangle.contains(Position { x: 1, y: 9 }));
            }
        }

        mod parallelogram {
            use super::*;

            const PARALLELOGRAM: Shape = Shape::Parallelogram {
                base: Position { x: 0, y: 0 },
                u: Position { x: 10, y: 5 },
                v: Position { x: 5, y: 10 },
            };

            #[test]
            fn detects_positions_inside() {
                assert!(PARALLELOGRAM.contains(Position { x: 5, y: 5 }));
                assert!(PARALLELOGRAM.contains(Position { x: 10, y: 10 }));
                assert!(PARALLELOGRAM.contains(Position { x: 5, y: 9 }));
            }

            #[test]
            fn returns_true_for_corners() {
                assert!(PARALLELOGRAM.contains(Position { x: 0, y: 0 }));
                assert!(PARALLELOGRAM.contains(Position { x: 5, y: 10 }));
                assert!(PARALLELOGRAM.contains(Position { x: 15, y: 15 }));
                assert!(PARALLELOGRAM.contains(Position { x: 10, y: 5 }));
            }

            #[test]
            fn detects_positions_outside() {
                assert!(!PARALLELOGRAM.contains(Position { x: 2, y: 8 }));
                assert!(!PARALLELOGRAM.contains(Position { x: 7, y: 13 }));
                assert!(!PARALLELOGRAM.contains(Position { x: 13, y: 7 }));
                assert!(!PARALLELOGRAM.contains(Position { x: 8, y: 2 }));
            }

            #[test]
            fn works_for_translated_parallelograms() {
                let parallelogram = Shape::Parallelogram {
                    base: Position { x: 10, y: 10 },
                    u: Position { x: 10, y: 0 },
                    v: Position { x: 0, y: 10 },
                };
                assert!(!parallelogram.contains(Position { x: 5, y: 5 }));
                assert!(!parallelogram.contains(Position { x: 15, y: 5 }));
                assert!(!parallelogram.contains(Position { x: 5, y: 15 }));
                assert!(parallelogram.contains(Position { x: 15, y: 15 }));
            }
        }
    }

    mod to_polygon {
        use super::*;

        mod triangle {
            use super::*;

            #[test]
            fn converts_a_triangle_correctly() {
                let triangle = Shape::Triangle {
                    a: Position { x: 0, y: 0 },
                    b: Position { x: 10, y: 10 },
                    c: Position { x: 5, y: 10 },
                };
                let expected: (Box<[i16]>, Box<[i16]>) =
                    (Box::new([0, 10, 5]), Box::new([0, 10, 10]));
                assert_eq!(triangle.to_polygon(1.0, 1.0), expected);
            }

            #[test]
            fn translates_touch_coordinates_to_screen_coordinates() {
                let triangle = Shape::Triangle {
                    a: Position { x: 1, y: 1 },
                    b: Position { x: 10, y: 10 },
                    c: Position { x: 5, y: 10 },
                };
                let expected: (Box<[i16]>, Box<[i16]>) =
                    (Box::new([2, 20, 10]), Box::new([3, 30, 30]));
                assert_eq!(triangle.to_polygon(2.0, 3.0), expected);
            }
        }

        mod parallelogram {
            use super::*;

            #[test]
            fn converts_correctly_base_zero() {
                let parallelogram = Shape::Parallelogram {
                    base: Position { x: 0, y: 0 },
                    u: Position { x: 10, y: 5 },
                    v: Position { x: 5, y: 10 },
                };

                let expected: (Box<[i16]>, Box<[i16]>) =
                    (Box::new([0, 10, 15, 5]), Box::new([0, 5, 15, 10]));
                assert_eq!(parallelogram.to_polygon(1.0, 1.0), expected);
            }

            #[test]
            fn converts_correctly_base_nonzero() {
                let parallelogram = Shape::Parallelogram {
                    base: Position { x: 33, y: 77 },
                    u: Position { x: 10, y: 5 },
                    v: Position { x: 5, y: 10 },
                };

                let expected: (Box<[i16]>, Box<[i16]>) =
                    (Box::new([33, 43, 48, 38]), Box::new([77, 82, 92, 87]));
                assert_eq!(parallelogram.to_polygon(1.0, 1.0), expected);
            }

            #[test]
            fn converts_touch_coordinates_correctly() {
                let parallelogram = Shape::Parallelogram {
                    base: Position { x: 0, y: 0 },
                    u: Position { x: 1, y: 0 },
                    v: Position { x: 0, y: 1 },
                };
                let expected: (Box<[i16]>, Box<[i16]>) =
                    (Box::new([0, 2, 2, 0]), Box::new([0, 0, 3, 3]));
                assert_eq!(parallelogram.to_polygon(2.0, 3.0), expected);
            }
        }

        mod stripes {
            use areas::Areas;

            #[test]
            fn translates_touch_coordinates_to_screen_coordinates() {
                let areas = Areas::stripes(1000, 1000, 10, 48).areas;
                let expected: (Box<[i16]>, Box<[i16]>) =
                    (Box::new([14, 21, 21, 14]), Box::new([0, 0, 5000, 5000]));
                assert_eq!(
                    areas
                        .get(2)
                        .unwrap()
                        .shape
                        .to_polygon(700.0 / 1000.0, 500.0 / 1000.0),
                    expected
                );
            }

            #[test]
            fn factors_in_the_area_size() {
                let areas = Areas::stripes(1000, 1000, 12, 48).areas;
                let x1 = (24.0 * 0.7) as i16;
                let x2 = (36.0 * 0.7) as i16;
                let y1 = (1.0 * 0.5) as i16;
                let y2 = ((1.0 * 0.5) + (10000.0 * 0.5)) as i16;
                let expected: (Box<[i16]>, Box<[i16]>) =
                    (Box::new([x1, x2, x2, x1]), Box::new([y1, y1, y2, y2]));
                assert_eq!(
                    areas
                        .get(2)
                        .unwrap()
                        .shape
                        .to_polygon(700.0 / 1000.0, 500.0 / 1000.0),
                    expected
                );
            }
        }
    }
}

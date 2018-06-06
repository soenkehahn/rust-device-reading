extern crate palette;
extern crate sdl2;

pub mod rectangle;
pub mod render;

use self::rectangle::Rectangle;
use self::sdl2::pixels::Color;
use self::sdl2::rect::Rect;
use evdev::{Position, TouchState};

fn midi_to_frequency(midi: i32) -> f32 {
    440.0 * 2.0_f32.powf(((midi - 69) as f32) / 12.0)
}

#[derive(Clone)]
pub struct Areas {
    rects: Vec<Rectangle>,
    touch_width: u32,
    touch_height: u32,
}

impl Areas {
    pub fn new(touch_width: u32, touch_height: u32, rect_size: i32, start_midi_note: i32) -> Areas {
        let mut rects = vec![];
        for i in 0..30 {
            rects.push(Rectangle {
                x: i * rect_size,
                y: 1,
                width: rect_size,
                height: 10000,
                midi_note: start_midi_note + i,
            });
        }
        Areas {
            rects,
            touch_width,
            touch_height,
        }
    }

    pub fn frequency(&self, position: Position) -> NoteEvent {
        let touched: Option<&Rectangle> = self.rects
            .iter()
            .filter(|rect| rect.contains(position))
            .next();
        match touched.map(|x| midi_to_frequency(x.midi_note())) {
            None => NoteEvent::NoteOff,
            Some(x) => NoteEvent::NoteOn(x),
        }
    }

    fn make_color(midi_note: i32) -> Color {
        use self::palette::rgb::Rgb;
        use self::palette::rgb::Srgb;
        use self::palette::Hsv;

        let hue_number = (midi_note * 7) % 12;

        let c: Rgb<_, u8> =
            Srgb::from(Hsv::new(hue_number as f32 * 30.0 + 240.0, 1.0, 1.0)).into_format();
        Areas::convert_color(c)
    }

    fn convert_color(color: palette::rgb::Rgb<self::palette::encoding::srgb::Srgb, u8>) -> Color {
        Color::RGB(color.red, color.green, color.blue)
    }

    fn ui_elements(self, screen_width: u32, screen_height: u32) -> Vec<(Rect, Color)> {
        let x_factor: f32 = screen_width as f32 / self.touch_width as f32;
        let y_factor: f32 = screen_height as f32 / self.touch_height as f32;
        self.rects
            .iter()
            .map(|x| {
                (
                    x.to_sdl_rect(x_factor, y_factor),
                    Areas::make_color(x.midi_note()),
                )
            })
            .collect()
    }
}

pub struct Frequencies {
    areas: Areas,
    iterator: Box<Iterator<Item = TouchState<Position>>>,
}

impl Frequencies {
    pub fn new(
        areas: Areas,
        iterator: impl Iterator<Item = TouchState<Position>> + 'static,
    ) -> Frequencies {
        Frequencies {
            areas,
            iterator: Box::new(iterator),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum NoteEvent {
    NoteOff,
    NoteOn(f32),
}

impl Iterator for Frequencies {
    type Item = NoteEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.iterator.next().map(|touchstate| match touchstate {
            TouchState::NoTouch => NoteEvent::NoteOff,
            TouchState::Touch(position) => self.areas.frequency(position),
        })
    }
}

#[cfg(test)]
mod test {
    use super::NoteEvent::*;
    use super::*;

    mod midi_to_frequency {
        use super::*;

        #[test]
        fn converts_the_concert_pitch_correctly() {
            assert_eq!(midi_to_frequency(69), 440.0);
        }

        #[test]
        fn converts_octaves_correctly() {
            assert_eq!(midi_to_frequency(57), 220.0);
        }

        #[test]
        fn converts_semitones_correctly() {
            assert_eq!(midi_to_frequency(70), 440.0 * 2.0_f32.powf(1.0 / 12.0));
        }
    }

    fn pos(x: i32) -> Position {
        Position { x, y: 5 }
    }

    mod areas {
        use super::*;

        mod frequency {
            use super::*;

            #[test]
            fn maps_x_values_to_frequencies() {
                let areas = Areas::new(800, 600, 10, 48);
                assert_eq!(areas.frequency(pos(5)), NoteOn(midi_to_frequency(48)));
            }

            #[test]
            fn maps_higher_x_values_to_higher_frequencies() {
                let areas = Areas::new(800, 600, 10, 48);
                assert_eq!(areas.frequency(pos(15)), NoteOn(midi_to_frequency(49)));
            }

            #[test]
            fn has_non_continuous_steps() {
                let areas = Areas::new(800, 600, 10, 48);
                assert_eq!(areas.frequency(pos(9)), NoteOn(midi_to_frequency(48)));
                assert_eq!(areas.frequency(pos(10)), NoteOn(midi_to_frequency(49)));
            }

            #[test]
            fn allows_to_change_area_size() {
                let areas = Areas::new(800, 600, 12, 48);
                assert_eq!(areas.frequency(pos(11)), NoteOn(midi_to_frequency(48)));
                assert_eq!(areas.frequency(pos(12)), NoteOn(midi_to_frequency(49)));
            }
        }

        mod make_color {
            use super::*;

            #[test]
            fn returns_blue_for_the_middle_c() {
                assert_eq!(Areas::make_color(60), Color::RGB(0, 0, 254));
            }

            #[test]
            fn returns_blue_one_octave_higher() {
                assert_eq!(Areas::make_color(72), Color::RGB(0, 0, 254));
            }

            #[test]
            fn cycles_through_twelve_colors_by_hue_in_cycle_of_fifth() {
                use self::palette::Hsv;
                use self::palette::Srgb;

                let mut color = Hsv::from(Srgb::new(0.0, 0.0, 1.0));
                color.hue = color.hue + 360.0 / 12.0;
                assert_eq!(
                    Areas::make_color(7),
                    Areas::convert_color(Srgb::from(color).into_format())
                );
                color.hue = color.hue + 360.0 / 12.0;
                assert_eq!(
                    Areas::make_color(62),
                    Areas::convert_color(Srgb::from(color).into_format())
                );
            }
        }

        mod ui_elements {
            use super::*;

            #[test]
            fn returns_a_rectangle_for_the_lowest_pitch() {
                let elements = Areas::new(800, 600, 10, 48).ui_elements(800, 600);
                assert_eq!(elements.get(0).unwrap().0, Rect::new(0, 1, 10, 10000));
            }

            #[test]
            fn returns_rectangles_for_higher_pitches() {
                let elements = Areas::new(800, 600, 10, 48).ui_elements(800, 600);
                assert_eq!(elements.get(1).unwrap().0, Rect::new(10, 1, 10, 10000));
                assert_eq!(elements.get(2).unwrap().0, Rect::new(20, 1, 10, 10000));
            }

            #[test]
            fn translates_touch_coordinates_to_screen_coordinates() {
                let elements = Areas::new(1000, 1000, 10, 48).ui_elements(700, 500);
                assert_eq!(elements.get(2).unwrap().0, Rect::new(14, 0, 7, 5000));
            }

            #[test]
            fn factors_in_the_area_size() {
                let elements = Areas::new(1000, 1000, 12, 48).ui_elements(700, 500);
                assert_eq!(
                    elements.get(2).unwrap().0,
                    Rect::new(
                        (24.0 * 0.7) as i32,
                        (1.0 * 0.5) as i32,
                        (12.0 * 0.7) as u32,
                        (10000.0 * 0.5) as u32
                    )
                );
            }

            #[test]
            fn returns_blue_for_c() {
                let elements = Areas::new(1000, 1000, 10, 60).ui_elements(700, 500);
                assert_eq!(elements.get(0).unwrap().1, Color::RGB(0, 0, 254));
            }

            #[test]
            fn returns_blue_for_c_when_starting_at_different_notes() {
                let elements = Areas::new(1000, 1000, 10, 59).ui_elements(700, 500);
                assert_eq!(elements.get(1).unwrap().1, Color::RGB(0, 0, 254));
            }
        }
    }

    mod frequencies {
        use super::*;

        #[test]
        fn yields_frequencies() {
            let areas = Areas::new(800, 600, 10, 48);
            let mut frequencies =
                Frequencies::new(areas, vec![TouchState::Touch(pos(5))].into_iter());
            assert_eq!(frequencies.next(), Some(NoteOn(midi_to_frequency(48))));
        }

        #[test]
        fn yields_notouch_for_pauses() {
            let areas = Areas::new(800, 600, 10, 48);
            let mut frequencies = Frequencies::new(areas, vec![TouchState::NoTouch].into_iter());
            assert_eq!(frequencies.next(), Some(NoteOff));
        }

        #[test]
        fn allows_to_specify_the_starting_note() {
            let areas = Areas::new(800, 600, 10, 49);
            let mut frequencies =
                Frequencies::new(areas, vec![TouchState::Touch(pos(5))].into_iter());
            assert_eq!(frequencies.next(), Some(NoteOn(midi_to_frequency(49))));
        }
    }
}

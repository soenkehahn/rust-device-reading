extern crate evdev_rs;

use evdev::evdev_rs::enums::{EventCode, EventType::*, EV_ABS, EV_SYN::*};
use evdev::evdev_rs::*;
use std::fs::File;
use AddMessage;
use ErrorString;

pub struct InputEventSource {
    _file: File,
    device: Device,
}

impl InputEventSource {
    pub fn new(path: &str) -> Result<InputEventSource, ErrorString> {
        let file = File::open(path).add_message(format!("file not found: {}", path))?;
        let mut device = Device::new().ok_or("evdev: can't initialize device")?;
        device
            .set_fd(&file)
            .add_message(format!("set_fd failed on {}", path))?;
        device.grab(GrabMode::Grab)?;
        Ok(InputEventSource {
            _file: file,
            device,
        })
    }
}

impl Iterator for InputEventSource {
    type Item = InputEvent;

    fn next(&mut self) -> Option<InputEvent> {
        match self.device.next_event(NORMAL | BLOCKING) {
            Err(e) => {
                eprintln!("error: next: {:?}", e);
                self.next()
            }
            Ok((status, event)) => {
                if status == ReadStatus::Sync {
                    eprintln!("ReadStatus == Sync");
                }
                Some(event)
            }
        }
    }
}

pub struct SynChunkSource {
    input_event_source: Box<Iterator<Item = InputEvent>>,
}

impl SynChunkSource {
    pub fn new(input_event_source: impl Iterator<Item = InputEvent> + 'static) -> SynChunkSource {
        SynChunkSource {
            input_event_source: Box::new(input_event_source),
        }
    }
}

impl ::std::fmt::Debug for SynChunkSource {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "<SynChunkSource>")
    }
}

fn is_syn_dropped_event(event: &InputEvent) -> bool {
    match event.event_type {
        EV_SYN => match event.event_code {
            EventCode::EV_SYN(SYN_DROPPED) => true,
            _ => false,
        },
        _ => false,
    }
}

fn is_syn_report_event(event: &InputEvent) -> bool {
    match event.event_type {
        EV_SYN => match event.event_code {
            EventCode::EV_SYN(SYN_REPORT) => true,
            _ => false,
        },
        _ => false,
    }
}

impl Iterator for SynChunkSource {
    type Item = Vec<InputEvent>;

    fn next(&mut self) -> Option<Vec<InputEvent>> {
        let mut result = vec![];
        loop {
            match self.input_event_source.next() {
                None => {
                    if result.is_empty() {
                        return None;
                    } else {
                        break;
                    }
                }
                Some(event) => {
                    if is_syn_dropped_event(&event) {
                        eprintln!("SynChunkSource: dropped events");
                    } else if is_syn_report_event(&event) {
                        break;
                    } else {
                        result.push(event);
                    }
                }
            }
        }
        Some(result)
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy)]
struct SlotState {
    position: Position,
    btn_touch: bool,
}

pub type Slots<T> = [T; 10];

pub fn slot_map<F, T, U: Copy + Default>(input: Slots<T>, f: F) -> Slots<U>
where
    F: Fn(&T) -> U,
{
    let mut result = [U::default(); 10];
    for (i, t) in input.into_iter().enumerate() {
        result[i] = f(t);
    }
    result
}

#[derive(Debug)]
pub struct PositionSource {
    syn_chunk_source: SynChunkSource,
    slots: Slots<SlotState>,
    slot_active: usize,
}

impl PositionSource {
    fn new_from_iterator(
        input_event_source: impl Iterator<Item = InputEvent> + 'static,
    ) -> PositionSource {
        PositionSource {
            syn_chunk_source: SynChunkSource::new(input_event_source),
            slots: [SlotState {
                position: Position { x: 0, y: 0 },
                btn_touch: false,
            }; 10],
            slot_active: 0,
        }
    }

    pub fn new(file: &str) -> Result<PositionSource, ErrorString> {
        Ok(PositionSource::new_from_iterator(InputEventSource::new(
            file,
        )?))
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TouchState<T> {
    NoTouch,
    Touch(T),
}

impl<T> TouchState<T> {
    pub fn get_first<'a, I>(iterator: I) -> &'a TouchState<T>
    where
        I: Iterator<Item = &'a TouchState<T>>,
    {
        for element in iterator {
            if let TouchState::Touch(_) = element {
                return element;
            }
        }
        &TouchState::NoTouch
    }
}

impl Iterator for PositionSource {
    type Item = Slots<TouchState<Position>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.syn_chunk_source.next() {
            None => None,
            Some(chunk) => {
                for event in chunk {
                    if let EV_ABS = event.event_type {
                        match event.event_code {
                            EventCode::EV_ABS(EV_ABS::ABS_MT_SLOT) => {
                                if event.value < self.slots.as_ref().len() as i32 {
                                    self.slot_active = event.value as usize;
                                }
                            }
                            EventCode::EV_ABS(EV_ABS::ABS_MT_POSITION_X) => {
                                self.slots[self.slot_active].position.x = event.value;
                            }
                            EventCode::EV_ABS(EV_ABS::ABS_MT_POSITION_Y) => {
                                self.slots[self.slot_active].position.y = event.value;
                            }
                            EventCode::EV_ABS(EV_ABS::ABS_MT_TRACKING_ID) => match event.value {
                                -1 => self.slots[self.slot_active].btn_touch = false,
                                _ => self.slots[self.slot_active].btn_touch = true,
                            },
                            _ => {}
                        }
                    }
                }
                let mut result = [TouchState::NoTouch; 10];
                for (i, slot_result) in result.iter_mut().enumerate() {
                    if self.slots[i].btn_touch {
                        *slot_result = TouchState::Touch(self.slots[i].position)
                    }
                }
                Some(result)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::TouchState::*;
    use super::*;
    use evdev::evdev_rs::enums::{EventCode, EventType, EV_ABS::*};

    struct Mock;

    impl Mock {
        fn ev(event_type: EventType, event_code: EventCode, value: i32) -> InputEvent {
            InputEvent {
                time: TimeVal {
                    tv_sec: 0,
                    tv_usec: 0,
                },
                event_type,
                event_code,
                value,
            }
        }

        fn positions(vec: Vec<InputEvent>) -> PositionSource {
            PositionSource::new_from_iterator(vec.into_iter())
        }
    }

    mod syn_chunks {
        use super::*;

        #[test]
        fn groups_events_until_ev_syn() {
            let vec = vec![
                Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1),
                Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 2),
                Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
            ];
            assert_eq!(
                SynChunkSource::new(vec.into_iter()).next(),
                Some(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 2),
                ])
            );
        }

        #[test]
        fn bundles_subsequent_chunks_correctly() {
            let vec = vec![
                Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1),
                Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                //
                Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 2),
                Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
            ];
            let mut syn_chunks = SynChunkSource::new(vec.into_iter());
            syn_chunks.next();
            assert_eq!(
                syn_chunks.next(),
                Some(vec![Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 2)])
            );
        }

        #[test]
        fn handles_terminating_streams_gracefully() {
            let vec = vec![Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1)];
            let mut syn_chunks = SynChunkSource::new(vec.into_iter());
            assert_eq!(
                syn_chunks.next(),
                Some(vec![Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1)])
            );
            assert_eq!(syn_chunks.next(), None);
            assert_eq!(syn_chunks.next(), None);
        }
    }

    mod touch_state {
        use super::*;

        mod get_first {
            use super::*;

            #[test]
            fn returns_the_first_element_if_not_none() {
                let array = [Touch(1), Touch(2)];
                assert_eq!(TouchState::get_first(array.iter()), &Touch(1));
            }

            #[test]
            fn returns_the_first_element_that_is_not_none() {
                let array = [NoTouch, Touch(2)];
                assert_eq!(TouchState::get_first(array.iter()), &Touch(2));
            }

            #[test]
            fn returns_no_touch_if_every_element_is_none() {
                let array: [TouchState<i32>; 2] = [NoTouch, NoTouch];
                assert_eq!(TouchState::get_first(array.iter()), &NoTouch);
            }

            #[test]
            fn returns_no_touch_for_an_empty_iterator() {
                let array: [TouchState<i32>; 0] = [];
                assert_eq!(TouchState::get_first(array.iter()), &NoTouch);
            }
        }
    }

    mod positions {
        use super::*;

        mod slot_zero {
            use super::*;

            impl PositionSource {
                pub fn next_slot(&mut self, n: usize) -> Option<TouchState<Position>> {
                    self.next().map(|states| states[n].clone())
                }
            }

            #[test]
            fn relays_a_position() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 0),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 23),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 42),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                assert_eq!(
                    positions.next_slot(0),
                    Some(Touch(Position { x: 23, y: 42 }))
                );
            }

            #[test]
            fn relays_following_positions() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 0),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 23),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 42),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 51),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 84),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                positions.next();
                assert_eq!(
                    positions.next_slot(0),
                    Some(Touch(Position { x: 51, y: 84 }))
                );
            }

            #[test]
            fn handles_syn_chunks_without_y() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 0),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 23),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 42),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 51),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                positions.next();
                assert_eq!(
                    positions.next_slot(0),
                    Some(Touch(Position { x: 51, y: 42 }))
                );
            }

            #[test]
            fn handles_syn_chunks_without_x() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 0),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 23),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 42),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 84),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                positions.next();
                assert_eq!(
                    positions.next_slot(0),
                    Some(Touch(Position { x: 23, y: 84 }))
                );
            }

            #[test]
            fn recognizes_touch_releases() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 0),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 23),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 42),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), -1),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                positions.next();
                assert_eq!(positions.next_slot(0), Some(NoTouch));
            }

            #[test]
            fn ignores_movements_from_other_slots() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 0),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 23),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 42),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 1000),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 1000),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 0),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 51),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 84),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                positions.next();
                assert_eq!(
                    positions.next_slot(0),
                    Some(Touch(Position { x: 23, y: 42 }))
                );
                assert_eq!(
                    positions.next_slot(0),
                    Some(Touch(Position { x: 51, y: 84 }))
                );
            }

            #[test]
            fn ignores_touch_releases_from_other_slots() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 0),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 23),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 42),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 2),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 1000),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 1000),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), -1),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 0),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 51),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 84),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                positions.next();
                assert_eq!(
                    positions.next_slot(0),
                    Some(Touch(Position { x: 23, y: 42 }))
                );
                assert_eq!(
                    positions.next_slot(0),
                    Some(Touch(Position { x: 23, y: 42 }))
                );
                assert_eq!(
                    positions.next_slot(0),
                    Some(Touch(Position { x: 51, y: 84 }))
                );
            }

            #[test]
            fn assumes_slot_zero_at_start() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 23),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 42),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                assert_eq!(
                    positions.next_slot(0),
                    Some(Touch(Position { x: 23, y: 42 }))
                );
            }

            #[test]
            fn tracks_slot_changes_and_touch_releases_in_the_same_syn_chunk_correctly() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 23),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 42),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), -1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 2),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 1000),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 1000),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                assert_eq!(
                    positions.next_slot(0),
                    Some(Touch(Position { x: 23, y: 42 }))
                );
                assert_eq!(positions.next_slot(0), Some(NoTouch));
            }
        }

        mod other_slots {
            use super::*;

            #[test]
            fn relays_a_position_for_other_slots() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 23),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 42),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                assert_eq!(
                    positions.next_slot(1),
                    Some(Touch(Position { x: 23, y: 42 }))
                );
            }

            #[test]
            fn ignores_movements_from_the_zero_slot() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 0),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 23),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 42),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 2),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 1023),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 1042),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 0),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 51),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 84),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_X), 1051),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_POSITION_Y), 1084),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                assert_eq!(positions.next_slot(1), Some(NoTouch));
                assert_eq!(
                    positions.next_slot(1),
                    Some(Touch(Position { x: 1023, y: 1042 }))
                );
                assert_eq!(
                    positions.next_slot(1),
                    Some(Touch(Position { x: 1023, y: 1042 }))
                );
                assert_eq!(
                    positions.next_slot(1),
                    Some(Touch(Position { x: 1051, y: 1084 }))
                );
            }

            #[test]
            fn handles_out_of_bound_slots_gracefully() {
                let mut positions = Mock::positions(vec![
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 1000),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                    //
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_SLOT), 2),
                    Mock::ev(EV_ABS, EventCode::EV_ABS(ABS_MT_TRACKING_ID), 1),
                    Mock::ev(EV_SYN, EventCode::EV_SYN(SYN_REPORT), 0),
                ]);
                positions.next();
            }
        }
    }
}

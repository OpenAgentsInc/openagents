use client_core::facade::controller::ControllerState;
use client_core::systems::cursor::{CursorEvent, HostEvent, UiFocus, handle_cursor_event};
use ecs_core::components::{ControllerMode, InputProfile};

#[test]
fn alt_toggle_switches_modes_and_emits_pointer_lock_events() {
    let mut state = ControllerState {
        profile: InputProfile::ActionCombat,
        mode: ControllerMode::Mouselook,
        ..Default::default()
    };

    let ui = UiFocus::default();

    let mut host_events = Vec::new();
    handle_cursor_event(&mut state, &ui, CursorEvent::Toggle, &mut host_events);
    assert_eq!(state.mode, ControllerMode::Cursor);
    assert!(matches!(
        host_events.as_slice(),
        [HostEvent::PointerLockRequest(false)]
    ));

    host_events.clear();
    handle_cursor_event(&mut state, &ui, CursorEvent::Toggle, &mut host_events);
    assert_eq!(state.mode, ControllerMode::Mouselook);
    assert!(matches!(
        host_events.as_slice(),
        [HostEvent::PointerLockRequest(true)]
    ));
}

#[test]
fn alt_hold_press_release_switches_modes() {
    let mut state = ControllerState {
        mode: ControllerMode::Mouselook,
        ..Default::default()
    };
    let ui = UiFocus::default();
    let mut host_events = Vec::new();
    handle_cursor_event(&mut state, &ui, CursorEvent::Hold(true), &mut host_events);
    assert_eq!(state.mode, ControllerMode::Cursor);
    assert!(matches!(
        host_events.as_slice(),
        [HostEvent::PointerLockRequest(false)]
    ));
    host_events.clear();
    handle_cursor_event(&mut state, &ui, CursorEvent::Hold(false), &mut host_events);
    assert_eq!(state.mode, ControllerMode::Mouselook);
    assert!(matches!(
        host_events.as_slice(),
        [HostEvent::PointerLockRequest(true)]
    ));
}

#[test]
fn classic_profile_rmb_hold_captures_and_releases() {
    let mut state = ControllerState {
        profile: InputProfile::ClassicCursor,
        mode: ControllerMode::Cursor,
        ..Default::default()
    };
    let ui = UiFocus::default();

    let mut host_events = Vec::new();
    handle_cursor_event(
        &mut state,
        &ui,
        CursorEvent::MouseRight(true),
        &mut host_events,
    );
    assert_eq!(state.mode, ControllerMode::Mouselook);
    assert!(matches!(
        host_events.as_slice(),
        [HostEvent::PointerLockRequest(true)]
    ));

    host_events.clear();
    handle_cursor_event(
        &mut state,
        &ui,
        CursorEvent::MouseRight(false),
        &mut host_events,
    );
    assert_eq!(state.mode, ControllerMode::Cursor);
    assert!(matches!(
        host_events.as_slice(),
        [HostEvent::PointerLockRequest(false)]
    ));
}

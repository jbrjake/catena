use super::*;

fn key(code: KeyCode) -> KeyInput {
    KeyInput::new(code, Modifiers::NONE)
}

fn shift(code: KeyCode) -> KeyInput {
    KeyInput::new(code, Modifiers::SHIFT)
}

#[test]
fn the_default_table_binds_plan_section_10_1s_keys() {
    let map = Keymap::default();
    let expect = [
        (key(KeyCode::Char('h')), Action::Pan(Dir::Left)),
        (key(KeyCode::Char('j')), Action::Pan(Dir::Down)),
        (key(KeyCode::Char('k')), Action::Pan(Dir::Up)),
        (key(KeyCode::Char('l')), Action::Pan(Dir::Right)),
        (key(KeyCode::Left), Action::Pan(Dir::Left)),
        (key(KeyCode::Down), Action::Pan(Dir::Down)),
        (key(KeyCode::Up), Action::Pan(Dir::Up)),
        (key(KeyCode::Right), Action::Pan(Dir::Right)),
        (key(KeyCode::Char('+')), Action::ZoomIn),
        (key(KeyCode::Char('=')), Action::ZoomIn),
        (key(KeyCode::Char('-')), Action::ZoomOut),
        (key(KeyCode::Char('n')), Action::CycleNext),
        (key(KeyCode::Char('N')), Action::CyclePrev),
        (shift(KeyCode::Left), Action::NavTo(Dir::Left)),
        (shift(KeyCode::Down), Action::NavTo(Dir::Down)),
        (shift(KeyCode::Up), Action::NavTo(Dir::Up)),
        (shift(KeyCode::Right), Action::NavTo(Dir::Right)),
        (key(KeyCode::Enter), Action::TogglePin),
        (key(KeyCode::Esc), Action::Clear),
        (key(KeyCode::Char('b')), Action::ToggleBundling),
    ];
    assert_eq!(map.bindings().len(), expect.len(), "{:?}", map.bindings());
    for (input, action) in expect {
        assert_eq!(map.action(input), Some(action), "{input:?}");
    }
}

#[test]
fn a_character_key_matches_whatever_shift_the_terminal_reports() {
    // `N` arrives as `Char('N')` with or without SHIFT, depending on the terminal; the
    // character's case already says it.
    let map = Keymap::default();
    assert_eq!(
        map.action(shift(KeyCode::Char('N'))),
        Some(Action::CyclePrev)
    );
    assert_eq!(map.action(key(KeyCode::Char('N'))), Some(Action::CyclePrev));
    assert_eq!(map.action(shift(KeyCode::Char('+'))), Some(Action::ZoomIn));
    assert_eq!(
        KeyInput::new(KeyCode::Char('N'), Modifiers::SHIFT).mods,
        Modifiers::NONE
    );
    // Other modifiers still count: Ctrl+n is not n.
    assert_eq!(
        map.action(KeyInput::new(KeyCode::Char('n'), Modifiers::CTRL)),
        None
    );
    // And SHIFT still counts on keys without a case.
    assert_ne!(
        map.action(shift(KeyCode::Left)),
        map.action(key(KeyCode::Left))
    );
}

#[test]
fn back_tab_is_shift_tab_whatever_shift_the_terminal_reports() {
    // crossterm reports Shift+Tab as BackTab with SHIFT; BackTab already says it.
    assert_eq!(shift(KeyCode::BackTab), key(KeyCode::BackTab));
    let mut map = Keymap::empty();
    map.bind(key(KeyCode::BackTab), Action::CyclePrev);
    assert_eq!(map.action(shift(KeyCode::BackTab)), Some(Action::CyclePrev));
    assert_eq!(
        KeyInput::new(KeyCode::BackTab, Modifiers::SHIFT | Modifiers::ALT).mods,
        Modifiers::ALT
    );
}

#[test]
fn binding_a_key_replaces_its_action_and_keeps_its_place() {
    let mut map = Keymap::default();
    let before: Vec<KeyInput> = map.bindings().iter().map(|&(k, _)| k).collect();
    map.bind(key(KeyCode::Char('h')), Action::ZoomOut);
    assert_eq!(map.action(key(KeyCode::Char('h'))), Some(Action::ZoomOut));
    let after: Vec<KeyInput> = map.bindings().iter().map(|&(k, _)| k).collect();
    assert_eq!(after, before, "same keys, same order");

    map.bind(key(KeyCode::Char('z')), Action::ZoomIn);
    assert_eq!(map.bindings().len(), before.len() + 1);
    assert_eq!(
        map.bindings().last(),
        Some(&(key(KeyCode::Char('z')), Action::ZoomIn))
    );
}

#[test]
fn unbinding_removes_only_that_key() {
    let mut map = Keymap::default();
    let n = map.bindings().len();
    assert_eq!(map.unbind(key(KeyCode::Esc)), Some(Action::Clear));
    assert_eq!(map.action(key(KeyCode::Esc)), None);
    assert_eq!(map.bindings().len(), n - 1);
    assert_eq!(map.unbind(key(KeyCode::Esc)), None);
    assert_eq!(map.action(key(KeyCode::Enter)), Some(Action::TogglePin));
}

#[test]
fn an_empty_keymap_binds_nothing() {
    let map = Keymap::empty();
    assert_eq!(map.bindings().len(), 0);
    assert_eq!(map.action(key(KeyCode::Char('h'))), None);
}

#[test]
fn modifiers_combine_and_test_like_flags() {
    let both = Modifiers::SHIFT | Modifiers::CTRL;
    assert!(both.contains(Modifiers::SHIFT) && both.contains(Modifiers::CTRL));
    assert!(!both.contains(Modifiers::ALT));
    assert!(both.contains(Modifiers::NONE));
    assert_eq!(both.without(Modifiers::SHIFT), Modifiers::CTRL);
    assert!(Modifiers::NONE.is_empty() && !both.is_empty());
}

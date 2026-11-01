use forge_foundation::{CardTypeLine, ColorSet, ManaCost, ZoneType};
use manabrew_engine::{
    card::CardInstance,
    game::GameState,
    ids::{CardId, PlayerId},
    svar::resolve_count_svar,
};

fn card(owner: PlayerId, kind: &str, token: bool) -> CardInstance {
    let mut card = CardInstance::new(
        CardId(0),
        "Test".into(),
        owner,
        CardTypeLine::parse(kind),
        ManaCost::no_cost(),
        ColorSet::COLORLESS,
        None,
        None,
        vec![],
        vec![],
    );
    card.is_token = token;
    card
}

fn count(game: &GameState, source: CardId, player: PlayerId) -> i32 {
    resolve_count_svar(
        "Count$LeftBattlefieldThisTurn Creature.YouCtrl",
        game,
        source,
        player,
    )
}

#[test]
fn counts_each_creature_departure_for_its_controller_and_resets() {
    let mut game = GameState::new(&["Alice", "Bob"], 20);
    let (a, b) = (PlayerId(0), PlayerId(1));
    let source = game.create_card(card(a, "Sorcery", false));
    let creature = game.create_card(card(b, "Creature", false));
    game.move_card(creature, ZoneType::Battlefield, a);
    assert_eq!(game.card(creature).controller, a);
    game.move_card(creature, ZoneType::Exile, b);
    assert_eq!(count(&game, source, a), 1);
    assert_eq!(count(&game, source, b), 0);
    game.move_card(creature, ZoneType::Battlefield, b);
    game.move_card(creature, ZoneType::Hand, b);
    assert_eq!(count(&game, source, a), 1);
    assert_eq!(count(&game, source, b), 1);
    game.new_turn_for_player(a);
    assert_eq!(count(&game, source, a), 0);
    assert_eq!(count(&game, source, b), 0);
}

#[test]
fn counts_tokens_and_battlefield_characteristics_but_not_noncreatures() {
    let mut game = GameState::new(&["Alice", "Bob"], 20);
    let a = PlayerId(0);
    let source = game.create_card(card(a, "Sorcery", false));
    let token = game.create_card(card(a, "Creature", true));
    game.move_card(token, ZoneType::Battlefield, a);
    game.move_card(token, ZoneType::Graveyard, a);
    let land = game.create_card(card(a, "Land", false));
    game.move_card(land, ZoneType::Battlefield, a);
    game.move_card(land, ZoneType::Graveyard, a);
    let animated = game.create_card(card(a, "Land", false));
    game.move_card(animated, ZoneType::Battlefield, a);
    game.card_mut(animated).type_line = CardTypeLine::parse("Land Creature");
    game.move_card(animated, ZoneType::Exile, a);
    game.card_mut(animated).type_line = CardTypeLine::parse("Land");
    assert_eq!(count(&game, source, a), 2);
}

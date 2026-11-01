use forge_foundation::{CardTypeLine, ColorSet, ManaCost, ZoneType};
use manabrew_engine::{
    card::CardInstance,
    game::GameState,
    ids::{CardId, PlayerId},
    svar::resolve_count_svar,
};
#[test]
fn putting_creature_on_bottom_counts_departure() {
    let mut game = GameState::new(&["Alice", "Bob"], 20);
    let player = PlayerId(0);
    let card = game.create_card(CardInstance::new(
        CardId(0),
        "Creature".into(),
        player,
        CardTypeLine::parse("Creature"),
        ManaCost::no_cost(),
        ColorSet::COLORLESS,
        Some(2),
        Some(2),
        vec![],
        vec![],
    ));
    game.move_card(card, ZoneType::Battlefield, player);
    game.put_on_bottom_of_library(card, player);
    assert_eq!(game.card(card).zone, ZoneType::Library);
    assert_eq!(
        resolve_count_svar(
            "Count$LeftBattlefieldThisTurn Creature.YouCtrl",
            &game,
            card,
            player
        ),
        1
    );
}

#[test]
fn bottom_counts_battlefield_controller_and_tokens_but_not_other_zones_or_lands() {
    let mut game = GameState::new(&["Alice", "Bob"], 20);
    let (owner, controller) = (PlayerId(0), PlayerId(1));
    let mut token = CardInstance::new(
        CardId(0),
        "Token".into(),
        owner,
        CardTypeLine::parse("Creature"),
        ManaCost::no_cost(),
        ColorSet::COLORLESS,
        Some(2),
        Some(2),
        vec![],
        vec![],
    );
    token.is_token = true;
    let card = game.create_card(token);
    game.move_card(card, ZoneType::Battlefield, controller);
    game.put_on_bottom_of_library(card, owner);
    assert_eq!(
        resolve_count_svar(
            "Count$LeftBattlefieldThisTurn Creature.YouCtrl",
            &game,
            card,
            controller
        ),
        1
    );
    assert_eq!(
        resolve_count_svar(
            "Count$LeftBattlefieldThisTurn Creature.YouCtrl",
            &game,
            card,
            owner
        ),
        0
    );
    let land = game.create_card(CardInstance::new(
        CardId(0),
        "Land".into(),
        controller,
        CardTypeLine::parse("Land"),
        ManaCost::no_cost(),
        ColorSet::COLORLESS,
        None,
        None,
        vec![],
        vec![],
    ));
    game.move_card(land, ZoneType::Battlefield, controller);
    game.put_on_bottom_of_library(land, controller);
    game.put_on_bottom_of_library(land, controller);
    assert_eq!(
        resolve_count_svar(
            "Count$LeftBattlefieldThisTurn Creature.YouCtrl",
            &game,
            card,
            controller
        ),
        1
    );
}

use forge_foundation::{CardTypeLine, ColorSet, ManaCost, ZoneType};
use manabrew_engine::{
    card::CardInstance,
    game::GameState,
    ids::{CardId, PlayerId},
    replacement::{replacement_handler::apply_replacements, ReplacementEvent, ReplacementResult},
    svar::resolve_count_svar,
};

#[test]
fn prevented_creature_departure_is_not_counted() {
    let mut game = GameState::new(&["Alice", "Bob"], 20);
    let player = PlayerId(0);
    let replacement = "R$ Event$ Moved | Destination$ Graveyard | Origin$ Battlefield | ValidCard$ Card.Self | ReplacementResult$ Prevented";
    let card = game.create_card(CardInstance::new(
        CardId(0),
        "Protected".into(),
        player,
        CardTypeLine::parse("Creature"),
        ManaCost::no_cost(),
        ColorSet::COLORLESS,
        Some(2),
        Some(2),
        vec![],
        vec![replacement.into()],
    ));
    game.move_card(card, ZoneType::Battlefield, player);
    let mut event = ReplacementEvent::Moved {
        card,
        origin: ZoneType::Battlefield,
        destination: ZoneType::Graveyard,
        is_discard: false,
        counter_map: None,
        counter_cause: None,
        counter_is_effect: false,
        after_replacement_static_abilities: Vec::new(),
    };
    assert_eq!(
        apply_replacements(&mut game, &mut event),
        ReplacementResult::Prevented
    );
    game.move_card(card, ZoneType::Graveyard, player);
    assert_eq!(
        game.card(card).zone,
        ZoneType::Battlefield,
        "prevented move must leave card on battlefield"
    );
    assert_eq!(
        resolve_count_svar(
            "Count$LeftBattlefieldThisTurn Creature.YouCtrl",
            &game,
            card,
            player
        ),
        0
    );
}

#[test]
fn redirected_departure_is_counted_and_same_zone_is_not() {
    let mut game = GameState::new(&["Alice", "Bob"], 20);
    let player = PlayerId(0);
    let redirect = "R$ Event$ Moved | Destination$ Graveyard | Origin$ Battlefield | ValidCard$ Card.Self | NewDestination$ Exile";
    let card = game.create_card(CardInstance::new(
        CardId(0),
        "Redirect".into(),
        player,
        CardTypeLine::parse("Creature"),
        ManaCost::no_cost(),
        ColorSet::COLORLESS,
        Some(2),
        Some(2),
        vec![],
        vec![redirect.into()],
    ));
    game.move_card(card, ZoneType::Battlefield, player);
    game.move_card(card, ZoneType::Battlefield, player);
    assert_eq!(
        resolve_count_svar(
            "Count$LeftBattlefieldThisTurn Creature.YouCtrl",
            &game,
            card,
            player
        ),
        0
    );
    game.move_card(card, ZoneType::Graveyard, player);
    assert_eq!(game.card(card).zone, ZoneType::Exile);
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
